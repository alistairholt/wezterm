//! Joins a link that a program broke over several rows.
//!
//! Programs that lay out their own text (Claude Code, for one) break a
//! long path or URL with a real newline, and often indent the rest. The
//! terminal then sees unrelated lines, and the hyperlink rules match each
//! piece on its own. This module finds the pieces that belong to the link
//! under the mouse so that they can share one link.

use super::open_file::is_separator;
use std::ops::Range;
use std::sync::Arc;
use termwiz::hyperlink::Hyperlink;
use termwiz::surface::Line;

/// How far above the hovered row to look for the start of a link.
pub const MAX_ROWS: usize = 8;

/// A program may keep a margin at the right edge. A row whose text ends
/// within this many cells of the edge counts as full.
const EDGE_SLACK: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Index into the slice of lines given to `find_chain`
    pub row: usize,
    pub cols: Range<usize>,
}

/// The pieces of one link, top to bottom, and the URI they spell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub segments: Vec<Segment>,
    pub uri: String,
}

struct RowCell<'a> {
    col: usize,
    width: usize,
    text: &'a str,
    link: Option<&'a Arc<Hyperlink>>,
}

impl RowCell<'_> {
    fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }

    fn ends_token(&self) -> bool {
        self.text.chars().next().map_or(true, is_separator)
    }
}

/// The columns from the first to just past the last non-blank cell.
fn text_extent(line: &Line) -> Option<Range<usize>> {
    let mut first = None;
    let mut end = 0;
    for cell in line.visible_cells() {
        if cell.str().chars().all(char::is_whitespace) {
            continue;
        }
        first.get_or_insert(cell.cell_index());
        end = cell.cell_index() + cell.width().max(1);
    }
    first.map(|first| first..end)
}

/// True if the program, not the terminal, ended this row at the right
/// edge: the text reaches the edge and the row isn't soft-wrapped.
fn fills_width(line: &Line, cols: usize) -> bool {
    !line.last_cell_was_wrapped()
        && text_extent(line).map_or(false, |extent| extent.end + EDGE_SLACK >= cols)
}

fn same_link(a: Option<&Arc<Hyperlink>>, b: &Arc<Hyperlink>) -> bool {
    a.map_or(false, |a| Arc::ptr_eq(a, b))
}

/// Calls `f` with the cells of `line` up to its last non-blank cell.
fn with_row<R>(line: &Line, f: impl FnOnce(&[RowCell]) -> R) -> R {
    let refs: Vec<_> = line.visible_cells().collect();
    let mut cells: Vec<RowCell> = refs
        .iter()
        .map(|cell| RowCell {
            col: cell.cell_index(),
            width: cell.width().max(1),
            text: cell.str(),
            link: cell.attrs().hyperlink(),
        })
        .collect();
    while cells.last().map_or(false, RowCell::is_blank) {
        cells.pop();
    }
    f(&cells)
}

fn cols_of(cells: &[RowCell]) -> Range<usize> {
    let last = &cells[cells.len() - 1];
    cells[0].col..last.col + last.width
}

fn text_of(cells: &[RowCell]) -> String {
    cells.iter().map(|c| c.text).collect()
}

/// The link that runs to the end of the row's text: its link, columns
/// and text. Only a link made by the hyperlink rules counts, and only
/// one whose URI ends with the text shown, so that the rest of the text
/// can be added to the end of it.
fn tail_link(line: &Line) -> Option<(Arc<Hyperlink>, Range<usize>, String)> {
    with_row(line, |cells| {
        let link = cells.last()?.link?;
        if !link.is_implicit() {
            return None;
        }
        let start = cells
            .iter()
            .rposition(|c| !same_link(c.link, link))
            .map_or(0, |i| i + 1);
        let run = &cells[start..];
        let text = text_of(run);
        if !link.uri().ends_with(&text) {
            return None;
        }
        Some((Arc::clone(link), cols_of(run), text))
    })
}

struct Head {
    /// The columns and text, if the link ends on this row
    cols: Range<usize>,
    text: String,
    /// The columns and text, if the link carries on to the next row.
    /// Only set when the token is all there is on the row.
    whole_row: Option<(Range<usize>, String)>,
    /// The row's text reaches the right edge of the pane
    fills: bool,
}

