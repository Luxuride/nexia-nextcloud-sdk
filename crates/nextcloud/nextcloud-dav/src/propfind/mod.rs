//! Which properties to ask for, and how the question is phrased.

mod property;
mod request_body;

pub use property::{DavNamespace, DavProperty};
pub use request_body::PropfindBody;
