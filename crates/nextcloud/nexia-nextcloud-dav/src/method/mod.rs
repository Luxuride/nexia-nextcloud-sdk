//! Request metadata that is not part of the request body.

mod depth;
mod destination;
mod overwrite;

pub use depth::{DEPTH_HEADER, Depth};
pub use destination::DESTINATION_HEADER;
pub use overwrite::{OVERWRITE_HEADER, Overwrite};
