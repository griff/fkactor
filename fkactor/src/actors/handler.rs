use std::sync::Arc;

use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::channel::mpsc::Sender;
use futures::channel::oneshot;


use super::{Context, Status, StatusResult, UntypedAid};
use crate::system::{Message, SystemMsg};

#[async_trait]
#[allow(unused_variables)]
pub trait Handler<T: Send + 'static> {
    async fn handle_message(&mut self, context: &mut Context, msg: T) -> StatusResult;
    async fn handle_start(&mut self, context: &mut Context) -> StatusResult {
        Ok(Status::Done)
    }
    async fn handle_shutdown(&mut self, context: &mut Context) -> StatusResult {
        Ok(Status::Stop)
    }
    async fn handle_stopped(&mut self, context: &mut Context, aid: UntypedAid, error: Option<Arc<String>>) -> StatusResult {
        Ok(Status::Done)
    }
    async fn handle(&mut self, context: &mut Context, item: Message<T>) -> StatusResult
    {
        match item {
            Message::Actual(msg) => self.handle_message(context, msg).await,
            Message::System(SystemMsg::Start) => self.handle_start(context).await,
            Message::System(SystemMsg::Stop) => self.handle_shutdown(context).await,
            Message::System(SystemMsg::Stopped { aid, error}) => {
                self.handle_stopped(context, aid, error).await
            }
        }
    }
}

/*
pub trait Handler<T: Send + 'static> {
    fn handle_message<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context, msg: Box<T>) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
               'ctx: 'async_trait,
    {
    }

    fn handle_start<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
               'ctx: 'async_trait,
    {
    }

    fn handle_shutdown<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
               'ctx: 'async_trait,
    {
    }

    fn handle_stopped<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context, aid: UntypedAid, error: Option<Arc<String>>) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
               'ctx: 'async_trait,
    {
    }

    fn handle<'this, 'ctx, 'async_trait>(&'this mut self, context: &'ctx mut Context, item: Message<T>) -> Pin<Box<dyn Future<Output=StatusResult> + Send + 'async_trait>>
        where 'this: 'async_trait,
               'ctx: 'async_trait,
    {
    }
}
*/


#[async_trait]
impl<M> Handler<M> for Sender<M>
    where M: Send + 'static
{
    async fn handle_message(&mut self, _: &mut Context, msg: M) -> StatusResult
    {
        self.send(msg).await?;
        Ok(Status::Done)
    }
}


#[async_trait]
impl<M> Handler<M> for Option<oneshot::Sender<M>>
    where M: Send + 'static
{
    async fn handle_message(&mut self, _: &mut Context, msg: M) -> StatusResult
    {
        if let Some(sender) = self.take() {
            sender.send(msg).unwrap_or_else(|_| eprintln!("Receiver already closed") );
        }
        Ok(Status::Stop)
    }
}
