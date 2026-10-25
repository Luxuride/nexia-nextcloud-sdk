//! Background worker actor and message protocol for driving the session state machine and cache.

mod actor;
mod task;

pub use actor::{WorkerActor, WorkerStores};
pub use task::WorkerTask;
