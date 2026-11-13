//! Finds a file path in terminal text at the mouse position, for
//! `OpenFileAtMouseCursor` (iTerm2's "semantic history").

use std::path::{Path, PathBuf};
use termwiz::surface::Line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTarget {
    pub path: PathBuf,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Returns the text of `line` and the char index of the cell at `column`.
/// Returns `None` if `column` is past the end of the text.
pub fn line_text_at(line: &Line, column: usize) -> Option<(String, usize)> {
    let mut text = String::new();
    let mut chars = 0;
    let mut hit = None;
    for cell in line.visible_cells() {
        let start = cell.cell_index();
        let end = start + cell.width().max(1);
        if hit.is_none() && column >= start && column < end {
            hit = Some(chars);
        }
        text.push_str(cell.str());
        chars += cell.str().chars().count();
    }
    hit.map(|idx| (text, idx))
}

/// Characters that end a path token. Spaces in paths are not supported:
/// terminal output gives no way to tell them from separators.
pub(super) fn is_separator(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '`' | '(' | ')' | '<' | '>' | '[' | ']' | '{' | '}' | '|' | ',' | ';'
        )
}

/// Splits a `:line` or `:line:column` location from `token`, cutting at
/// the first `:` followed by a digit. Text after the numbers is dropped,
/// as in grep's `file:12:match`.
fn split_line_suffix(token: &str) -> (&str, Option<u32>, Option<u32>) {
    fn leading_number(s: &str) -> Option<(u32, &str)> {
        let len = s.bytes().take_while(|b| b.is_ascii_digit()).count();
        if len == 0 {
            return None;
        }
        Some((s[..len].parse().ok()?, &s[len..]))
    }

    let Some(colon) = token
        .match_indices(':')
        .map(|(i, _)| i)
        .find(|&i| i > 0 && leading_number(&token[i + 1..]).is_some())
    else {
        return (token, None, None);
    };
    let base = &token[..colon];
    let Some((line, rest)) = leading_number(&token[colon + 1..]) else {
        return (token, None, None);
    };
    let column = rest
        .strip_prefix(':')
        .and_then(leading_number)
        .map(|(n, _)| n);
    (base, Some(line), column)
}

