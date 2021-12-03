use std::{fmt::Debug, sync::Arc, ops::DerefMut};

use async_trait::async_trait;
use futures::lock::Mutex;
use serde::{Deserialize, Serialize};

use super::{Context, Handler, StatusResult, UntypedAid};

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