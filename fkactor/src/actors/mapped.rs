use std::fmt;
use std::marker::PhantomData;

use async_trait::async_trait;

use super::{AidError};
use crate::system::SystemMsg;
use crate::actors::aid::internal::{MessageSender, UntypedActorSender};

pub(crate) struct MappedSender<F,M1,M2> {
    pub(crate) map: F,
    pub(crate) sender: Box<dyn MessageSender<M2> + Sync>,
    pub(crate) phantom: PhantomData<fn() -> M1>,
}

impl<F,M1,M2> Clone for MappedSender<F,M1,M2>
    where F: Clone
{
    fn clone(&self) -> Self {
        MappedSender {
            map: self.map.clone(),
            sender: self.sender.clone(),
            phantom: PhantomData,
        }
    }
}

impl<F,M1,M2> fmt::Debug for MappedSender<F,M1,M2> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.sender, f)
    }
}


#[async_trait]
impl<F,M1,M2> MessageSender<M1> for MappedSender<F,M1,M2>
    where M1: Send + 'static,
          M2: Send + 'static,
           F: Fn(M1) -> M2 + Clone + Sync + Send + 'static,
{
    async fn send_msg(&mut self, msg: M1) -> Result<(), AidError> {
        let r = (self.map)(msg);
        self.sender.send_msg(r).await
    } 
    async fn send_system(&mut self, msg: SystemMsg) -> Result<(), AidError> {
        self.sender.send_system(msg).await
    }
    fn stop(&mut self) {
        self.sender.stop()
    }
    fn __private_clone_box__(&self) -> Box<dyn MessageSender<M1> + Sync> {
        Box::new(self.clone())
    }
    fn __private_untyped__(&self) -> UntypedActorSender {
        self.sender.__private_untyped__()
    }
}