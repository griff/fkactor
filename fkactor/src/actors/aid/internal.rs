use std::any::TypeId;
use std::fmt;
use std::task::{self, Poll};
use std::pin::Pin;

use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::stream::FusedStream;
use futures::stream::Stream;
use futures::channel::mpsc::{self, Receiver, Sender};
use log::trace;

use crate::actors::AidError;
use crate::system::{Message, SystemMsg};

use super::Aid;


pub(crate) fn channel<M: 'static>(channel_size: u16, aid: Aid) -> (ActorSender<M>, ActorReceiver<M>) {
    let (system, rs) = mpsc::channel(channel_size as usize);
    let (sender, r) = mpsc::channel(channel_size as usize);
    (ActorSender { aid: aid.clone(), system, sender },
     ActorReceiver { aid: aid.clone(), stream: Some(r), system: Some(rs), ready: None, completed: false })
}

#[async_trait]
pub trait SystemSender: fmt::Debug + Send
{
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError>;
    fn stop(&mut self);

    #[doc(hidden)]
    fn __private_clone_box__(&self) -> Box<dyn SystemSender + Sync>;

}

#[async_trait]
impl SystemSender for Sender<SystemMsg>
{
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let SystemMsg::Stop = msg {
            self.stop();
        } else {
            if let Err(err) = self.send(msg).await {
                if err.is_disconnected() {
                    return Err(AidError::ActorAlreadyStopped);
                }
            }
        }
        Ok(())
    }

    fn __private_clone_box__(&self) -> Box<dyn SystemSender + Sync> {
        Box::new(self.clone())
    }

    fn stop(&mut self) {
        self.close_channel()
    }
}
impl Clone for Box<dyn SystemSender + Sync>
{
    fn clone(&self) -> Self {
        self.__private_clone_box__()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct UntypedActorSender {
    type_id: TypeId,
    sender: Box<dyn SystemSender + Sync>,
}

impl UntypedActorSender {
    pub(crate) async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        self.sender.send_system(msg).await
    }

    pub(crate) fn stop(&mut self) {
        self.sender.stop()
    }
}

/*
#[async_trait]
impl SystemSender for UntypedActorSender
{
    async fn start(&mut self) -> Result<(), AidError> {
        self.send_system(SystemMsg::Start).await
    }

    fn stop(&mut self) {
        self.sender.__private_stop__()
    }

    async fn stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Result<(), AidError>
    {
        self.send_system(SystemMsg::Stopped{aid, error}).await
    }

    async fn shutdown(&mut self) {
        if let Err(err) = self.send_system(SystemMsg::Stop).await {
            self.stop();
            if err != AidError::ActorAlreadyStopped {
                panic!("An error other than disconnect occured: {:?}", err);
            }
        } else {
            self.stop();
        }
    }
}
*/

/*
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct UntypedActorSender {
    type_id: TypeId,
    sender: Sender<InternalMessage>,
}

impl UntypedActorSender
{
    pub async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let Err(err) = self.sender.send(Message::System(msg)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.sender.close_channel()
    }
}


#[async_trait]
impl SystemSender for UntypedActorSender
{
    async fn start(&mut self) -> Result<(), AidError> {
        self.send_system(SystemMsg::Start).await
    }

    fn stop(&mut self) {
        self.sender.close_channel()
    }

    async fn stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Result<(), AidError>
    {
        self.send_system(SystemMsg::Stopped{aid, error}).await
    }

    async fn shutdown(&mut self) {
        if let Err(err) = self.send_system(SystemMsg::Stop).await {
            self.stop();
            if err != AidError::ActorAlreadyStopped {
                panic!("An error other than disconnect occured: {:?}", err);
            }
        } else {
            self.stop();
        }
    }
}
*/

#[async_trait]
pub trait MessageSender<M>: fmt::Debug + Send
{
    async fn send_msg(&mut self, msg: M) -> Result<(), AidError>;
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError>;
    fn stop(&mut self);

    #[doc(hidden)]
    fn __private_clone_box__(&self) -> Box<dyn MessageSender<M> + Sync>;

    #[doc(hidden)]
    fn __private_untyped__(&self) -> UntypedActorSender;
}

impl<M> Clone for Box<dyn MessageSender<M> + Sync>
{
    fn clone(&self) -> Self {
        self.__private_clone_box__()
    }
}



#[derive(Debug)]
pub struct ActorSender<M> {
    aid: Aid,
    system: Sender<SystemMsg>,
    sender: Sender<M>,
}

impl<M> Clone for ActorSender<M> {
    fn clone(&self) -> Self {
        ActorSender {
            aid: self.aid.clone(),
            system: self.system.clone(),
            sender: self.sender.clone(),
        }
    }
}

#[async_trait]
impl<M> MessageSender<M> for ActorSender<M>
    where M: fmt::Debug + Send + 'static,
{
    async fn send_msg(&mut self, msg: M) -> Result<(), AidError> {
        if let Err(err) = self.sender.send(msg).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    } 
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let SystemMsg::Stop = msg {
            self.stop();
        } else {
            if let Err(err) = self.system.send(msg).await {
                trace!("send_system {}: {:?}", self.aid, err.to_string());
                if err.is_disconnected() {
                    return Err(AidError::ActorAlreadyStopped);
                }
            }
        }
        Ok(())
    }
    fn stop(&mut self) {
        self.system.close_channel();
        self.sender.close_channel();
    }
    fn __private_clone_box__(&self) -> Box<dyn MessageSender<M> + Sync> {
        Box::new(self.clone())
    }
    fn __private_untyped__(&self) -> UntypedActorSender {
        UntypedActorSender {
            type_id: TypeId::of::<M>(),
            sender: Box::new(self.system.clone()),
        }
    }
}

