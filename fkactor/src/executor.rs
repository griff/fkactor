use std::collections::HashMap;
use std::future::Future;
use std::task::{self, Poll};
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::sink::Sink;
use futures::stream::{Stream, StreamExt};
use futures_util::lock::Mutex;
use tokio::task::{spawn, JoinHandle};

use super::StdError;
use super::actors::{UntypedAid, Context, Handler, Status, StatusResult};
use super::system::{Message};

#[derive(Debug, Clone)]
pub(crate) enum SystemManage {
    Started(UntypedAid, UntypedAid),
    RegisterMonitor(UntypedAid, UntypedAid),
}

async fn run_actor<M, St, H>(mut ctx: Context, stream: St, handler: H)
    where St: Stream<Item=Message<M>> + Send + 'static,
          H: Handler<M> + Send + Unpin + 'static,
          M: Send + 'static,
{
    ctx.system.notify_started(ctx.aid.clone()).await;
    let run = RunHandler::Ready(ctx.clone(), handler);
    let error = if let Err(RunHandlerError::Error(err)) = stream.map(Ok).forward(run).await {
        Some(Arc::new(err.to_string()))
    } else {
        None
    };
    ctx.system.notify_stopped(ctx.aid, error).await;
}


pub(crate) struct ActorManager {
    monitored: HashMap<UntypedAid, Vec<UntypedAid>>,
    actors: Arc<Mutex<HashMap<UntypedAid, UntypedAid>>>,
    /*
    /// Holds a map of the actor ids by the UUID in the actor id. UUIDs of actor ids are assigned
    /// when an actor is spawned using version 4 UUIDs.
    aids_by_uuid: Arc<DashMap<Uuid, Aid>>,
    /// Holds a map of user assigned names to actor ids set when the actors were spawned. Note
    /// that only actors with an assigned name will be in this map.
    aids_by_name: Arc<DashMap<String, Aid>>,
    /// Holds a map of monitors where the key is the `aid` of the actor being monitored and
    /// the value is a vector of `aid`s that are monitoring the actor.
    monitoring_by_monitored: Arc<DashMap<Aid, HashSet<Aid>>>,
    */
}

impl ActorManager {
    pub fn new(actors: Arc<Mutex<HashMap<UntypedAid, UntypedAid>>>) -> Self {
        ActorManager {
            monitored: Default::default(),
            actors,
        }
    }
}


#[async_trait]
impl Handler<SystemManage> for ActorManager {
    async fn handle_message(&mut self, _context: &mut Context, msg: SystemManage) -> StatusResult {
        match msg {
            SystemManage::RegisterMonitor(aid, sender) => {
                self.monitored.entry(aid).or_default().push(sender);
            }
            SystemManage::Started(aid, sender) => {
                let mut map = self.actors.lock().await;
                map.insert(aid, sender);
            },
        }
        Ok(Status::Done)
    }
    async fn handle_shutdown(&mut self, _context: &mut Context) -> StatusResult {
        for (aid, senders) in self.monitored.drain() {
            for mut sender in senders {
                sender.stopped(aid.clone(), None).await.unwrap_or_else(|err| {
                    eprintln!("Could not notfiy of stopped {:?}", err);
                });
            }
        }
        Ok(Status::Done)
    }
    async fn handle_stopped(&mut self, _context: &mut Context, aid: UntypedAid, error: Option<Arc<String>>) -> StatusResult {
        let mut map = self.actors.lock().await;
        map.remove(&aid);
        if let Some(senders) = self.monitored.get_mut(&aid) {
            for sender in senders.iter_mut() {
                sender.stopped(aid.clone(), error.clone()).await.unwrap_or_else(|err| {
                    eprintln!("Could not notfiy of stopped {:?}", err);
                });
            }
        }
        Ok(Status::Done)
    }

}


pub enum RunHandlerError {
    Stopped,
    Error(StdError),
}

pub enum RunHandler<H> {
    Ready(Context, H),
    Processing(Pin<Box<dyn Future<Output=Result<(Context, H, Status), StdError>> + Send>>),
    Invalid,
}

impl<H> RunHandler<H>
{
    fn take(&mut self) -> Self {
        std::mem::replace(self, RunHandler::Invalid)
    }

    pub fn is_ready(&self) -> bool {
        match self {
            RunHandler::Ready(..) => true,
            _ => false
        }
    }
}