/// The text at the start of the row that carries on a link from the row
/// above.
fn head_token(line: &Line, cols: usize) -> Option<Head> {
    let fills = fills_width(line, cols);
    with_row(line, |cells| {
        let first = cells.iter().position(|c| !c.is_blank())?;
        let cells = &cells[first..];
        let len = cells
            .iter()
            .position(RowCell::ends_token)
            .unwrap_or(cells.len());
        if len == 0 {
            return None;
        }
        let token = &cells[..len];
        if text_of(token).contains("://") {
            // A link of its own, not the rest of the one above
            return None;
        }
        let whole_row = (len == cells.len()).then(|| (cols_of(token), text_of(token)));
        // The link ends on this row. Where the rules linked the start of
        // the row, they know best where it ends. Otherwise drop the
        // punctuation that ends a sentence.
        let token = match token[0].link.filter(|link| link.is_implicit()) {
            Some(link) => {
                let end = token
                    .iter()
                    .position(|c| !same_link(c.link, link))
                    .unwrap_or(len);
                &token[..end]
            }
            None => {
                let end = token
                    .iter()
                    .rposition(|c| !matches!(c.text, "." | ":"))
                    .map_or(0, |i| i + 1);
                &token[..end]
            }
        };
        if token.is_empty() {
            return None;
        }
        Some(Head {
            cols: cols_of(token),
            text: text_of(token),
            whole_row,
            fills,
        })
    })
}

/// Builds the chain that starts with the link at the end of row `start`.
///
/// There are two ways to tell that the rows are one link. Either every
/// row but the last is filled to the right edge of the pane, which is
/// where a program breaks a word that is too long. Or the joined text is
/// the path of a file that `exists`: a program may keep a margin, and
/// text printed before the pane was resized stays broken at the old
/// width, so a broken path often stops short of the edge.
fn chain_from(
    lines: &[&mut Line],
    start: usize,
    cols: usize,
    exists: &dyn Fn(&str) -> bool,
) -> Option<Chain> {
    let (link, tail_cols, tail_text) = tail_link(&*lines[start])?;
    let link_start = tail_cols.start;
    let mut indent = None;
    // Every row so far is filled to the edge
    let mut full = fills_width(&*lines[start], cols);
    // The rows that carry on to a further row, then each way the link
    // could end: (segments, visible text, filled to the edge).
    let mut middle: Vec<(Segment, String)> = vec![];
    let mut endings: Vec<(Vec<Segment>, String, bool)> = vec![];

    for row in start + 1..lines.len().min(start + 1 + MAX_ROWS) {
        let Some(head) = head_token(&*lines[row], cols) else {
            break;
        };
        // The rest of a broken link starts at the left margin of its
        // paragraph. Text indented further than the link, or than the
        // row before, is something else (a stack frame under a path).
        if head.cols.start > link_start || *indent.get_or_insert(head.cols.start) != head.cols.start
        {
            break;
        }
        let mut segments = vec![Segment {
            row: start,
            cols: tail_cols.clone(),
        }];
        let mut text = tail_text.clone();
        for (segment, middle_text) in &middle {
            segments.push(segment.clone());
            text.push_str(middle_text);
        }
        segments.push(Segment {
            row,
            cols: head.cols.clone(),
        });
        text.push_str(&head.text);
        endings.push((segments, text, full));

        let Some((whole_cols, whole_text)) = head.whole_row else {
            break;
        };
        full = full && head.fills;
        middle.push((
            Segment {
                row,
                cols: whole_cols,
            },
            whole_text,
        ));
    }

    // The longest reading first. A row that is full carries on to the
    // next one, so the longest full reading is the link.
    let (segments, text, _) = endings
        .into_iter()
        .rev()
        .find(|(_, text, full)| *full || exists(text))?;
    let shown: usize = tail_text.len();
    let uri = format!("{}{}", link.uri(), &text[shown..]);
    Some(Chain { segments, uri })
}

/// Finds the broken link that covers `column` of row `hovered`.
/// `cols` is the width of the pane. `exists` says whether text names a
/// file; see `chain_from`.
pub fn find_chain(
    lines: &[&mut Line],
    hovered: usize,
    column: usize,
    cols: usize,
    exists: &dyn Fn(&str) -> bool,
) -> Option<Chain> {
    if hovered >= lines.len() {
        return None;
    }
    // Take the highest start: a middle piece can look like a link too.
    (hovered.saturating_sub(MAX_ROWS)..=hovered).find_map(|start| {
        chain_from(lines, start, cols, exists).filter(|chain| {
            chain
                .segments
                .iter()
                .any(|s| s.row == hovered && s.cols.contains(&column))
        })
    })
}

