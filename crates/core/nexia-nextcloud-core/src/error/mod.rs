//! The error type shared by every SDK crate.

mod decode;
mod sdk;
mod status;

pub use decode::DecodeError;
pub use sdk::Error;
pub use status::{MAX_BODY_LEN, StatusError};
