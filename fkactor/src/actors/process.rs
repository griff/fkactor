use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::io;
use std::pin::Pin;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::channel::mpsc;
use futures_util::future::{ready, TryFutureExt};
use futures_util::stream::StreamExt;
use futures_util::sink::SinkExt;
use tokio::spawn;

use super::{Change, Changed, Context, Handler, Status, StatusResult, TypedAid, UntypedAid};

struct Counter {
	stopper: Option<oneshot::Sender<()>>,
	count: usize,
}
pub struct Process<F, A, I, R> {
    counts: HashMap<I, Counter>,
    spawner: mpsc::Sender<R>,
	recipient: TypedAid<A>,
	start: F,
}

impl<A, I, F, R> Process<F, A, I, R>
    where R: Future<Output=io::Result<()>> + Send + 'static,
          A: Send + 'static,
          F: FnMut(TypedAid<A>, I, oneshot::Receiver<()>) -> R,
          I: Eq + Hash + Clone + Send + Sync + 'static,
{
    pub fn new(recipient: TypedAid<A>, start: F) -> Process<F, A, I, R> {
        let (spawner, receiver) = mpsc::channel(10);
        spawn(receiver.for_each(|f: R| {
            spawn(f
                .inspect_err(|err| eprintln!("update id error {:?}", err))
                .unwrap_or_else(|_| ()));
            ready(())
        }));
        Process {
            counts: Default::default(),
            recipient, start, spawner,
        }
    }

	async fn handle_add(&mut self, id: &I) -> StatusResult {
		if let Some(mut counter) = self.counts.get_mut(id) {
			counter.count += 1;
		} else {
            let (s, r) = oneshot::channel();
            let recipient = self.recipient.clone();
			self.spawner.send((self.start)(recipient, id.clone(), r)).await.unwrap();
			self.counts.insert(id.clone(), Counter {
				stopper: Some(s),
				count: 1,
			});
		}
		Ok(Status::Done)
	}

	fn handle_remove(&mut self, id: &I) -> StatusResult {
        let mut remove = false;
		if let Some(mut counter) = self.counts.get_mut(id) {
			if counter.count <= 1 {
                remove = true;
				if let Some(e) = counter.stopper.take() {
					drop(e.send(()));
				}
			} else {
				counter.count -= 1;
			}
		}
        if remove {
            self.counts.remove(id);
        }
		Ok(Status::Done)
	}
}

//#[async_trait]
impl<A, I, F, R> Handler<Change<I, ()>> for Process<F, A, I, R>
where R: Future<Output=io::Result<()>> + Send + 'static,
      A: Debug + Send + 'static,
      F: (FnMut(TypedAid<A>, I, oneshot::Receiver<()>) -> R) + Send,
      I: Eq + Hash + Debug + Clone + Send + Sync + 'static,
{
    /*
    async fn handle_message(&mut self, context: &mut Context, msg: Box<Change<I, ()>>) -> StatusResult {
        match &msg.change {
            Changed::Added(_) => self.handle_add(&msg.id).await,
            Changed::Updated(_) => Ok(Status::Done),
            Changed::Removed => self.handle_remove(&msg.id),
        }
    }

    async fn handle_start(&mut self, context: &mut Context) -> StatusResult {
        context.system.monitor(context.aid.clone(), self.recipient.clone()).await;
        Ok(Status::Done)
    }

    async fn handle_shutdown(&mut self, context: &mut Context) -> StatusResult {
        for (_key, mut counter) in self.counts.drain() {
            if let Some(e) = counter.stopper.take() {
                drop(e.send(()));
            }
        }
        Ok(Status::Stop)
    }

    async fn handle_stopped(&mut self, context: &mut Context, aid: UntypedAid, error: Option<Arc<String>>) -> StatusResult {
        if self.recipient == aid  {
            Ok(Status::Stop)
        } else {
            Ok(Status::Done)
        }
    }
    */

    fn handle_message<'this, 'ctx, 'async_trait>(&'this mut self, _: &'ctx mut Context, msg: Change<I, ()>) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
    where 'this: 'async_trait,
           'ctx: 'async_trait,
    {
        Box::pin(async move {
            match &msg.change {
                Changed::Added(_) => self.handle_add(&msg.id).await,
                Changed::Updated(_) => Ok(Status::Done),
                Changed::Removed => self.handle_remove(&msg.id),
            }
        })
    }

    fn handle_start<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
            'ctx: 'async_trait,
    {
        Box::pin(async move {
            let monitored = self.recipient.clone().untyped();
            context.system.monitor(context.aid.clone(), monitored).await;
            Ok(Status::Done)
        })
    }

    fn handle_shutdown<'this, 'ctx, 'async_trait>(&'this mut self, _: &'ctx mut Context) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
            'ctx: 'async_trait,
    {
        Box::pin(async move {
            for (_key, mut counter) in self.counts.drain() {
                if let Some(e) = counter.stopper.take() {
                    drop(e.send(()));
                }
            }
            Ok(Status::Stop)
        })
    }

    fn handle_stopped<'this, 'ctx, 'async_trait>(&'this mut self, _: &'ctx mut Context, aid: UntypedAid, _error: Option<Arc<String>>) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
            'ctx: 'async_trait,
    {
        Box::pin(async move {
            if self.recipient == aid  {
                Ok(Status::Stop)
            } else {
                Ok(Status::Done)
            }
        })
    }

}