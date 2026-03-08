//! File-finder source: walks the directory tree and lists files.

use super::{Source, SourceItem};
use std::path::{Path, PathBuf};

/// Directories that are always skipped during file walking.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".next",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".direnv",
];

/// Source that lists files under a root directory.
pub struct FileSource {
    root: PathBuf,
}

impl FileSource {
    /// Create a file source rooted at `root`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Recursively walk directories and collect file paths.
    fn walk(&self) -> Vec<SourceItem> {
        let mut items = Vec::new();
        let mut stack = vec![self.root.clone()];

        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();

                if path.is_dir() {
                    if !SKIP_DIRS.contains(&name_str.as_ref()) && !name_str.starts_with('.') {
                        stack.push(path);
                    }
                    continue;
                }

                if let Some(rel) = Self::relative_display(&path, &self.root) {
                    items.push(SourceItem::simple(&rel));
                }
            }
        }

        items.sort_by(|a, b| a.display.cmp(&b.display));
        items
    }

    /// Produce a relative path string from `path` relative to `root`.
    fn relative_display(path: &Path, root: &Path) -> Option<String> {
        path.strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
}

impl Source for FileSource {
    fn collect(&self, _query: &str) -> Vec<SourceItem> {
        self.walk()
    }

    fn name(&self) -> &str {
        "Files"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path();

        fs::write(root.join("foo.rs"), "").unwrap();
        fs::write(root.join("bar.txt"), "").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();

        // Directories that should be skipped.
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "").unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/pkg.js"), "").unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/debug"), "").unwrap();

        dir
    }

    #[test]
    fn walks_files_and_skips_hidden() {
        let dir = make_temp_tree();
        let source = FileSource::new(dir.path().to_path_buf());
        let items = source.collect("");

        let names: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
        assert!(names.contains(&"foo.rs"), "should find foo.rs: {names:?}");
        assert!(names.contains(&"bar.txt"), "should find bar.txt: {names:?}");
        assert!(
            names.contains(&"src/main.rs"),
            "should find src/main.rs: {names:?}"
        );
        assert!(
            names.contains(&"src/lib.rs"),
            "should find src/lib.rs: {names:?}"
        );

        // Skipped directories.
        assert!(
            !names.iter().any(|n| n.contains(".git")),
            "should not include .git: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.contains("node_modules")),
            "should not include node_modules: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.contains("target")),
            "should not include target: {names:?}"
        );
    }

    #[test]
    fn results_are_sorted() {
        let dir = make_temp_tree();
        let source = FileSource::new(dir.path().to_path_buf());
        let items = source.collect("");
        let names: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn empty_dir_returns_empty() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let source = FileSource::new(dir.path().to_path_buf());
        let items = source.collect("");
        assert!(items.is_empty());
    }

    #[test]
    fn is_not_live() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let source = FileSource::new(dir.path().to_path_buf());
        assert!(!source.is_live());
    }
}
