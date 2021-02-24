use std::borrow::Borrow;
use std::cmp;
use std::fmt;
use std::hash;
use std::sync::Arc;

use super::{Aid, TypedAid};
use super::internal::UntypedActorSender;
use crate::actors::AidError;
use crate::system::SystemMsg;


#[derive(Debug, Clone)]
pub struct UntypedAid {
    pub(crate) aid: Aid,
    pub(crate) inner: UntypedActorSender,
}

impl<M> From<TypedAid<M>> for UntypedAid
    where M: fmt::Debug + Send + 'static
{
    fn from(v: TypedAid<M>) -> Self {
        v.untyped()
    }
}

impl cmp::PartialEq for UntypedAid {
    fn eq(&self, other: &Self) -> bool {
        self.aid == other.aid
    }
}

impl<M> cmp::PartialEq<TypedAid<M>> for UntypedAid {
    fn eq(&self, other: &TypedAid<M>) -> bool {
        self.aid == other.aid
    }
}

impl cmp::Eq for UntypedAid {}

impl cmp::PartialOrd for UntypedAid {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.aid.partial_cmp(&other.aid)
    }
}

impl cmp::Ord for UntypedAid {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.aid.cmp(&other.aid)
    }
}

impl hash::Hash for UntypedAid {
    fn hash<H: hash::Hasher>(&self, state: &'_ mut H) {
        self.aid.hash(state)
    }
}

impl Borrow<Aid> for UntypedAid {
    fn borrow(&self) -> &Aid {
        &self.aid
    }
}

impl AsRef<Aid> for UntypedAid {
    fn as_ref(&self) -> &Aid {
        &self.aid
    }
}

impl UntypedAid {
    pub fn stop(&mut self) {
        self.inner.stop()
    }

    pub(crate) async fn stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Result<(), AidError>
    {
        self.inner.send_system(SystemMsg::Stopped{aid, error}).await
    }

    pub(crate) async fn start(&mut self) -> Result<(), AidError>
    {
        self.inner.send_system(SystemMsg::Start).await
    }
}

/*
impl UntypedAid {
    pub fn downcast<M: Send + 'static>(&self) -> Option<TypedAid<M>> {
        if TypeId::of::<M>() == self.inner.type_id {
            Some(TypedAid {
                aid: self.aid.clone(),
                sender: Box::new(ActorSender { untyped: self.inner.clone(), phantom: PhantomData }),
            })
        } else {
            None
        }
    }
}
*/
/*
impl SystemSender for UntypedAid {
    fn start<'this, 'async_trait>(&'this mut self) -> Pin<Box<dyn Future<Output=Result<(), AidError>> + Send + 'async_trait >>
        where 'this: 'async_trait
    {
        self.inner.start()
    }
    fn stop(&mut self) {
        self.inner.stop()
    }
    fn stopped<'this, 'async_trait>(&'this mut self, aid: UntypedAid, error: Option<Arc<String>>) -> Pin<Box<dyn Future<Output=Result<(), AidError>> + Send + 'async_trait>>
        where 'this: 'async_trait
    {
        self.inner.stopped(aid, error)
    }
    fn shutdown<'this, 'async_trait>(&'this mut self) -> Pin<Box<dyn Future<Output=()> + Send + 'async_trait>>
        where 'this: 'async_trait
    {
        self.inner.shutdown()
    }
}
*/
