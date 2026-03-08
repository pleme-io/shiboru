//! Live grep source: searches file contents for a pattern.

use super::{Source, SourceItem};
use std::path::PathBuf;
use std::process::Command;

/// Source that runs `grep -rn` (or `rg` if available) against the working
/// directory and returns matching lines.
pub struct GrepSource {
    root: PathBuf,
}

impl GrepSource {
    /// Create a grep source rooted at `root`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Run grep and parse the output.
    fn search(&self, pattern: &str) -> Vec<SourceItem> {
        if pattern.is_empty() {
            return Vec::new();
        }

        // Try rg first (faster, respects .gitignore), fall back to grep.
        let output = Command::new("rg")
            .args([
                "--no-heading",
                "--line-number",
                "--color=never",
                "--max-count=200",
                "--max-columns=200",
                pattern,
            ])
            .current_dir(&self.root)
            .output()
            .or_else(|_| {
                Command::new("grep")
                    .args([
                        "-rn",
                        "--color=never",
                        "--include=*",
                        "-m",
                        "200",
                        pattern,
                    ])
                    .current_dir(&self.root)
                    .output()
            });

        let output = match output {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_grep_output(&stdout)
    }
}

impl Source for GrepSource {
    fn collect(&self, query: &str) -> Vec<SourceItem> {
        self.search(query)
    }

    fn name(&self) -> &str {
        "Grep"
    }

    fn is_live(&self) -> bool {
        true
    }
}

/// Parse `file:line:text` output into source items.
fn parse_grep_output(output: &str) -> Vec<SourceItem> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            // Format: file:line:text  (or file:line:col:text for rg)
            let first_colon = line.find(':')?;
            let rest = &line[first_colon + 1..];
            let second_colon = rest.find(':')?;

            let file = &line[..first_colon];
            let line_no = &rest[..second_colon];
            let text = rest[second_colon + 1..].trim();

            let display = format!("{file}:{line_no}: {text}");
            let action = format!("{file}:{line_no}");
            let filter_text = display.clone();

            Some(SourceItem {
                display,
                filter_text,
                action,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grep_output_basic() {
        let output = "src/main.rs:10:fn main() {\nsrc/lib.rs:5:pub mod foo;\n";
        let items = parse_grep_output(output);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].action, "src/main.rs:10");
        assert!(items[0].display.contains("fn main()"));
        assert_eq!(items[1].action, "src/lib.rs:5");
    }

    #[test]
    fn parse_grep_output_empty() {
        let items = parse_grep_output("");
        assert!(items.is_empty());
    }

    #[test]
    fn parse_grep_output_malformed() {
        // Lines without enough colons are skipped.
        let output = "no-colon-here\nonly:one\nvalid:42:match\n";
        let items = parse_grep_output(output);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].action, "valid:42");
    }

    #[test]
    fn grep_source_is_live() {
        let source = GrepSource::new(PathBuf::from("/tmp"));
        assert!(source.is_live());
    }

    #[test]
    fn grep_source_empty_query() {
        let source = GrepSource::new(PathBuf::from("/tmp"));
        let items = source.collect("");
        assert!(items.is_empty());
    }

    #[test]
    fn grep_source_searches_real_files() {
        use std::fs;

        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path();
        fs::write(root.join("hello.txt"), "hello world\ngoodbye world\n").unwrap();
        fs::write(root.join("other.txt"), "no match here\nhello again\n").unwrap();

        let source = GrepSource::new(root.to_path_buf());
        let items = source.collect("hello");

        // Should find at least the two "hello" lines.
        assert!(
            items.len() >= 2,
            "expected at least 2 matches, got {}: {items:?}",
            items.len()
        );

        let actions: Vec<&str> = items.iter().map(|i| i.action.as_str()).collect();
        assert!(
            actions.iter().any(|a| a.contains("hello.txt")),
            "should match in hello.txt: {actions:?}"
        );
        assert!(
            actions.iter().any(|a| a.contains("other.txt")),
            "should match in other.txt: {actions:?}"
        );
    }
}