pub struct ActorReceiver<M> {
    aid: Aid,
    stream: Option<Receiver<M>>,
    system: Option<Receiver<SystemMsg>>,
    ready: Option<Message<M>>,
    completed: bool,
}

impl<M> Stream for ActorReceiver<M>
    where M: Unpin + 'static
{
    type Item = Message<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>>
    {
        loop {
            if let Some (msg) = self.ready.take() {
                return Poll::Ready(Some(msg));
            }
            match (self.stream.take(), self.system.take()) {
                (Some(mut stream), Some(mut system)) => {
                    match (Pin::new(&mut stream).poll_next(cx), Pin::new(&mut system).poll_next(cx)) {
                        (Poll::Ready(None), Poll::Ready(None)) => {
                            trace!("{} Both none {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            return Poll::Ready(Some(Message::System(SystemMsg::Stop)));
                        },
                        (Poll::Ready(None), Poll::Ready(Some(msg))) => {
                            system.close();
                            trace!("{} Stream none, system msg {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.system = Some(system);
                            return Poll::Ready(Some(Message::System(msg)));
                        },
                        (Poll::Ready(None), Poll::Pending) => {
                            system.close();
                            trace!("{} Stream none, system pending {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.system = Some(system);
                        },
                        (Poll::Ready(Some(item)), Poll::Ready(None)) => {
                            stream.close();
                            trace!("{} Stream some, System none {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.stream = Some(stream);
                            return Poll::Ready(Some(Message::Actual(item)));
                        },
                        (Poll::Ready(Some(item)), Poll::Ready(Some(msg))) => {
                            trace!("{} Stream some, System some {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.stream = Some(stream);
                            self.system = Some(system);
                            self.ready = Some(Message::System(msg));
                            return Poll::Ready(Some(Message::Actual(item)));
                        },
                        (Poll::Ready(Some(item)), Poll::Pending) => {
                            trace!("{} Stream some, System pending {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.stream = Some(stream);
                            self.system = Some(system);
                            return Poll::Ready(Some(Message::Actual(item)));
                        },
                        (Poll::Pending, Poll::Ready(None)) => {
                            stream.close();
                            trace!("{} Stream pending, System none {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.stream = Some(stream);
                        },
                        (Poll::Pending, Poll::Ready(Some(msg))) => {
                            trace!("{} Stream pending, System some {} {} {:?}", self.aid, system.is_terminated(), stream.is_terminated(), msg);
                            self.stream = Some(stream);
                            self.system = Some(system);
                            return Poll::Ready(Some(Message::System(msg)));
                        },
                        (Poll::Pending, Poll::Pending) => {
                            trace!("{} Both pending {} {}", self.aid, system.is_terminated(), stream.is_terminated());
                            self.system = Some(system);
                            self.stream = Some(stream);
                            return Poll::Pending;
                        },
                    }
                },
                (Some(mut stream), None) => {
                    match Pin::new(&mut stream).poll_next(cx) {
                        Poll::Pending => {
                            trace!("{} Stream pending, system done", self.aid);
                            self.stream = Some(stream);
                            return Poll::Pending;
                        },
                        Poll::Ready(Some(msg)) => {
                            trace!("{} Stream some, system done", self.aid);
                            self.stream = Some(stream);
                            return Poll::Ready(Some(Message::Actual(msg)));
                        },
                        Poll::Ready(None) => {
                            trace!("{} Stream none, system done", self.aid);
                            return Poll::Ready(Some(Message::System(SystemMsg::Stop)));
                        }
                    }
                },
                (None, Some(mut system)) => {
                    match Pin::new(&mut system).poll_next(cx) {
                        Poll::Pending => {
                            trace!("{} Stream done, system pending", self.aid);
                            self.system = Some(system);
                            return Poll::Pending;
                        },
                        Poll::Ready(Some(msg)) => {
                            trace!("{} Stream done, system some", self.aid);
                            self.system = Some(system);
                            return Poll::Ready(Some(Message::System(msg)));
                        },
                        Poll::Ready(None) => {
                            trace!("{} Stream done, system none", self.aid);
                            return Poll::Ready(Some(Message::System(SystemMsg::Stop)));
                        }
                    }
                },
                (None, None) => {
                    trace!("{} No stream", self.aid);
                    self.completed = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<M> FusedStream for ActorReceiver<M>
    where M: Unpin + 'static
{
    fn is_terminated(&self) -> bool {
        self.completed && self.stream.is_none() && self.system.is_none()
    }
}