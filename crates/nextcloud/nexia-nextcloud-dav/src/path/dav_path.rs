//! A path below one user's file tree.

use std::fmt;

use nexia_nextcloud_core::url::{percent_encode, percent_encode_path};
use nexia_nextcloud_core::{ApiPath, UrlError};

use super::validation;

/// Where a resource lives in Nextcloud's WebDAV tree.
///
/// The user name is held separately from the path below it because every
/// endpoint in this crate needs the same `files/{user}` prefix and nothing
/// else may appear between them. Validation happens once, in
/// [`DavPath::new`], so a later call cannot send a traversal segment or an
/// already-prefixed path.
///
/// The path is stored decoded and escaped for the request target: [`Self::relative`] and
/// [`Self::segments`] return the name the user sees, while [`Self::as_str`] returns what goes on the
/// wire. Sending the decoded form instead made every name with a space, a `%` or a non-ASCII letter
/// fail, and let a pre-encoded `%2F` arrive as a path separator instead of a character.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DavPath {
    user: String,
    relative: String,
    api: ApiPath,
}

impl DavPath {
    /// The root of `user`'s files.
    pub fn root(user: impl AsRef<str>) -> Result<Self, UrlError> {
        Self::new(user, "")
    }

    /// The resource at `relative` below `user`'s root.
    ///
    /// `relative` is expected to be relative: `""` is the root itself,
    /// `"Photos/img.jpg"` a nested file. Traversal segments, empty segments
    /// and a leading `/` are rejected with a [`UrlError`].
    pub fn new(user: impl AsRef<str>, relative: impl AsRef<str>) -> Result<Self, UrlError> {
        let user = validation::validate_user(user.as_ref())?.to_owned();
        let relative = validation::validate_relative(relative.as_ref())?;
        let wire_user = percent_encode(&user);
        let wire = if relative.is_empty() {
            format!("files/{wire_user}")
        } else {
            format!("files/{wire_user}/{}", percent_encode_path(&relative))
        };
        Ok(Self {
            api: ApiPath::dav(wire)?,
            user,
            relative,
        })
    }

    /// User whose files this path addresses.
    pub fn user(&self) -> &str {
        &self.user
    }

    /// Path below the user's root, without a leading or trailing slash.
    pub fn relative(&self) -> &str {
        &self.relative
    }

    /// Whether this path is the user's root.
    pub fn is_root(&self) -> bool {
        self.relative.is_empty()
    }

    /// Segments of the path below the user root, empty for the root itself.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.relative
            .split('/')
            .filter(|segment| !segment.is_empty())
    }

    /// A child of this path, validated like a fresh path.
    pub fn join(&self, segment: impl AsRef<str>) -> Result<Self, UrlError> {
        let segment = segment.as_ref();
        let combined = if self.relative.is_empty() {
            segment.to_owned()
        } else {
            format!("{}/{segment}", self.relative)
        };
        Self::new(&self.user, combined)
    }

    /// Full path including the WebDAV root and the `files` prefix.
    pub fn as_str(&self) -> &str {
        self.api.as_str()
    }

    /// The path as a request target.
    pub fn to_api_path(&self) -> ApiPath {
        self.api.clone()
    }
}

impl fmt::Display for DavPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::DavPath;

    #[test]
    fn root_covers_the_user_tree() {
        let path = DavPath::root("alice").unwrap();
        assert!(path.is_root());
        assert_eq!(path.as_str(), "/remote.php/dav/files/alice");
    }

    #[test]
    fn nested_paths_keep_their_segments() {
        let path = DavPath::new("alice", "Photos/2024/img.jpg").unwrap();
        assert_eq!(
            path.as_str(),
            "/remote.php/dav/files/alice/Photos/2024/img.jpg"
        );
        assert_eq!(path.segments().count(), 3);
        assert!(!path.is_root());
    }

    #[test]
    fn api_path_matches_the_rendered_string() {
        let path = DavPath::new("alice", "Photos").unwrap();
        assert_eq!(path.to_api_path().as_str(), path.as_str());
    }

    #[test]
    fn rejects_traversal_in_either_part() {
        assert!(DavPath::new("alice", "../etc/passwd").is_err());
        assert!(DavPath::new("alice", "Photos/../../etc").is_err());
        assert!(DavPath::new("..", "Photos").is_err());
    }

    #[test]
    fn rejects_an_absolute_relative_path() {
        assert!(DavPath::new("alice", "/files/alice").is_err());
    }

    #[test]
    fn join_appends_one_segment() {
        let parent = DavPath::new("alice", "Photos").unwrap();
        assert_eq!(parent.join("img.jpg").unwrap().relative(), "Photos/img.jpg");
        assert!(parent.join("../secret").is_err());
        assert_eq!(
            DavPath::root("alice")
                .unwrap()
                .join("a")
                .unwrap()
                .relative(),
            "a"
        );
    }

    #[test]
    fn display_renders_the_wire_path() {
        let path = DavPath::new("alice", "a.txt").unwrap();
        assert_eq!(path.to_string(), "/remote.php/dav/files/alice/a.txt");
    }

    #[test]
    fn names_are_escaped_on_the_wire_only() {
        let path = DavPath::new("alice", "Photos/My Photo #1 100%.txt").unwrap();

        assert_eq!(
            path.as_str(),
            "/remote.php/dav/files/alice/Photos/My%20Photo%20%231%20100%25.txt"
        );
        // Callers still see the name the user sees, segment by segment.
        assert_eq!(path.relative(), "Photos/My Photo #1 100%.txt");
        assert_eq!(
            path.segments().collect::<Vec<_>>(),
            vec!["Photos", "My Photo #1 100%.txt"]
        );
        assert_eq!(
            path.join("next.txt").unwrap().relative(),
            "Photos/My Photo #1 100%.txt/next.txt"
        );
    }

    #[test]
    fn a_pre_encoded_separator_stays_a_character() {
        // `a%2Fb` is one file called `a%2Fb`, not two segments: the `%` is escaped, so the server
        // decodes `%252F` back to the literal name instead of splitting the path.
        let path = DavPath::new("alice", "a%2Fb.txt").unwrap();

        assert_eq!(path.as_str(), "/remote.php/dav/files/alice/a%252Fb.txt");
        assert_eq!(path.segments().collect::<Vec<_>>(), vec!["a%2Fb.txt"]);
    }

    #[test]
    fn non_ascii_names_and_users_are_escaped() {
        let path = DavPath::new("jörg", "Grüße.jpg").unwrap();

        assert_eq!(
            path.as_str(),
            "/remote.php/dav/files/j%C3%B6rg/Gr%C3%BC%C3%9Fe.jpg"
        );
        assert_eq!(path.user(), "jörg");
        assert_eq!(path.relative(), "Grüße.jpg");
    }
}
