//! Hand-written XML support for the small subset WebDAV needs.
//!
//! Split by concern: [`escape`] handles character data, [`writer`] assembles
//! elements, and [`response`] locates them and exposes the lookups the
//! decoders call. [`Element`] is a located element and [`MissingElement`] the
//! error raised when a required one is absent.
//!
//! There is no general XML parser here; the limits of the reader are spelled
//! out in [`response`].

mod element;
mod missing;
mod scanner;

pub mod escape;
pub mod response;
pub mod writer;

pub use element::Element;
pub use missing::MissingElement;
pub use response::{elements, first_element, has_element, require_element, text_of};
pub use writer::XmlWriter;
