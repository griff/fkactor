use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use futures::stream::Stream;
use futures::lock::Mutex;
use log::error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::actors::{ActorBuilder, Context, Handler, TypedAid, UntypedAid};
use super::executor::{ActorManager, Executor, SystemManage};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemMsg {
    Start,
    Stopped { aid: UntypedAid, error: Option<Arc<String>> },
    Stop,
}

#[derive(Debug, Clone)]
pub enum Message<M> {
    System(SystemMsg),
    Actual(M),
}

impl<M> Message<M>
{
    pub fn map<F, R>(self, map: F) -> Message<R>
        where F: FnOnce(M) -> R,
    {
        match self {
            Message::System(m) => Message::System(m),
            Message::Actual(msg) => Message::Actual((map)(msg))
        }
    }
}

impl<M> PartialEq for Message<M>
    where M: PartialEq<M>,
{
    fn eq(&self, other: &Self) -> bool
    {
        match (self, other) {
            (Message::System(msg1), Message::System(msg2)) => msg1 == msg2,
            (Message::Actual(msg1), Message::Actual(msg2)) => msg1 == msg2,
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool
    {
        match (self, other) {
            (Message::System(msg1), Message::System(msg2)) => msg1 != msg2,
            (Message::Actual(msg1), Message::Actual(msg2)) => msg1 != msg2,
            _ => true
        }
    }
}

impl<M: Eq> Eq for Message<M> {}

impl<M> PartialOrd for Message<M>
    where M: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering>
    {
        match (self, other) {
            (Message::System(SystemMsg::Start), Message::Actual(_)) => Some(Ordering::Less),
            (Message::Actual(_), Message::System(SystemMsg::Start)) => Some(Ordering::Greater),
            (Message::System(msg1), Message::System(msg2)) => msg1.partial_cmp(msg2),
            (Message::Actual(msg1), Message::Actual(msg2)) => msg1.partial_cmp(msg2),
            (Message::System(_), Message::Actual(_)) => Some(Ordering::Greater),
            (Message::Actual(_), Message::System(_)) => Some(Ordering::Less),
        }
    }
}
impl<M> Ord for Message<M>
    where M: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering
    {
        match (self, other) {
            (Message::System(SystemMsg::Start), Message::Actual(_)) => Ordering::Less,
            (Message::Actual(_), Message::System(SystemMsg::Start)) => Ordering::Greater,
            (Message::System(msg1), Message::System(msg2)) => msg1.cmp(msg2),
            (Message::Actual(msg1), Message::Actual(msg2)) => msg1.cmp(msg2),
            (Message::System(_), Message::Actual(_)) => Ordering::Greater,
            (Message::Actual(_), Message::System(_)) => Ordering::Less,
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActorSystemConfig {
    pub message_channel_size: u16,
    //pub send_timeout: Duration,
    //pub thread_pool_size: u16,
    //pub warn_threshold: Duration,
    //pub time_slice: Duration,
    //pub thread_wait_time: Duration,
    pub start_on_launch: bool,
}

impl ActorSystemConfig {
    /// Return a new config with the changed `message_channel_size`.
    pub fn message_channel_size(mut self, value: u16) -> Self {
        self.message_channel_size = value;
        self
    }
    /*
    /// Return a new config with the changed `send_timeout`.
    pub fn send_timeout(mut self, value: Duration) -> Self {
        self.send_timeout = value;
        self
    }

    /// Return a new config with the changed `thread_pool_size`.
    pub fn thread_pool_size(mut self, value: u16) -> Self {
        self.thread_pool_size = value;
        self
    }

    /// Return a new config with the changed `warn_threshold`.
    pub fn warn_threshold(mut self, value: Duration) -> Self {
        self.warn_threshold = value;
        self
    }

    /// Return a new config with the changed `time_slice`.
    pub fn time_slice(mut self, value: Duration) -> Self {
        self.time_slice = value;
        self
    }

    /// Return a new config with the changed `thread_wait_time`.
    pub fn thread_wait_time(mut self, value: Duration) -> Self {
        self.thread_wait_time = value;
        self
    }
    */
}

impl Default for ActorSystemConfig {
    /// Create the config with the default values.
    fn default() -> ActorSystemConfig {
        ActorSystemConfig {
            //thread_pool_size: (num_cpus::get() * 4) as u16,
            //warn_threshold: Duration::from_millis(1),
            //time_slice: Duration::from_millis(1),
            //thread_wait_time: Duration::from_millis(100),
            message_channel_size: 32,
            //send_timeout: Duration::from_millis(1),
            start_on_launch: true,
        }
    }
}

struct SystemData {
    uuid: Uuid,
    config: ActorSystemConfig,
    actors: Arc<Mutex<HashMap<UntypedAid, UntypedAid>>>,
}

#[derive(Clone)]
pub struct ActorSystem {
    data: Arc<SystemData>,
    monitor: TypedAid<SystemManage>,
    executor: Executor,
}

impl ActorSystem {
    pub fn create(config: ActorSystemConfig) -> ActorSystem {
        let uuid = Uuid::new_v4();
        let (monitor, stream) = TypedAid::new(Some("System".into()), uuid.clone(), 500);
        let mut system = ActorSystem {
            data: Arc::new(SystemData {
                actors: Default::default(),
                uuid, config,
            }),
            executor: Default::default(),
            monitor: monitor.clone(),
        };
        let handler = ActorManager::new(system.data.actors.clone());
        let ctx = Context {
            aid: monitor.untyped(),
            system: system.clone(),
        };
        system.executor.run_sync(ctx, stream, handler);
        if system.config().start_on_launch {
            system.executor.start_sync();
        }
        system
    }

    pub async fn start(&mut self) {
        self.executor.start().await
    }

    pub async fn shutdown(&mut self) {
        let actors = self.data.actors.lock().await.clone();
        for (mut aid, _) in actors.into_iter() {
            aid.stop();
        }        
    }

    pub fn builder(&self) -> ActorBuilder {
        ActorBuilder::new(self.clone())
    }

    pub(crate) async fn notify_started(&mut self, aid: UntypedAid) {
        if aid == self.monitor {
            return;
        }
        self.monitor.send(SystemManage::Started(aid.clone(), aid.clone())).await.unwrap_or_else(|err| {
            error!("Failed to notify started status {:?}", err);
        });
    }

    pub(crate) async fn notify_stopped(&mut self, aid: UntypedAid, error: Option<Arc<String>>) {
        if aid == self.monitor {
            return;
        }
        self.monitor.stopped(aid, error).await.unwrap_or_else(|err| {
            error!("Failed to notify stopped status {:?}", err);
        });
    }

    pub(crate) async fn spawn<M, St, H>(&mut self, mut aid: UntypedAid, stream: St, handler: H)
        where St: Stream<Item=Message<M>> + Send + 'static,
               H: Handler<M> + Send + Unpin + 'static,
               M: Send + 'static,
    {
        aid.start().await.unwrap_or_else(|err| {
            error!("Failed to send start message {:?}", err);
        });
        let ctx = Context {
            system: self.clone(),
            aid,
        };
        self.executor.run(ctx, stream, handler).await
    }

    pub fn config(&self) -> &ActorSystemConfig {
        &self.data.config
    }

    pub fn uuid(&self) -> Uuid {
        self.data.uuid.clone()
    }

    pub async fn monitor<M1, M2>(&mut self, monitoring: M1, monitored: M2)
        where M1: Into<UntypedAid>,
              M2: Into<UntypedAid>,
    {
        self.monitor.send(SystemManage::RegisterMonitor(monitoring.into(), monitored.into())).await.unwrap_or_else(|err| {
            error!("Could not monitor Aid {:?}", err);
        });
    }
}

impl fmt::Debug for ActorSystem {
    fn fmt(&self, formatter: &'_ mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "ActorSystem{{uuid: {}, config: {:?}}}",
            self.data.uuid.to_string(),
            self.data.config,
        )
    }
}