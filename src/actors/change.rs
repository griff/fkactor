use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::DerefMut;
use std::sync::Arc;

use async_trait::async_trait;
use futures_util::lock::Mutex;
use serde::{Deserialize, Serialize};

use super::{Context, Handler, Status, StatusResult, TypedAid, UntypedAid};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Changed<C> {
	Added(C),
	Updated(C),
	Removed,
}

impl<C> Changed<C> {
    pub fn map<F, R>(self, map: F) -> Changed<R>
        where F: FnOnce(C) -> R
    {
        match self {
            Changed::Added(c) => Changed::Added(map(c)),
            Changed::Updated(c) => Changed::Updated(map(c)),
            Changed::Removed => Changed::Removed,
        }
    }

	pub fn value(&self) -> Option<&C> {
		match self {
			Changed::Added(ref c) => Some(c),
			Changed::Updated(ref c) => Some(c),
			Changed::Removed => None,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Change<I,C> {
	pub id: I,
	pub change: Changed<C>,
}

impl<I,C> Change<I,C> {
	pub fn added(id:I, change: C) -> Change<I,C> {
		Change {
			id,
			change: Changed::Added(change),
		}
	}

	pub fn removed(id:I) -> Change<I,C>
    {
		Change {
			id,
			change: Changed::Removed,
		}
	}

	pub fn updated(id:I, change: C) -> Change<I,C> {
		Change {
			id,
			change: Changed::Updated(change),
		}
	}

	pub fn value(&self) -> Option<&C> {
		self.change.value()
	}

    pub fn map_id<F, R>(self, map: F) -> Change<R, C>
        where F: FnOnce(I) -> R
    {
        Change {
            id: map(self.id),
            change: self.change,
        }
    }

    pub fn map_change<F, R>(self, map: F) -> Change<I, R>
        where F: FnOnce(C) -> R
    {
        Change {
            id: self.id,
            change: self.change.map(map),
        }
    }
}

impl<I, C> Clone for Change<I, C>
    where I: Clone,
          C: Clone,
{
    fn clone(&self) -> Self {
        Change {
            id: self.id.clone(),
            change: self.change.clone(),
        }
    }
}



pub trait ChangeUpdater<M>
{
	fn update(&mut self, msg: M);
}

impl<C,T> ChangeUpdater<Box<C>> for T
	where T: for<'c> ChangeUpdater<&'c C>
{
	fn update(&mut self, msg: Box<C>) {
		self.update(&msg)
	}
}
/*
impl<I,C> ChangeUpdater<Box<Change<I,C>>> for HashMap<I,C>
	where I: Eq + Hash + Debug + Clone,
		  C: Clone + Debug,
{
	fn update(&mut self, msg: Box<Change<I,C>>) {
        match &msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			}
		}
	}
}
*/
impl<'c, I,C> ChangeUpdater<&'c Change<I,C>> for HashMap<I,C>
	where I: Eq + Hash + Debug + Clone,
		  C: Clone + Debug,
{
	fn update(&mut self, msg: &'c Change<I,C>) {
        match &msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			}
		}
	}
}
impl<I,C> ChangeUpdater<Change<I,C>> for HashMap<I,C>
	where I: Eq + Hash + Debug + Clone,
		  C: Clone + Debug,
{
	fn update(&mut self, msg: Change<I,C>) {
        match msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id, change);
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id, change);
			}
		}
	}
}
impl<'c, I,C> ChangeUpdater<&'c Change<I,C>> for BTreeMap<I,C>
	where I: Eq + Ord + Hash + Debug + Clone,
		  C: Clone + Debug,
{
	fn update(&mut self, msg: &'c Change<I,C>) {
        match &msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			}
		}
	}
}
impl<I,C> ChangeUpdater<Change<I,C>> for BTreeMap<I,C>
	where I: Eq + Ord + Hash + Debug + Clone,
		  C: Clone + Debug,
{
	fn update(&mut self, msg: Change<I,C>) {
        match msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id, change);
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id, change);
			}
		}
	}
}

pub type ChangeDB<I,C> = HashMap<I, C>;