/// Reads Python's `", line N` after a quoted file name.
fn python_line_after(after: &str) -> Option<u32> {
    let digits: String = after
        .strip_prefix("\", line ")?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Finds the file named by the text around char index `click` in `text`.
/// Relative paths are resolved against `cwd`, and `~/` against `home`.
/// Only a path for which `exists` returns true is returned.
pub fn find_file_target(
    text: &str,
    click: usize,
    cwd: Option<&Path>,
    home: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<FileTarget> {
    let chars: Vec<char> = text.chars().collect();
    if click >= chars.len() || is_separator(chars[click]) {
        return None;
    }
    let mut start = click;
    while start > 0 && !is_separator(chars[start - 1]) {
        start -= 1;
    }
    let mut end = click + 1;
    while end < chars.len() && !is_separator(chars[end]) {
        end += 1;
    }
    let token: String = chars[start..end].iter().collect();
    let after: String = chars[end..].iter().collect();

    // Punctuation that ends a sentence or a compiler message, as in
    // "see src/main.rs." or "src/main.rs:12:5:".
    let token = token.trim_end_matches(|c| matches!(c, '.' | ':'));
    let (base, mut line, mut column) = split_line_suffix(token);
    if line.is_none() {
        line = python_line_after(&after);
        column = None;
    }

    let mut candidates = vec![base];
    // `git diff` prints paths as a/foo and b/foo.
    for prefix in ["a/", "b/"] {
        if let Some(stripped) = base.strip_prefix(prefix) {
            candidates.push(stripped);
        }
    }

    for candidate in candidates {
        let path = if let Some(rest) = candidate.strip_prefix("~/") {
            match home {
                Some(home) => home.join(rest),
                None => continue,
            }
        } else if candidate == "~" {
            match home {
                Some(home) => home.to_path_buf(),
                None => continue,
            }
        } else {
            let p = Path::new(candidate);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                match cwd {
                    Some(cwd) => cwd.join(p),
                    None => continue,
                }
            }
        };
        if exists(&path) {
            return Some(FileTarget { path, line, column });
        }
    }
    None
}

/// Builds the command that opens `target`. With no `template`, uses
/// macOS's `open`. Otherwise substitutes `{file}`, `{line}` and
/// `{column}` in each argument; a missing line or column becomes 1.
pub fn build_open_command(template: Option<&[String]>, target: &FileTarget) -> Vec<String> {
    let file = target.path.to_string_lossy();
    match template {
        None | Some([]) => vec!["/usr/bin/open".to_string(), file.to_string()],
        Some(args) => {
            let line = target.line.unwrap_or(1).to_string();
            let column = target.column.unwrap_or(1).to_string();
            args.iter()
                .map(|arg| {
                    arg.replace("{file}", &file)
                        .replace("{line}", &line)
                        .replace("{column}", &column)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn find(text: &str, needle: &str, existing: &[&str]) -> Option<FileTarget> {
        // Click on the first char of `needle`.
        let byte = text.find(needle).expect("needle in text");
        let click = text[..byte].chars().count();
        find_file_target(
            text,
            click,
            Some(Path::new("/work")),
            Some(Path::new("/home/me")),
            |p| existing.iter().any(|e| Path::new(e) == p),
        )
    }

    fn target(path: &str, line: Option<u32>, column: Option<u32>) -> Option<FileTarget> {
        Some(FileTarget {
            path: PathBuf::from(path),
            line,
            column,
        })
    }

    #[test]
    fn relative_path_resolves_against_cwd() {
        assert_eq!(
            find("ls src/main.rs here", "main", &["/work/src/main.rs"]),
            target("/work/src/main.rs", None, None)
        );
    }

    #[test]
    fn missing_file_is_ignored() {
        assert_eq!(find("ls src/main.rs here", "main", &[]), None);
    }

    #[test]
    fn compiler_location_gives_line_and_column() {
        // rustc and gcc style, with a trailing colon.
        assert_eq!(
            find("error: src/lib.rs:12:5: oops", "src", &["/work/src/lib.rs"]),
            target("/work/src/lib.rs", Some(12), Some(5))
        );
        // grep -n style.
        assert_eq!(
            find("src/lib.rs:40:fn main()", "src", &["/work/src/lib.rs"]),
            target("/work/src/lib.rs", Some(40), None)
        );
    }

    #[test]
    fn python_traceback_line() {
        assert_eq!(
            find(
                "  File \"/srv/app.py\", line 17, in <module>",
                "app",
                &["/srv/app.py"]
            ),
            target("/srv/app.py", Some(17), None)
        );
    }

    #[test]
    fn git_diff_prefix_is_stripped() {
        assert_eq!(
            find(
                "diff --git a/foo/bar.rs b/foo/bar.rs",
                "b/foo",
                &["/work/foo/bar.rs"]
            ),
            target("/work/foo/bar.rs", None, None)
        );
    }

    #[test]
    fn home_and_absolute_paths() {
        assert_eq!(
            find("see ~/notes.txt.", "~/", &["/home/me/notes.txt"]),
            target("/home/me/notes.txt", None, None)
        );
        assert_eq!(
            find("(/etc/hosts)", "/etc", &["/etc/hosts"]),
            target("/etc/hosts", None, None)
        );
    }

    #[test]
    fn click_on_separator_finds_nothing() {
        assert_eq!(find("a b", " ", &["/work/a", "/work/b"]), None);
    }

    #[test]
    fn open_command_placeholders() {
        let t = FileTarget {
            path: PathBuf::from("/w/a.rs"),
            line: Some(3),
            column: None,
        };
        assert_eq!(
            build_open_command(None, &t),
            vec!["/usr/bin/open", "/w/a.rs"]
        );
        let tmpl = vec![
            "code".to_string(),
            "-g".to_string(),
            "{file}:{line}:{column}".to_string(),
        ];
        assert_eq!(
            build_open_command(Some(&tmpl), &t),
            vec!["code", "-g", "/w/a.rs:3:1"]
        );
    }

    #[test]
    fn line_text_maps_wide_cells() {
        let line = Line::from_text("日本 a.rs", &Default::default(), 0, None);
        // Columns: 日=0..2, 本=2..4, space=4, a=5.
        let (text, idx) = line_text_at(&line, 5).unwrap();
        assert_eq!(text, "日本 a.rs");
        assert_eq!(text.chars().nth(idx), Some('a'));
        let (_, idx) = line_text_at(&line, 1).unwrap();
        assert_eq!(idx, 0);
    }
}
