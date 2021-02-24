
// Allows to use macros from fkactor_derive in this crate
extern crate self as fkactor;
pub use fkactor_derive::Lens;

pub mod actors;
pub mod executor;
pub mod lens;
pub mod system;

pub use lens::{AsyncLens, AsyncLensExt, Lens, LensExt};

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