impl<'h, H,M> Sink<Message<M>> for RunHandler<H>
    where H: Handler<M> + Send + Unpin + 'static,
          M: Send + 'static
{
    type Error = RunHandlerError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), Self::Error>>
    {
        match self.take() {
            RunHandler::Invalid => panic!("Runhandler polled while invalid"),
            a @ RunHandler::Ready(..) => {
                *self = a;
                Poll::Ready(Ok(()))
            }
            RunHandler::Processing(mut fut) => {
                match fut.as_mut().poll(cx) {
                    Poll::Ready(Err(err)) => {
                        Poll::Ready(Err(RunHandlerError::Error(err)))
                    },
                    Poll::Ready(Ok((.., Status::Stop))) => {
                        Poll::Ready(Err(RunHandlerError::Stopped))
                    },
                    Poll::Ready(Ok((ctx, h, Status::Done))) => {
                        *self = RunHandler::Ready(ctx, h);
                        Poll::Ready(Ok(()))
                    },
                    Poll::Pending => {
                        *self = RunHandler::Processing(fut);
                        Poll::Pending
                    }
                }
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message<M>) -> Result<(), Self::Error>
    {
        if let RunHandler::Ready(mut ctx, mut h) = self.take() {
            let fut = async move {
                match h.handle(&mut ctx, item).await {
                    Ok(s) => Ok((ctx, h, s)),
                    Err(err) => Err(err),
                }
            };
            *self = RunHandler::Processing(Box::pin(fut));
            Ok(())
        } else {
            panic!("start_send called without being ready");
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), Self::Error>>
    {
        self.poll_ready(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), Self::Error>>
    {
        let ret = self.as_mut().poll_ready(cx);
        match ret {
            Poll::Ready(Ok(())) => {
                *self = RunHandler::Invalid;
            }
            Poll::Ready(Err(RunHandlerError::Stopped)) => {
                return Poll::Ready(Ok(()));
            }
            _ => {},
        }
        ret
    }
} 



enum ExecutorState {
    Initializing(Vec<Box<dyn Future<Output=()> + Send + Unpin>>),
    Started(Vec<JoinHandle<()>>),
    Invalid
}

impl ExecutorState {
    fn take(&mut self) -> Self {
        std::mem::replace(self, ExecutorState::Invalid)
    }

    pub fn start(&mut self) {
        match self.take() {
            ExecutorState::Initializing(actors) => {
                let mut joins = Vec::with_capacity(actors.len());
                for actor in actors.into_iter() {
                    joins.push(spawn(actor));
                }
                *self = ExecutorState::Started(joins);
            },
            _ => {}
        }
    }

    pub fn spawn<T>(&mut self, fut: T)
        where T: Future<Output=()> + Unpin + Send + 'static,
    {
        match self {
            ExecutorState::Initializing(ref mut actors) => {
                actors.push(Box::new(fut))
            },
            ExecutorState::Started(ref mut joins) => {
                joins.push(spawn(fut))
            },
            _ => panic!("Invalid executor state"),
        }
    }
}

impl Default for ExecutorState {
    fn default() -> Self {
        ExecutorState::Initializing(Vec::new())
    }
}

#[derive(Default, Clone)]
pub struct Executor {
    inner: Arc<Mutex<ExecutorState>>,
}

impl Executor {
    pub async fn start(&mut self) {
        self.inner.lock().await.start()
    }

    pub fn run_sync<M, St, H>(&mut self, ctx: Context, stream: St, handler: H)
        where St: Stream<Item=Message<M>> + Send + 'static,
               H: Handler<M> + Send + Unpin + 'static,
               M: Send + 'static,
    {
        let fut = Box::pin(run_actor(ctx, stream, handler));
        self.inner.try_lock().expect("Unlocked").spawn(fut)
    }

    pub async fn run<M, St, H>(&mut self, ctx: Context, stream: St, handler: H)
        where St: Stream<Item=Message<M>> + Send + 'static,
               H: Handler<M> + Send + Unpin + 'static,
               M: Send + 'static,
    {
        let fut = Box::pin(run_actor(ctx, stream, handler));
        self.spawn(fut).await
    }

    pub async fn spawn<T>(&mut self, fut: T)
        where T: Future<Output=()> + Unpin + Send + 'static,
    {
        self.inner.lock().await.spawn(fut)
    }
}