/// Puts `link` on columns `cols` of `line`. The renderer does this to
/// its own copy of a row that carries a piece of a joined link.
pub fn set_link(line: &mut Line, cols: Range<usize>, link: &Arc<Hyperlink>) {
    let cells = line.cells_mut_for_attr_changes_only();
    let end = cols.end.min(cells.len());
    for cell in &mut cells[cols.start.min(end)..end] {
        cell.attrs_mut().set_hyperlink(Some(Arc::clone(link)));
    }
    // A copy still points at the original's cached render state
    line.clear_appdata();
}

#[cfg(test)]
mod test {
    use super::*;
    use termwiz::cell::CellAttributes;
    use termwiz::hyperlink::Rule;

    const COLS: usize = 40;

    /// Rules like the ones a config adds for file paths, plus URLs.
    fn rules() -> Vec<Rule> {
        vec![
            Rule::new(r"\w+://\S+", "$0").unwrap(),
            Rule::with_highlight(r"(?:^|\s)((?:/[\w.@+-]*[\w@+-])+)", "path:$1", 1).unwrap(),
            Rule::with_highlight(
                r"(?:^|\s)([\w.@+-]*[\w@+-](?:/[\w.@+-]*[\w@+-])+)",
                "path:$1",
                1,
            )
            .unwrap(),
        ]
    }

    fn lines(text: &[&str]) -> Vec<Line> {
        let rules = rules();
        text.iter()
            .map(|t| {
                let mut line = Line::from_text(t, &CellAttributes::default(), 1, None);
                line.scan_and_create_hyperlinks(&rules);
                line
            })
            .collect()
    }

    fn find(lines: &mut [Line], hovered: usize, column: usize) -> Option<Chain> {
        find_with(lines, hovered, column, &|_| false)
    }

    fn find_with(
        lines: &mut [Line],
        hovered: usize,
        column: usize,
        exists: &dyn Fn(&str) -> bool,
    ) -> Option<Chain> {
        let refs: Vec<&mut Line> = lines.iter_mut().collect();
        find_chain(&refs, hovered, column, COLS, exists)
    }

    fn link_at(line: &Line, col: usize) -> Option<Arc<Hyperlink>> {
        line.get_cell(col)?.attrs().hyperlink().cloned()
    }

    // A path that fills the row and carries on, indented, on the next
    // row is one link from either row.
    #[test]
    fn joins_two_rows() {
        let mut l = lines(&[
            "  /private/tmp/claude-501/9e0a3bfc-1cd3-",
            "  4a34/scratchpad/voyager-details.png",
        ]);
        let expected = Chain {
            segments: vec![
                Segment {
                    row: 0,
                    cols: 2..40,
                },
                Segment {
                    row: 1,
                    cols: 2..37,
                },
            ],
            uri: "path:/private/tmp/claude-501/9e0a3bfc-1cd3-4a34/scratchpad/voyager-details.png"
                .to_string(),
        };
        assert_eq!(find(&mut l, 0, 10), Some(expected.clone()));
        assert_eq!(find(&mut l, 1, 10), Some(expected));
        // Not over the link
        assert_eq!(find(&mut l, 1, 0), None);
        assert_eq!(find(&mut l, 1, 38), None);
    }

    // The rest of the path need not match a rule itself, and text after
    // it is left out, as is a full stop that ends the sentence.
    #[test]
    fn rest_is_plain_text() {
        let mut l = lines(&[
            "see /private/tmp/claude-501/screenshot-ab",
            "cdef.png. Then more",
        ]);
        let chain = find(&mut l, 1, 3).unwrap();
        assert_eq!(
            chain.uri,
            "path:/private/tmp/claude-501/screenshot-abcdef.png"
        );
        assert_eq!(chain.segments[1].cols, 0..8);
        assert_eq!(find(&mut l, 1, 12), None);
    }

