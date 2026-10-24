//! Breadcrumb path decomposition and folder navigation history state machine.

use serde::{Deserialize, Serialize};

use crate::path::{normalize_path, parent_path};

/// Segment of an interactive breadcrumb path bar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreadcrumbSegment {
    /// Display label for this breadcrumb button.
    pub name: String,
    /// Full canonical virtual path navigated to when clicked.
    pub full_path: String,
    /// Whether this segment represents the root mount.
    pub is_root: bool,
}

/// Splits a virtual path into clickable breadcrumb segments starting at `root_label`.
pub fn breadcrumb_segments(current_path: &str, root_label: &str) -> Vec<BreadcrumbSegment> {
    let mut segments = vec![BreadcrumbSegment {
        name: root_label.to_string(),
        full_path: "/".to_string(),
        is_root: true,
    }];

    let trimmed = current_path.trim_matches('/');
    if trimmed.is_empty() {
        return segments;
    }

    let mut acc = String::new();
    for part in trimmed.split('/') {
        if part.is_empty() {
            continue;
        }
        acc.push('/');
        acc.push_str(part);
        segments.push(BreadcrumbSegment {
            name: part.to_string(),
            full_path: acc.clone(),
            is_root: false,
        });
    }

    segments
}

/// Pure navigation history state machine tracking current path, back stack, and forward stack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationHistory {
    current: String,
    back_stack: Vec<String>,
    forward_stack: Vec<String>,
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationHistory {
    /// Creates a new navigation history positioned at `"/"`.
    pub fn new() -> Self {
        Self {
            current: "/".to_string(),
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
        }
    }

    /// Returns the current canonical directory path.
    pub fn current_path(&self) -> &str {
        &self.current
    }

    /// Returns `true` if backward history navigation is available.
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Returns `true` if forward history navigation is available.
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Navigates backward to the previous directory in the history stack.
    pub fn go_back(&mut self) -> bool {
        let Some(prev) = self.back_stack.pop() else {
            return false;
        };
        let old = std::mem::replace(&mut self.current, prev);
        self.forward_stack.push(old);
        true
    }

    /// Navigates forward to the next directory in the history stack.
    pub fn go_forward(&mut self) -> bool {
        let Some(next) = self.forward_stack.pop() else {
            return false;
        };
        let old = std::mem::replace(&mut self.current, next);
        self.back_stack.push(old);
        true
    }

    /// Navigates into a child directory of the current directory.
    pub fn navigate_into(&mut self, child_name: &str) -> &str {
        let trimmed = child_name.trim_matches('/');
        if trimmed.is_empty() {
            return &self.current;
        }
        let target = if self.current == "/" {
            format!("/{trimmed}")
        } else {
            format!("{}/{trimmed}", self.current.trim_end_matches('/'))
        };
        self.navigate_to(&target)
    }

    /// Navigates up to the parent directory of the current directory.
    pub fn navigate_up(&mut self) -> &str {
        if let Some(parent) = parent_path(&self.current) {
            self.navigate_to(&parent)
        } else {
            &self.current
        }
    }

    /// Navigates directly to a target path, pushing the previous path onto the back stack.
    pub fn navigate_to(&mut self, path: &str) -> &str {
        let normalized = normalize_path(path).unwrap_or_else(|_| "/".to_string());
        if normalized != self.current {
            let old = std::mem::replace(&mut self.current, normalized);
            self.back_stack.push(old);
            self.forward_stack.clear();
        }
        &self.current
    }

    /// Computes breadcrumb segments for the current path.
    pub fn breadcrumbs(&self, root_label: &str) -> Vec<BreadcrumbSegment> {
        breadcrumb_segments(&self.current, root_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_back_forward_and_breadcrumbs() {
        let mut nav = NavigationHistory::new();
        assert_eq!(nav.current_path(), "/");
        assert!(!nav.can_go_back());

        nav.navigate_into("Documents");
        assert_eq!(nav.current_path(), "/Documents");
        assert!(nav.can_go_back());

        nav.navigate_into("Projects");
        assert_eq!(nav.current_path(), "/Documents/Projects");

        let crumbs = nav.breadcrumbs("Nextcloud");
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].name, "Nextcloud");
        assert_eq!(crumbs[1].full_path, "/Documents");
        assert_eq!(crumbs[2].full_path, "/Documents/Projects");

        assert!(nav.go_back());
        assert_eq!(nav.current_path(), "/Documents");
        assert!(nav.can_go_forward());

        assert!(nav.go_forward());
        assert_eq!(nav.current_path(), "/Documents/Projects");

        nav.navigate_up();
        assert_eq!(nav.current_path(), "/Documents");
    }
}