#[async_trait]
impl<I,C> Handler<Change<I,C>> for ChangeDB<I,C>
    where I: Eq + Hash + Debug + Clone + Send + Sync + 'static,
          C: Debug + Clone + Send + Sync + 'static,
{
    async fn handle_message(&mut self, _: &mut Context, msg: Change<I,C>) -> StatusResult {
        match &msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			}
		}
        Ok(Status::Done)
    }
}

#[async_trait]
impl<I,C> Handler<Change<I,C>> for BTreeMap<I,C>
    where I: Eq + Ord + Hash + Debug + Clone + Send + Sync + 'static,
          C: Debug + Clone + Send + Sync + 'static,
{
    async fn handle_message(&mut self, _: &mut Context, msg: Change<I,C>) -> StatusResult {
        match &msg.change {
			Changed::Added(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			},
			Changed::Removed => {
                eprintln!("Remove {:?}", msg.id);
				self.remove(&msg.id);
			},
			Changed::Updated(change) => {
                eprintln!("Added {:?}: {:?}", msg.id, change);
				self.insert(msg.id.clone(), change.clone());
			}
		}
        Ok(Status::Done)
    }
}

pub type LockedChangeDB<I,C> = Arc<Mutex<ChangeDB<I,C>>>;

#[derive(Debug, Clone)]
pub struct EventedDB<I,C>
    where I: Send + 'static,
          C: Send + 'static,
{
	pub db: LockedChangeDB<I,C>,
	pub events: super::Fanout<Change<I,C>>,
}

//pub struct Locked<S>(pub(crate) Arc<Mutex<S>>);
pub struct ArcMutex<S, L, U> {
	pub(crate) data: Arc<Mutex<S>>,
	pub(crate) lens: L,
	pub(crate) phantom: std::marker::PhantomData<U>,
}

#[async_trait]
impl<T, S, L, U> Handler<T> for ArcMutex<S, L, U>
	where U: ChangeUpdater<T> + Send + 'static,
		  T: Send + 'static,
		  S: Send,
		  L: crate::Lens<S, U> + Send,
{
	async fn handle_message(&mut self, _: &mut Context, msg: T) -> StatusResult {
		let mut target = self.data.lock().await;
		let content = target.deref_mut();
		self.lens.with_mut(content, |inner:&mut U| {
			inner.update(msg);
		});
		Ok(Status::Done)
	}
}

pub struct ArcMutexFwd<S, L, U, F> {
	pub(crate) data: Arc<Mutex<S>>,
	pub(crate) lens: L,
	pub(crate) phantom: std::marker::PhantomData<U>,
	pub(crate) fwd: TypedAid<F>,
}

#[async_trait]
impl<T, S, L, U, F> Handler<T> for ArcMutexFwd<S, L, U, F>
	where U: for<'c> ChangeUpdater<&'c T> + Send + 'static,
		  T: Send + 'static,
		  S: Send,
		  L: crate::Lens<S, U> + Send,
		  T: Into<F>,
		  F: Debug + Send + 'static,
{
	async fn handle_message(&mut self, _: &mut Context, msg: T) -> StatusResult {
		let mut target = self.data.lock().await;
		let content = target.deref_mut();
		self.lens.with_mut(content, |inner:&mut U| {
			inner.update(&msg);
		});
		self.fwd.send(msg.into()).await?;
		Ok(Status::Done)
	}
}

#[async_trait]
impl<T, S> Handler<T> for Arc<Mutex<S>>
	where S: Handler<T> + Send + 'static,
		  T: Send + 'static,
{
	async fn handle_message(&mut self, context: &mut Context, msg: T) -> StatusResult {
		let mut target = self.lock().await;
		let content = target.deref_mut();
		content.handle_message(context, msg).await
	}

	async fn handle_start(&mut self, context: &mut Context) -> StatusResult {
		let mut target = self.lock().await;
		let content = target.deref_mut();
		content.handle_start(context).await
    }

	async fn handle_shutdown(&mut self, context: &mut Context) -> StatusResult {
		let mut target = self.lock().await;
		let content = target.deref_mut();
		content.handle_shutdown(context).await
	}

	async fn handle_stopped(&mut self, context: &mut Context, aid: UntypedAid, error: Option<Arc<String>>) -> StatusResult {
		let mut target = self.lock().await;
		let content = target.deref_mut();
		content.handle_stopped(context, aid, error).await
	}
}