    // A path over three rows is found from the middle row, even though
    // the middle row matches a rule by itself.
    #[test]
    fn joins_three_rows() {
        let mut l = lines(&[
            "/private/tmp/claude-501/9e0a3bfc-1cd3-4a",
            "34-a119/ca7f14338e16/scratchpad/aaaaaaaa",
            "bbbb/slowboy.png",
        ]);
        assert!(link_at(&l[1], 5).is_some());
        let chain = find(&mut l, 1, 5).unwrap();
        assert_eq!(chain.segments.len(), 3);
        assert_eq!(
            chain.uri,
            "path:/private/tmp/claude-501/9e0a3bfc-1cd3-4a34-a119/ca7f14338e16/scratchpad/aaaaaaaabbbb/slowboy.png"
        );
    }

    // A broken URL joins the same way.
    #[test]
    fn joins_url() {
        let mut l = lines(&[
            "  go to https://example.com/a/very/long/p",
            "  ath/index.html now",
        ]);
        let chain = find(&mut l, 0, 20).unwrap();
        assert_eq!(chain.uri, "https://example.com/a/very/long/path/index.html");
    }

    // Short rows are a list of paths, not one broken path.
    #[test]
    fn short_rows_stay_apart() {
        let mut l = lines(&["/tmp/one.png", "/tmp/two.png"]);
        assert_eq!(find(&mut l, 0, 3), None);
        assert_eq!(find(&mut l, 1, 3), None);
    }

    // Text indented further than the link isn't the rest of it.
    #[test]
    fn deeper_indent_stays_apart() {
        let mut l = lines(&["/private/tmp/claude-501/9e0a3bfc-1cd3-4a", "    at main"]);
        assert_eq!(find(&mut l, 0, 3), None);
    }

    // A full row followed by a URL is two links.
    #[test]
    fn next_row_with_own_url_stays_apart() {
        let mut l = lines(&[
            "/private/tmp/claude-501/9e0a3bfc-1cd3-4a",
            "https://example.com/",
        ]);
        assert_eq!(find(&mut l, 0, 3), None);
    }

    // A soft-wrapped row is already one logical line, which the rules
    // handle themselves.
    #[test]
    fn soft_wrapped_row_is_left_alone() {
        let mut l = lines(&[
            "/private/tmp/claude-501/9e0a3bfc-1cd3-4a",
            "34/scratchpad/voyager-details.png",
        ]);
        l[0].set_last_cell_was_wrapped(true, 1);
        assert_eq!(find(&mut l, 0, 3), None);
    }

    // Rows that stop short of the edge (the program keeps a margin, or
    // the pane was resized after they were printed) are one link when
    // the joined text is a file that exists, and not otherwise.
    #[test]
    fn short_rows_join_when_the_file_exists() {
        let text = [
            "- Voyager Briefcase, desktop:",
            "  /private/tmp/claude-501/9e0a3bfc-1",
            "  cd3/scratchpad/l-voyager.png",
            "  /private/tmp/claude-501/other.png",
        ];
        let mut l = lines(&text);
        assert_eq!(find(&mut l, 1, 5), None);

        let file = "/private/tmp/claude-501/9e0a3bfc-1cd3/scratchpad/l-voyager.png";
        let exists = |text: &str| text == file;
        let chain = find_with(&mut l, 2, 5, &exists).unwrap();
        assert_eq!(chain.uri, format!("path:{file}"));
        assert_eq!(chain.segments.len(), 2);
        assert_eq!(find_with(&mut l, 1, 5, &exists), Some(chain));
        // The path on the row after it is not part of it
        assert_eq!(find_with(&mut l, 3, 5, &exists), None);
    }

    // set_link puts the link on the given columns only.
    #[test]
    fn set_link_covers_the_columns() {
        let mut l = lines(&["cdef.png. Then more"]);
        let link = Arc::new(Hyperlink::new_implicit("path:/tmp/abcdef.png"));
        set_link(&mut l[0], 0..8, &link);
        assert!(same_link(link_at(&l[0], 0).as_ref(), &link));
        assert!(same_link(link_at(&l[0], 7).as_ref(), &link));
        assert_eq!(link_at(&l[0], 8), None);
        // Columns past the end of the line are ignored
        set_link(&mut l[0], 15..400, &link);
        assert!(same_link(link_at(&l[0], 18).as_ref(), &link));
    }
}
