use std::fmt;
use std::future::Future;
use std::hash;
use std::io;

use futures::channel::mpsc::{self, Receiver};
use futures::channel::oneshot;

use crate::system::ActorSystem;
use super::{ActorStreamBuilder, Fanout, Handler, TypedAid, Process};
use super::fanout::{FanoutHandler, FanoutMessage};
use super::change::Change;
use super::aid::ActorReceiver;

pub struct ActorBuilder {
    pub channel_size: Option<u16>,
    pub name: Option<String>,
    pub system: ActorSystem,
}

impl ActorBuilder {
    pub(crate) fn new(system: ActorSystem) -> ActorBuilder {
        ActorBuilder {
            channel_size: None,
            name: None,
            system,
        }
    }

    pub fn name<S: Into<String>>(self, name: S) -> ActorBuilder {
        ActorBuilder {
            channel_size: self.channel_size,
            name: Some(name.into()),
            system: self.system,
        }
    }

    pub fn channel_size(self, channel_size: u16) -> ActorBuilder {
        ActorBuilder {
            channel_size: Some(channel_size),
            name: self.name,
            system: self.system,
        }
    }

    pub async fn with<H, M>(mut self, handler: H) -> TypedAid<M>
        where M: fmt::Debug + Send + Unpin + 'static,
              H: Handler<M> + Send + Unpin + 'static,
    {
        let channel_size = self.channel_size.unwrap_or_else(|| self.system.config().message_channel_size);
        let (aid, stream) = TypedAid::new(self.name, self.system.uuid(), channel_size);
        self.system.spawn(aid.clone().untyped(), stream, handler).await;
        aid
    }

    pub async fn channel<M>(self, buffer: usize) -> (TypedAid<M>, Receiver<M>)
        where M: fmt::Debug + Send + Unpin + 'static
    {
        let (sender, receiver) = mpsc::channel(buffer);
        (self.with(sender).await, receiver)
    }

    pub async fn fanout<M>(self) -> (TypedAid<M>, Fanout<M>)
        where M: fmt::Debug + Clone + Send + Sync + Unpin + 'static,
    {
        let mut builder2 = self.system.builder();
        if let Some(name) = &self.name {
            builder2 = builder2.name(&format!("{}-redirect", name));
        }
        let aid = self.with(FanoutHandler::default()).await;
        let fanout = Fanout::new(aid.clone());
        let sender = aid.map(|m| FanoutMessage::Message(m));
        let redirect = builder2.with(sender).await;
        ( redirect, fanout )
    }

    pub async fn process<F, I, R, M>(self, recipient: TypedAid<M>, start: F) -> TypedAid<Change<I, ()>>
        where R: Future<Output=io::Result<()>> + Send + 'static,
              M: fmt::Debug + Send + Unpin + 'static,
              F: (FnMut(TypedAid<M>, I, oneshot::Receiver<()>) -> R) + Send + Sync + Unpin + 'static,
              I: Eq + hash::Hash + fmt::Debug + Clone + Send + Sync + Unpin + 'static,
    {
        self.with(Process::new(recipient, start)).await
    }

    pub fn stream<M>(self) -> ActorStreamBuilder<M, ActorReceiver<M>>
        where M: fmt::Debug + Send + 'static
    {
        let channel_size = self.channel_size.unwrap_or_else(|| self.system.config().message_channel_size);
        let (aid, stream) = TypedAid::new(self.name, self.system.uuid(), channel_size);
        ActorStreamBuilder { system: self.system, aid, stream }
    }
    /*

     */
}
