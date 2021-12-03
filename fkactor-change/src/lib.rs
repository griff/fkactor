use std::{fmt, marker::PhantomData};
use std::sync::Arc;

use async_trait::async_trait;
use fkactor::actors::{TypedAid, ActorBuilder};
use futures::lock::Mutex;

mod change;

pub use self::change::{Change, Changed, ChangeDB, ChangeUpdater, EventedDB};
use self::change::{ArcMutex, ArcMutexFwd};

#[async_trait]
pub trait ActorBuilderExt {
    async fn mutex<T, S, L, U>(self, data: Arc<Mutex<S>>, lens: L) -> TypedAid<T>
    	where U: ChangeUpdater<T> + Send + Unpin + 'static,
	    	  T: fmt::Debug + Send + Unpin + 'static,
		      S: Send + 'static,
		      L: lens::Lens<S, U> + Send + Unpin + 'static;

    async fn mutex_forward<T, S, L, U, F>(self, data: Arc<Mutex<S>>, lens: L, fwd: TypedAid<F>) -> TypedAid<T>
        where U: for<'c> ChangeUpdater<&'c T> + Send + Unpin + 'static,
              T: fmt::Debug + Send + Unpin + 'static,
              S: Send + 'static,
              L: lens::Lens<S, U> + Send + Unpin + 'static,
              T: Into<F>,
              F: fmt::Debug + Send + Unpin + 'static;
}

#[async_trait]
impl ActorBuilderExt for ActorBuilder {
    async fn mutex<T, S, L, U>(self, data: Arc<Mutex<S>>, lens: L) -> TypedAid<T>
        where U: ChangeUpdater<T> + Send + Unpin + 'static,
            T: fmt::Debug + Send + Unpin + 'static,
            S: Send + 'static,
            L: lens::Lens<S, U> + Send + Unpin + 'static,
    {
        let handler = ArcMutex {
            data, lens,
            phantom: PhantomData,        
        };
        self.with(handler).await
    }

    async fn mutex_forward<T, S, L, U, F>(self, data: Arc<Mutex<S>>, lens: L, fwd: TypedAid<F>) -> TypedAid<T>
        where U: for<'c> ChangeUpdater<&'c T> + Send + Unpin + 'static,
            T: fmt::Debug + Send + Unpin + 'static,
            S: Send + 'static,
            L: lens::Lens<S, U> + Send + Unpin + 'static,
            T: Into<F>,
            F: fmt::Debug + Send + Unpin + 'static,
    {
        let handler = ArcMutexFwd {
            data, lens, fwd,
            phantom: PhantomData,        
        };
        self.with(handler).await
    }
}