
pub mod actors;
pub mod executor;
pub mod system;

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
