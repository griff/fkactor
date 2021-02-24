use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use super::{Change, Changed, AidError, Context, Handler, Status, StatusResult, TypedAid, UntypedAid};

pub type FanoutAid<M> = TypedAid<FanoutMessage<M>>;

#[derive(Debug, Clone)]
pub enum FanoutMessage<M> {
    Subscription(Change<TypedAid<M>, ()>),
    Message(M),
}

impl<M> From<M> for FanoutMessage<M>
{
    fn from(v: M) -> FanoutMessage<M> {
        FanoutMessage::Message(v)
    }
}

/*
impl<F,M> From<Box<F>> for FanoutMessage<M>
    where Box<F>: Into<Box<M>>
{
    fn from(f: Box<F>) -> FanoutMessage<M> {
        FanoutMessage::Message(f.into())
    }
}
*/
/*
impl<F, M> From<F> for FanoutMessage<M>
    where F: Into<Box<M>>
{
    fn from(v: F) -> FanoutMessage<M> {
        FanoutMessage::Message(v.into())
    }
}
*/
/*
impl<M> From<Box<M>> for Box<FanoutMessage<M>>
    where M: Clone
{
    fn from(v: Box<M>) -> Box<FanoutMessage<M>> {
        Box::new(FanoutMessage::Message(v.clone()))
    }
}
*/


#[derive(Debug, Clone)]
pub struct Fanout<M> {
    inner: TypedAid<FanoutMessage<M>>,
}

impl<M> Fanout<M>
    where M: Debug + Clone + Send + 'static
{
    pub(crate) fn new(aid: TypedAid<FanoutMessage<M>>) -> Fanout<M> {
        Fanout {
            inner: aid,
        }
    }

	pub async fn subscribe<A: Into<TypedAid<M>>>(&mut self, aid: A) -> Result<(), AidError>
	{
		self.inner.send(FanoutMessage::Subscription(Change::added(aid.into(), ()))).await
    }
    
	pub async fn unsubscribe<A: Into<TypedAid<M>>>(&mut self, aid: A) -> Result<(), AidError>
	{
		self.inner.send(FanoutMessage::Subscription(Change::removed(aid.into()) as Change<TypedAid<M>, ()>)).await
	}
}

pub struct FanoutHandler<M> {
    fan: HashSet<TypedAid<M>>,
    fan2: HashSet<TypedAid<M>>,
}

impl<M> Default for FanoutHandler<M> {
    fn default() -> Self {
        FanoutHandler {
            fan: Default::default(),
            fan2: Default::default(),
        }
    }
}

#[async_trait]
impl<M> Handler<FanoutMessage<M>> for FanoutHandler<M>
    where M: Debug + Clone + Send + Sync + 'static,
{
    async fn handle_message(&mut self, context: &mut Context, msg: FanoutMessage<M>) -> StatusResult {
        match msg {
            FanoutMessage::Subscription(change) => {
                match change.change {
                    Changed::Added(_) | Changed::Updated(_) => {
                        self.fan.insert(change.id.clone());
                        context.system.monitor(context.aid.clone(), change.id).await;
                    },
                    Changed::Removed => {
                        self.fan.remove(&change.id);
                    }
                }
            }
            FanoutMessage::Message(m) => {
                for mut aid in self.fan.drain() {
                    let m = m.clone();
                    let fut = Box::pin(async move{
                        let msg = m.clone();
                        aid.send(msg).await.map(move |_| aid)
                    });
                    match fut.await {
                        Ok(aid) => {
                            self.fan2.insert(aid);
                        },
                        Err(AidError::ActorAlreadyStopped) => {

                        },
                        Err(err) => {
                            return Err(err.into());
                        }
                    }
                }
                std::mem::swap(&mut self.fan, &mut self.fan2);
            }
        }
        Ok(Status::Done)
    }

    async fn handle_stopped(&mut self, _context: &mut Context, aid: UntypedAid, _error: Option<Arc<String>>) -> StatusResult {
        self.fan.remove(aid.as_ref());
        Ok(Status::Done)
    }

}
