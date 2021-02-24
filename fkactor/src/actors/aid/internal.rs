use std::any::TypeId;
use std::fmt;
use std::task::{self, Poll};
use std::pin::Pin;

use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::stream::Stream;
use futures::channel::mpsc::{self, Receiver, Sender};

use crate::actors::AidError;
use crate::system::{Message, SystemMsg};


pub(crate) fn channel<M: 'static>(channel_size: u16) -> (ActorSender<M>, ActorReceiver<M>) {
    let (sender, r) = mpsc::channel(channel_size as usize);
    (ActorSender { sender },
     ActorReceiver { stream: Some(r) })
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
impl<M> SystemSender for Sender<Message<M>>
    where M: fmt::Debug + Send + 'static
{
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let Err(err) = self.send(Message::System(msg)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
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
    sender: Sender<Message<M>>,
}

/*
impl<M> fmt::Debug for ActorSender<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        //fmt::Debug::fmt(&self.sender, f)
        write!(f, "Muh!")
    }
}
*/

impl<M> Clone for ActorSender<M> {
    fn clone(&self) -> Self {
        ActorSender {
            sender: self.sender.clone(),
        }
    }
}

#[async_trait]
impl<M> MessageSender<M> for ActorSender<M>
    where M: fmt::Debug + Send + 'static,
{
    async fn send_msg(&mut self, msg: M) -> Result<(), AidError> {
        if let Err(err) = self.sender.send(Message::Actual(msg)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    } 
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let Err(err) = self.sender.send(Message::System(msg)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    }
    fn stop(&mut self) {
        self.sender.close_channel()
    }
    fn __private_clone_box__(&self) -> Box<dyn MessageSender<M> + Sync> {
        Box::new(self.clone())
    }
    fn __private_untyped__(&self) -> UntypedActorSender {
        UntypedActorSender {
            type_id: TypeId::of::<M>(),
            sender: Box::new(self.sender.clone()),
        }
    }
}
/*
impl<M> ActorSender<M>
    where M: Send + 'static
{
    pub async fn send_new(&mut self, msg: M) -> Result<(), AidError> {
        self.send(Box::new(msg)).await
    }
    pub async fn send(&mut self, msg: Box<M>) -> Result<(), AidError> {
        if let Err(err) = self.untyped.sender.send(Message::Actual(msg as Box<dyn Any + Send>)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    } 

    pub async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        if let Err(err) = self.untyped.sender.send(Message::System(msg)).await {
            if err.is_disconnected() {
                return Err(AidError::ActorAlreadyStopped);
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.untyped.sender.close_channel()
    }
}
*/

pub struct ActorReceiver<M> {
    stream: Option<Receiver<Message<M>>>,
}

impl<M> Stream for ActorReceiver<M>
    where M: Unpin + 'static
{
    type Item = Message<M>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>>
    {
        match self.stream.take() {
            Some(mut stream) => {
                match Pin::new(&mut stream).poll_next(cx) {
                    msg @ Poll::Ready(Some(Message::System(SystemMsg::Stop))) => {
                        msg
                    },
                    other => {
                        self.stream = Some(stream);
                        other
                    },
                }
            },
            None => Poll::Ready(None)
        }
    }
}
