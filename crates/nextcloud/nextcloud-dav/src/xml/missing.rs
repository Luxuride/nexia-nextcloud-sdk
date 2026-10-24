//! The error a decode step raises for an element it required.

use std::error::Error;
use std::fmt;

/// An element a decode step required but could not find.
///
/// Carries the qualified name so a [`DecodeError`](nextcloud_sdk_core::DecodeError)
/// wrapping it says *which* element was absent, not just that decoding failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingElement {
    name: String,
}

impl MissingElement {
    /// Records the name that was not found.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }

    /// Qualified name that was not found.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for MissingElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "missing element `{}`", self.name)
    }
}

impl Error for MissingElement {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::MissingElement;

    #[test]
    fn names_the_element_it_reports() {
        assert_eq!(MissingElement::new("d:href").name(), "d:href");
        assert!(MissingElement::new("d:href").to_string().contains("d:href"));
    }

    #[test]
    fn is_a_standard_error_usable_as_a_source() {
        let error: Box<dyn Error + Send + Sync> = Box::new(MissingElement::new("d:multistatus"));
        assert!(error.to_string().contains("d:multistatus"));
    }
}
