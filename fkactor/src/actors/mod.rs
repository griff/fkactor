use thiserror::Error;

use super::StdError;
use super::system::ActorSystem;

mod aid;
mod builder;
mod change;
mod fanout;
mod handler;
mod mapped;
mod process;
mod stream_builder;

pub use self::aid::{Aid, TypedAid, UntypedAid};
pub use self::builder::ActorBuilder;
pub use self::change::{Change, Changed, ChangeDB, ChangeUpdater, EventedDB};
pub use self::fanout::{Fanout, FanoutAid, FanoutMessage};
pub use self::handler::Handler;
pub use self::process::Process;
pub use self::stream_builder::ActorStreamBuilder;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AidError {
    #[error("Actor already stopped")]
    ActorAlreadyStopped,
    #[error("Send timeout for {0}")]
    SendTimedOut(Aid),
}

#[derive(Clone)]
pub struct Context {
    pub aid: UntypedAid,
    pub system: ActorSystem,
}

pub enum Status {
    Done,
    Stop,
}
pub type StatusResult = Result<Status, StdError>;


