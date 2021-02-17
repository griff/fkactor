use std::fmt;
use std::future::Future;

use futures::future::{ready, FutureExt};
use futures::stream::{Stream, StreamExt};

use super::{Handler, TypedAid};
use crate::system::{ActorSystem, Message};

pub struct ActorStreamBuilder<M, St> {
    pub(crate) aid: TypedAid<M>,
    pub(crate) stream: St,
    pub system: ActorSystem,
}

impl<M1, M2, St> ActorStreamBuilder<M1, St>
    where St: Stream<Item=Message<M2>> + Send + 'static,
          M1: fmt::Debug + Send + 'static,
          M2: fmt::Debug + Send + 'static,
{
    pub fn with<F, R, M3>(self, map: F) -> ActorStreamBuilder<M1, R>
        where F: FnOnce(St) -> R,
              R: Stream<Item=Message<M3>>,
    {
        let stream = (map)(self.stream);
        ActorStreamBuilder {
            system: self.system,
            aid: self.aid,
            stream
        }
    }

    pub async fn build<H>(mut self, handler: H) -> TypedAid<M1>
        where H: Handler<M2> + Send + Unpin + 'static,
    {
        self.system.spawn(self.aid.clone().untyped(), self.stream, handler).await;
        self.aid
    }

    pub fn map<F, R>(self, mut map: F) -> ActorStreamBuilder<M1, impl Stream<Item=Message<R>>>
        where F: FnMut(M2) -> R, 
    {
        self.with(move |stream| {
            stream.map(move |msg| {
                match msg {
                    Message::Actual(m) => {
                        let ret = (map)(m);
                        Message::Actual(ret)
                    },
                    Message::System(msg) => Message::System(msg),
                }
            })
        })
    }

    pub fn filter<Fut, F>(self, mut filter: F) -> ActorStreamBuilder<M1, impl Stream<Item=Message<M2>>>
        where F: FnMut(&M2) -> Fut,
              Fut: Future<Output=bool>,
    {
        self.with(move |stream| {
            stream.filter(move |msg| {
                match msg {
                    Message::Actual(m) => {
                        ((filter)(m)).left_future()
                    },
                    _ => ready(true).right_future()
                }
            })
        })
    }

    pub fn filter_map<Fut, T, F>(self, mut f: F) -> ActorStreamBuilder<M1, impl Stream<Item=Message<T>>>
        where F: FnMut(M2) -> Fut,
            Fut: Future<Output = Option<T>>,
            Self: Sized, 
    {
        self.with(move |stream| {
            stream.filter_map(move |msg| {
                match msg {
                    Message::Actual(m) => {
                        let fut = (f)(m);
                        fut.map(|m| m.map(|ret| Message::Actual(ret) )).left_future()
                    },
                    Message::System(msg) => ready(Some(Message::System(msg))).right_future(),
                }
            })
        })
    }

    pub fn then<Fut, F, T>(self, mut f: F) -> ActorStreamBuilder<M1, impl Stream<Item=Message<T>>>
        where F: FnMut(M2) -> Fut,
            Fut: Future<Output=T>,
            Self: Sized, 
    {
        self.with(move |stream| {
            stream.then(move |msg| {
                match msg {
                    Message::Actual(m) => {
                        let fut = (f)(m);
                        fut.map(|m| Message::Actual(m) ).left_future()
                    },
                    Message::System(msg) => ready(Message::System(msg)).right_future(),
                }
            })
        })
    }
}
