use std::borrow::Borrow;
use std::cmp;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use super::{Aid, UntypedAid};
use super::internal::{channel, ActorReceiver, MessageSender};
use crate::actors::{AidError, Context, Handler, Status, StatusResult};
use crate::system::SystemMsg;
use crate::actors::mapped::MappedSender;


#[derive(Debug)]
pub struct TypedAid<M> {
    pub(crate) aid: Aid,
    sender: Box<dyn MessageSender<M> + Sync>
}

impl<M> Clone for TypedAid<M>
{
    fn clone(&self) -> Self {
        TypedAid {
            aid: self.aid.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<M> cmp::PartialEq for TypedAid<M> {
    fn eq(&self, other: &Self) -> bool {
        self.aid == other.aid
    }
}
impl<M> cmp::PartialEq<UntypedAid> for TypedAid<M> {
    fn eq(&self, other: &UntypedAid) -> bool {
        self.aid == other.aid
    }
}

impl<M> cmp::Eq for TypedAid<M> {}

impl<M> cmp::PartialOrd for TypedAid<M> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.aid.partial_cmp(&other.aid)
    }
}
impl<M> cmp::Ord for TypedAid<M> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.aid.cmp(&other.aid)
    }
}

impl<M> hash::Hash for TypedAid<M> {
    fn hash<H: hash::Hasher>(&self, state: &'_ mut H) {
        self.aid.hash(state)
    }
}

impl<M> Borrow<Aid> for TypedAid<M> {
    fn borrow(&self) -> &Aid {
        &self.aid
    }
}

impl<M> AsRef<Aid> for TypedAid<M> {
    fn as_ref(&self) -> &Aid {
        &self.aid
    }
}

impl<M> TypedAid<M>
    where M: fmt::Debug + Send + 'static
{
    pub(crate) fn new(name: Option<String>, system_uuid: Uuid, channel_size: u16) -> (TypedAid<M>, ActorReceiver<M>)
    {
        let (sender, r) = channel(channel_size);
        let aid = Aid::new(name, system_uuid);

        (TypedAid { aid, sender: Box::new(sender) }, r)
    }

    pub fn untyped(self) -> UntypedAid {
        UntypedAid {
            aid: self.aid,
            inner: self.sender.__private_untyped__(),
        }
    }

    pub async fn send<M1>(&mut self, msg: M1) -> Result<(), AidError>
        where M1: Into<M>
    {
        self.sender.send_msg(msg.into()).await
    }

    pub fn stop(&mut self) {
        self.sender.stop();
    }

    pub(crate) async fn stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Result<(), AidError>
    {
        self.sender.send_system(SystemMsg::Stopped{aid, error}).await
    }
    /*
    async fn shutdown(&mut self) {
        if let Err(err) = self.sender.send_system(SystemMsg::Stop).await {
            self.stop();
            if err != AidError::ActorAlreadyStopped {
                panic!("An error other than disconnect occured: {:?}", err);
            }
        } else {
            self.stop();
        }
    }
    */

    pub fn map<F, M1>(self, f: F) -> TypedAid<M1>
        where F: Fn(M1) -> M + Clone + Send + Sync + 'static,
              M1: Send + 'static,
    {
        let sender = MappedSender {
            map: f,
            sender: self.sender,
            phantom: PhantomData,
        };
        TypedAid {
            aid: self.aid,
            sender: Box::new(sender),
        }
    }

    pub fn from<M1>(self) -> TypedAid<M1>
        where M: From<M1>,
              M1: Send + 'static,
    {
        self.map(From::from)
    }
}

impl<M> TypedAid<Box<M>>
    where M: fmt::Debug + Send + 'static
{
    pub async fn send_new<I>(&mut self, msg: I) -> Result<(), AidError>
        where I: Into<Box<M>>,
    {
        self.sender.send_msg(msg.into()).await
    }
}

/*
#[async_trait]
impl<M> SystemSender for TypedAid<M>
    where M: Send + 'static
{
    async fn start(&mut self) -> Result<(), AidError> {
        self.sender.send_system(SystemMsg::Start).await
    }

    fn stop(&mut self) {
        self.sender.__private_stop__();
    }

    async fn stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Result<(), AidError>
    {
        self.sender.send_system(SystemMsg::Stopped{aid, error}).await
    }

    async fn shutdown(&mut self) {
        if let Err(err) = self.sender.send_system(SystemMsg::Stop).await {
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
impl<M1, M2> Handler<M2> for TypedAid<M1>
    where M1: fmt::Debug + Clone + Send + 'static,
          M2: fmt::Debug + Clone + Send + 'static,
          M2: Into<M1> + Send + 'static,
{
    async fn handle_message(&mut self, _context: &mut Context, msg: M2) -> StatusResult {
        self.send(msg.into()).await?;
        Ok(Status::Done)
    }

    async fn handle_start(&mut self, context: &mut Context) -> StatusResult {
        let monitoring = context.aid.clone();
        let monitored = self.clone().untyped();
        context.system.monitor(monitoring, monitored).await;
        Ok(Status::Done)
    }

    async fn handle_shutdown(&mut self, _context: &mut Context) -> StatusResult {
        self.stop();
        Ok(Status::Stop)
    }

    async fn handle_stopped(&mut self, _context: &mut Context, _aid: UntypedAid, _error: Option<Arc<String>>) -> StatusResult {
        Ok(Status::Stop)
    }
}

