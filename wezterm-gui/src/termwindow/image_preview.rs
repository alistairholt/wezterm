//! Thumbnails under the paths of image files in a pane's output.
//!
//! The terminal can't put a picture into another program's output: the
//! program owns the screen. Instead the GUI draws a thumbnail under the
//! row that names an image file and moves the rows below it down. Only
//! what is drawn moves; the terminal's rows stay as they are.

use super::open_file::is_separator;
use crate::quad::{QuadTrait, TripleLayerQuadAllocator, TripleLayerQuadAllocatorTrait};
use crate::termwindow::render::paint::AllowImage;
use crate::termwindow::{TermWindow, TermWindowNotif, UIItem, UIItemType};
use ::window::bitmaps::{TextureCoord, TextureRect, TextureSize};
use ::window::color::LinearRgba;
use ::window::WindowOps;
use anyhow::Context;
use mux::pane::{CachePolicy, WithPaneLines};
use mux::renderable::RenderableDimensions;
use mux::tab::PositionedPane;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use termwiz::image::{ImageData, ImageDataType};
use termwiz::surface::{Line, SequenceNo};
use wezterm_term::StableRowIndex;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/// Files larger than this are not read.
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
/// Thumbnails are stored no larger than this, in pixels. An image is
/// scaled to the width; if it is then taller than the height, only the
/// top of it is kept, which is all that is ever drawn.
const MAX_THUMB_WIDTH: u32 = 1600;
const MAX_THUMB_HEIGHT: u32 = 1000;
/// How long a file's state is trusted before it is looked at again.
const RECHECK_AFTER: Duration = Duration::from_secs(2);
const MAX_CACHE_ENTRIES: usize = 128;
/// The most rows a path is followed over.
const MAX_JOINED_ROWS: usize = 8;
/// How many files are read at once.
const MAX_LOADS: usize = 4;
/// Blank pixels above a thumbnail.
const TOP_GAP: f32 = 2.0;
/// A thumbnail that is cut off fades out over this many rows at most.
const FADE_ROWS: f32 = 2.5;
/// The height of each strip of the fade, in pixels.
const FADE_STRIP: f32 = 2.0;

/// What `find_candidates` needs to know about a row.
#[derive(Debug, Clone, Default)]
pub struct RowText {
    /// The row's text without trailing blanks
    pub text: String,
    /// True if the terminal soft-wrapped the row onto the next one
    pub wrapped: bool,
}

impl RowText {
    pub fn from_line(line: &Line) -> Self {
        Self {
            text: line.as_str().trim_end().to_string(),
            wrapped: line.last_cell_was_wrapped(),
        }
    }
}

/// Text that may name an image file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The row the text ends on; the thumbnail goes under it
    pub row: usize,
    /// The column the thumbnail starts at
    pub col: usize,
    pub text: String,
}

/// One token of the output, read in each way it could be a path. The
/// first reading that names a file wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateGroup {
    pub readings: Vec<Candidate>,
    /// The row the token starts on
    pub start_row: usize,
    /// Set when the token starts a row: it may be the rest of a path
    /// that started on the row above
    pub head_of: Option<usize>,
}

/// True if `text` looks like the path of an image file. A bare file
/// name doesn't count: `ls` in a folder of photos would fill the pane.
fn has_image_extension(text: &str) -> bool {
    let Some((stem, ext)) = text.rsplit_once('.') else {
        return false;
    };
    stem.contains('/') && IMAGE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e))
}

/// Drops the punctuation that ends a sentence, and a `file://` prefix.
fn clean(token: &str) -> &str {
    let token = token.trim_end_matches(|c| matches!(c, '.' | ':'));
    token.strip_prefix("file://").unwrap_or(token)
}

fn indent_of(chars: &[char]) -> usize {
    chars.iter().take_while(|c| c.is_whitespace()).count()
}

/// Finds the text in `rows` that looks like the path of an image file.
///
/// A path can run over several rows, either soft-wrapped by the terminal
/// or broken by the program with its own newlines (see `wrapped_link`).
/// A broken path need not reach the right edge: a program may keep a
/// margin, and text printed before a resize stays broken at the old
/// width. So a token that ends a row is read every way: joined with the
/// start of each following row, and by itself. Only a reading that names
/// a file gets a thumbnail.
pub fn find_candidates(rows: &[RowText]) -> Vec<CandidateGroup> {
    let rows: Vec<(Vec<char>, &RowText)> =
        rows.iter().map(|r| (r.text.chars().collect(), r)).collect();
    let mut groups = vec![];

    for (i, (chars, _)) in rows.iter().enumerate() {
        let indent = indent_of(chars);
        let mut pos = 0;
        while pos < chars.len() {
            if is_separator(chars[pos]) {
                pos += 1;
                continue;
            }
            let start = pos;
            while pos < chars.len() && !is_separator(chars[pos]) {
                pos += 1;
            }
            let token: String = chars[start..pos].iter().collect();
            let mut readings = vec![];

            if pos == chars.len() {
                // The token ends the row, so the path may carry on. Each
                // row it could end on is a reading, the longest first:
                // the one that names a file is the path.
                let mut joined = token.clone();
                let mut last = i;
                let mut rest_indent = None;
                let mut joins = vec![];
                while let Some((next_chars, _)) = rows.get(last + 1) {
                    let next_indent = indent_of(next_chars);
                    let follows = if rows[last].1.wrapped {
                        next_indent == 0
                    } else {
                        // As for links: the rest starts at the left
                        // margin, no further right than the path began
                        next_indent <= start
                            && *rest_indent.get_or_insert(next_indent) == next_indent
                    };
                    if !follows {
                        break;
                    }
                    let head_len = next_chars[next_indent..]
                        .iter()
                        .take_while(|c| !is_separator(**c))
                        .count();
                    if head_len == 0 {
                        break;
                    }
                    joined.extend(&next_chars[next_indent..next_indent + head_len]);
                    last += 1;
                    joins.push(Candidate {
                        row: last,
                        col: next_indent,
                        text: clean(&joined).to_string(),
                    });
                    if next_indent + head_len != next_chars.len() || last - i >= MAX_JOINED_ROWS {
                        break;
                    }
                }
                readings.extend(joins.into_iter().rev());
            }
            readings.push(Candidate {
                row: i,
                col: indent,
                text: clean(&token).to_string(),
            });
            readings.retain(|c| has_image_extension(&c.text));
            if !readings.is_empty() {
                groups.push(CandidateGroup {
                    readings,
                    start_row: i,
                    head_of: (start == indent).then_some(i),
                });
            }
        }
    }
    groups
}

/// Turns the text of a candidate into a path. Relative paths need `cwd`.
pub fn resolve_path(text: &str, cwd: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(rest) = text.strip_prefix("~/") {
        return Some(home?.join(rest));
    }
    let path = Path::new(text);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        Some(cwd?.join(path))
    }
}

/// A decoded, scaled-down image.
pub struct Thumbnail {
    pub image: Arc<ImageData>,
    pub width: u32,
    pub height: u32,
    /// The width of the image file, which may be more than `width`
    pub source_width: u32,
}

enum LoadResult {
    Unchanged,
    Missing,
    Loaded(Thumbnail, SystemTime),
}

/// Reads and scales the image at `path`. Runs on its own thread.
fn load(path: &Path, known_mtime: Option<SystemTime>) -> LoadResult {
    let Ok(meta) = std::fs::metadata(path) else {
        return LoadResult::Missing;
    };
    if !meta.is_file() || meta.len() > MAX_FILE_SIZE {
        return LoadResult::Missing;
    }
    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if known_mtime == Some(mtime) {
        return LoadResult::Unchanged;
    }
    let decoded = image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(image::ImageError::IoError)
        .and_then(|reader| reader.decode());
    let decoded = match decoded {
        Ok(decoded) => decoded,
        Err(err) => {
            log::debug!("image preview: can't read {}: {err:#}", path.display());
            return LoadResult::Missing;
        }
    };
    // Keep the top of the image, as much as fits the stored height once
    // it is scaled to the stored width.
    let source_width = decoded.width();
    let scale = (MAX_THUMB_WIDTH as f32 / decoded.width().max(1) as f32).min(1.0);
    let keep = ((MAX_THUMB_HEIGHT as f32 / scale).ceil() as u32).min(decoded.height());
    let decoded = if keep < decoded.height() {
        decoded.crop_imm(0, 0, decoded.width(), keep)
    } else {
        decoded
    };
    let decoded = if decoded.width() > MAX_THUMB_WIDTH {
        decoded.thumbnail(MAX_THUMB_WIDTH, MAX_THUMB_HEIGHT)
    } else {
        decoded
    };
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return LoadResult::Missing;
    }
    let image = Arc::new(ImageData::with_data(ImageDataType::new_single_frame(
        width,
        height,
        rgba.into_raw(),
    )));
    LoadResult::Loaded(
        Thumbnail {
            image,
            width,
            height,
            source_width,
        },
        mtime,
    )
}

struct Entry {
    thumb: Option<Arc<Thumbnail>>,
    mtime: Option<SystemTime>,
    loading: bool,
    checked: Instant,
}

/// The thumbnails of the image files seen in the window's panes.
#[derive(Default)]
pub struct PreviewCache {
    entries: HashMap<PathBuf, Entry>,
    /// Goes up whenever a thumbnail appears, changes or goes away
    generation: usize,
    /// How many files are being read
    loading: usize,
    /// True if a file wasn't read because enough were being read already
    waiting: bool,
}

impl PreviewCache {
    /// Returns the thumbnail for `path` if it is ready. Otherwise, or
    /// if the file hasn't been looked at for a while, has it read on
    /// another thread; the window repaints when that changes anything.
    fn get(&mut self, path: &Path, window: Option<&::window::Window>) -> Option<Arc<Thumbnail>> {
        let now = Instant::now();
        if !self.entries.contains_key(path) {
            if self.entries.len() >= MAX_CACHE_ENTRIES {
                self.trim();
            }
            self.entries.insert(
                path.to_path_buf(),
                Entry {
                    thumb: None,
                    mtime: None,
                    loading: false,
                    // Old enough to be read straight away
                    checked: now.checked_sub(RECHECK_AFTER * 2).unwrap_or(now),
                },
            );
        }
        let entry = self.entries.get_mut(path)?;
        if !entry.loading && now.duration_since(entry.checked) >= RECHECK_AFTER {
            if self.loading >= MAX_LOADS {
                self.waiting = true;
            } else if let Some(window) = window {
                self.loading += 1;
                entry.loading = true;
                entry.checked = now;
                let window = window.clone();
                let path = path.to_path_buf();
                let known_mtime = entry.mtime;
                std::thread::spawn(move || {
                    let result = load(&path, known_mtime);
                    window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
                        if term_window.image_previews.finish(&path, result) {
                            if let Some(window) = term_window.window.as_ref() {
                                window.invalidate();
                            }
                        }
                    })));
                });
            }
        }
        entry.thumb.clone()
    }

    /// Records the result of a read. Returns true if it changed what
    /// should be drawn.
    fn finish(&mut self, path: &Path, result: LoadResult) -> bool {
        self.loading = self.loading.saturating_sub(1);
        // Have the layouts worked out again, so that a file that had to
        // wait is read now.
        let waiting = std::mem::take(&mut self.waiting);
        if waiting {
            self.generation += 1;
        }
        let Some(entry) = self.entries.get_mut(path) else {
            return waiting;
        };
        entry.loading = false;
        entry.checked = Instant::now();
        let changed = match result {
            LoadResult::Unchanged => false,
            LoadResult::Missing => {
                entry.mtime = None;
                entry.thumb.take().is_some()
            }
            LoadResult::Loaded(thumb, mtime) => {
                entry.thumb = Some(Arc::new(thumb));
                entry.mtime = Some(mtime);
                true
            }
        };
        if changed {
            self.generation += 1;
        }
        changed || waiting
    }

    /// Drops the half of the entries that were looked at longest ago.
    fn trim(&mut self) {
        let mut times: Vec<Instant> = self.entries.values().map(|e| e.checked).collect();
        times.sort();
        let cutoff = times[times.len() / 2];
        self.entries.retain(|_, e| e.loading || e.checked > cutoff);
    }
}

/// A thumbnail and where it goes.
pub struct PlacedThumb {
    /// The row of the viewport it sits under
    pub line_idx: usize,
    pub col: usize,
    /// How many rows it takes up
    pub rows: usize,
    /// Its size on screen, in pixels
    pub width: f32,
    pub height: f32,
    /// How much of the image's height is drawn, from the top (0 to 1)
    pub shown: f32,
    pub path: PathBuf,
    pub thumb: Arc<Thumbnail>,
}

/// How to draw an image: scaled down to fit `avail_width`, and if it
/// is then taller than `max_rows` rows, cut off at the bottom so that
/// its top shows. An image narrower than `avail_width` keeps its own
/// size: `max_width` is the width of the image file, which the stored
/// `image_width` may be less than. Returns the width and height to
/// draw, the rows it takes, and how much of the image's height is shown
/// (0 to 1, from the top).
pub fn thumb_size(
    image_width: u32,
    image_height: u32,
    max_width: u32,
    cell_height: f32,
    max_rows: usize,
    avail_width: f32,
) -> (f32, f32, usize, f32) {
    let max_height = (max_rows.max(1) as f32 * cell_height - TOP_GAP).max(1.0);
    let scale = avail_width.max(1.0).min(max_width as f32) / image_width as f32;
    let width = (image_width as f32 * scale).round().max(1.0);
    let full_height = (image_height as f32 * scale).max(1.0);
    let height = full_height.min(max_height).round().max(1.0);
    let shown = (height / full_height).min(1.0);
    let rows = ((height + TOP_GAP) / cell_height).ceil().max(1.0) as usize;
    (width, height, rows, shown)
}

/// The most rows each of `count` thumbnails may take. They share half
/// of the pane between them, so that the text stays readable when
/// several images are on screen, but none gets fewer than three rows.
pub fn rows_per_thumb(max_rows: usize, count: usize, viewport_rows: usize) -> usize {
    let share = (viewport_rows / 2) / count.max(1);
    max_rows.min(share.max(3)).max(1)
}

/// Where each row of a pane's viewport is drawn once thumbnails have
/// taken their rows.
#[derive(Default)]
pub struct PreviewLayout {
    /// For each row of the viewport, how many rows down it is drawn from
    /// its own place. Negative when rows have moved up off the top.
    offsets: Vec<i64>,
    /// The rows the thumbnails add in all, less the rows moved off the top
    tail_offset: i64,
    pub thumbs: Vec<PlacedThumb>,
    /// `lines_fingerprint` of the lines this was worked out from
    pub fingerprint: u64,
}

/// A number that changes when any of a viewport's lines is replaced.
///
/// A pane's own seqno doesn't cover this for a mux client: its lines
/// arrive from the server after the first paint, as blank lines being
/// swapped for real ones, with no change to the pane's seqno.
pub fn lines_fingerprint(lines: &[&mut Line]) -> u64 {
    lines.iter().fold(lines.len() as u64, |acc, line| {
        acc.wrapping_mul(0x100000001b3)
            .wrapping_add(line.current_seqno() as u64)
    })
}

impl PreviewLayout {
    /// `thumbs` must be in row order. `last_used` is the last row of
    /// the viewport that must stay on screen: the cursor's row or the
    /// last row with text, whichever is lower. Rows move down to make
    /// room for the thumbnails; if that would push `last_used` off the
    /// bottom, everything moves up instead and the top rows go out of
    /// view, as they do when a program prints more output.
    pub fn build(thumbs: Vec<PlacedThumb>, viewport_rows: usize, last_used: usize) -> Self {
        if thumbs.is_empty() {
            return Self::default();
        }
        let mut offsets = Vec::with_capacity(viewport_rows);
        let mut added = 0i64;
        let mut next = thumbs.iter().peekable();
        let mut added_through_last_used = 0;
        for idx in 0..viewport_rows {
            offsets.push(added);
            while let Some(thumb) = next.next_if(|t| t.line_idx <= idx) {
                added += thumb.rows as i64;
            }
            if idx == last_used.min(viewport_rows.saturating_sub(1)) {
                added_through_last_used = added;
            }
        }
        let bottom =
            last_used.min(viewport_rows.saturating_sub(1)) as i64 + 1 + added_through_last_used;
        let overflow = (bottom - viewport_rows as i64).max(0);
        for offset in offsets.iter_mut() {
            *offset -= overflow;
        }
        Self {
            offsets,
            tail_offset: added - overflow,
            thumbs,
            fingerprint: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.thumbs.is_empty()
    }

    /// The row at which row `idx` of the viewport is drawn.
    pub fn visual_row(&self, idx: i64) -> i64 {
        if self.offsets.is_empty() {
            idx
        } else if idx < 0 {
            idx + self.offsets[0]
        } else {
            idx + self
                .offsets
                .get(idx as usize)
                .copied()
                .unwrap_or(self.tail_offset)
        }
    }

    /// The row of the viewport drawn at `visual`. A position over a
    /// thumbnail gives the row the thumbnail sits under.
    pub fn row_at_visual(&self, visual: i64) -> i64 {
        if self.offsets.is_empty() {
            return visual;
        }
        let len = self.offsets.len() as i64;
        if visual >= len + self.tail_offset {
            return visual - self.tail_offset;
        }
        for idx in (0..len).rev() {
            if idx + self.offsets[idx as usize] <= visual {
                return idx;
            }
        }
        visual - self.offsets[0]
    }

    /// The row at which the top of thumbnail `n` is drawn.
    pub fn thumb_visual_row(&self, n: usize) -> i64 {
        let thumb = &self.thumbs[n];
        // Thumbnails that share a row stack up under it
        let above: usize = self.thumbs[..n]
            .iter()
            .filter(|t| t.line_idx == thumb.line_idx)
            .map(|t| t.rows)
            .sum();
        self.visual_row(thumb.line_idx as i64) + 1 + above as i64
    }
}

/// What a pane's layout was worked out from. While it stays the same,
/// so does the layout.
#[derive(Clone, PartialEq)]
pub struct LayoutKey {
    seqno: SequenceNo,
    viewport_top: StableRowIndex,
    rows: usize,
    cols: usize,
    cursor_y: StableRowIndex,
    cell_height: usize,
    cell_width: usize,
    cache_generation: usize,
    config_generation: usize,
}

struct CollectRows {
    stable_top: StableRowIndex,
    rows: Vec<RowText>,
    last_text: Option<usize>,
    fingerprint: u64,
}

impl WithPaneLines for CollectRows {
    fn with_lines_mut(&mut self, stable_top: StableRowIndex, lines: &mut [&mut Line]) {
        self.stable_top = stable_top;
        self.fingerprint = lines_fingerprint(lines);
        for (idx, line) in lines.iter().enumerate() {
            let row = RowText::from_line(line);
            if !row.text.is_empty() {
                self.last_text = Some(idx);
            }
            self.rows.push(row);
        }
    }
}

impl TermWindow {
    /// Works out which thumbnails to draw in a pane and where its rows
    /// go. The result is kept until the pane's content, viewport or
    /// size changes, or a thumbnail arrives.
    pub fn image_preview_layout(
        &mut self,
        pos: &PositionedPane,
        current_viewport: Option<StableRowIndex>,
        dims: &RenderableDimensions,
    ) -> Rc<PreviewLayout> {
        let pane_id = pos.pane.pane_id();
        if !self.config.inline_image_previews {
            self.pane_state(pane_id).image_previews = None;
            return Rc::new(PreviewLayout::default());
        }

        let viewport_top = current_viewport.unwrap_or(dims.physical_top);
        let cursor = pos.pane.get_cursor_position();
        let key = LayoutKey {
            seqno: pos.pane.get_current_seqno(),
            viewport_top,
            rows: dims.viewport_rows,
            cols: dims.cols,
            cursor_y: cursor.y,
            cell_height: self.render_metrics.cell_size.height as usize,
            cell_width: self.render_metrics.cell_size.width as usize,
            cache_generation: self.image_previews.generation,
            config_generation: self.config.generation(),
        };
        if let Some((cached_key, layout)) = self.pane_state(pane_id).image_previews.as_ref() {
            if *cached_key == key {
                return Rc::clone(layout);
            }
        }

        let mut collect = CollectRows {
            stable_top: viewport_top,
            rows: Vec::with_capacity(dims.viewport_rows),
            last_text: None,
            fingerprint: 0,
        };
        pos.pane.with_lines_mut(
            viewport_top..viewport_top + dims.viewport_rows as StableRowIndex,
            &mut collect,
        );

        let groups = find_candidates(&collect.rows);
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let max_rows = self.config.inline_image_preview_max_rows;
        let mut thumbs: Vec<PlacedThumb> = vec![];
        if !groups.is_empty() {
            let home = dirs_next::home_dir();
            let mut cwd = None;
            let needs_cwd = groups
                .iter()
                .flat_map(|g| &g.readings)
                .any(|c| !c.text.starts_with('/') && !c.text.starts_with("~/"));
            if needs_cwd {
                cwd = pos
                    .pane
                    .get_current_working_dir(CachePolicy::AllowStale)
                    .and_then(|url| url.to_file_path().ok());
            }
            // Rows whose first token was used as the rest of a path
            let mut used_heads = vec![false; collect.rows.len()];
            for group in &groups {
                if group.head_of.map_or(false, |row| used_heads[row]) {
                    continue;
                }
                for reading in &group.readings {
                    let Some(path) = resolve_path(&reading.text, cwd.as_deref(), home.as_deref())
                    else {
                        continue;
                    };
                    let Some(thumb) = self.image_previews.get(&path, self.window.as_ref()) else {
                        continue;
                    };
                    if thumbs.iter().any(|t| t.path == path) {
                        // Shown once, under the first mention
                        break;
                    }
                    for row in group.start_row + 1..=reading.row {
                        used_heads[row] = true;
                    }
                    // Sized below, once the number of thumbnails is known
                    thumbs.push(PlacedThumb {
                        line_idx: reading.row,
                        col: reading.col,
                        rows: 0,
                        width: 0.,
                        height: 0.,
                        shown: 1.,
                        path,
                        thumb,
                    });
                    break;
                }
            }
            thumbs.sort_by_key(|t| t.line_idx);
            let max_rows = rows_per_thumb(max_rows, thumbs.len(), dims.viewport_rows);
            for placed in thumbs.iter_mut() {
                let avail = dims.cols.saturating_sub(placed.col) as f32 * cell_width;
                let (width, height, rows, shown) = thumb_size(
                    placed.thumb.width,
                    placed.thumb.height,
                    placed.thumb.source_width,
                    cell_height,
                    max_rows,
                    avail,
                );
                placed.width = width;
                placed.height = height;
                placed.rows = rows;
                placed.shown = shown;
            }
        }

        let cursor_row = usize::try_from(cursor.y - collect.stable_top).ok();
        let last_used = collect
            .last_text
            .max(cursor_row.filter(|row| *row < dims.viewport_rows))
            .unwrap_or(0);
        let mut layout = PreviewLayout::build(thumbs, dims.viewport_rows, last_used);
        layout.fingerprint = collect.fingerprint;
        let layout = Rc::new(layout);
        self.pane_state(pane_id).image_previews = Some((key, Rc::clone(&layout)));
        layout
    }
}

impl TermWindow {
    /// Draws a pane's thumbnails and makes each one clickable.
    pub fn paint_image_previews(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        layout: &PreviewLayout,
        top_pixel_y: f32,
        left_pixel_x: f32,
    ) -> anyhow::Result<()> {
        if layout.is_empty() || self.allow_images == AllowImage::No {
            return Ok(());
        }
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let pane_top = top_pixel_y + pos.top as f32 * cell_height;
        let pane_bottom = pane_top + pos.height as f32 * cell_height;
        let origin_x = self.dimensions.pixel_width as f32 / -2.;
        let origin_y = self.dimensions.pixel_height as f32 / -2.;

        for (n, placed) in layout.thumbs.iter().enumerate() {
            let top = pane_top + layout.thumb_visual_row(n) as f32 * cell_height + TOP_GAP;
            let bottom = top + placed.height;
            // Rows can move up past the top of the pane; draw the part
            // of the thumbnail that is still inside it.
            let visible_top = top.max(pane_top);
            let visible_bottom = bottom.min(pane_bottom);
            if visible_bottom <= visible_top {
                continue;
            }
            let x = left_pixel_x + placed.col as f32 * cell_width;

            let gl_state = self.render_state.as_ref().unwrap();
            let (sprite, next_due, _load_state) = gl_state
                .glyph_cache
                .borrow_mut()
                .cached_image(&placed.thumb.image, None, self.allow_images)
                .context("cached_image")?;
            self.update_next_frame_time(next_due);

            let texture_width = sprite.texture.width() as f32;
            let texture_height = sprite.texture.height() as f32;
            // Only the top `shown` of the image is on show
            let sprite_height = sprite.coords.size.height as f32 * placed.shown;
            let hsv = if pos.is_active {
                None
            } else {
                Some(self.config.inactive_pane_hsb)
            };

            // An image that is cut off fades out at the bottom, to show
            // that there is more of it. A quad has one opacity, so the
            // fade is a stack of thin strips, each fainter than the last.
            let fade = if placed.shown < 1.0 {
                (placed.height * 0.5).min(cell_height * FADE_ROWS)
            } else {
                0.0
            };
            let fade_top = bottom - fade;
            let mut strips = vec![(top, fade_top, 1.0)];
            let count = (fade / FADE_STRIP).ceil() as usize;
            for n in 0..count {
                let strip_top = fade_top + n as f32 * FADE_STRIP;
                let strip_bottom = (strip_top + FADE_STRIP).min(bottom);
                // Ease out, so the fade doesn't start with a visible step
                let t = (n as f32 + 0.5) / count as f32;
                strips.push((strip_top, strip_bottom, (1.0 - t) * (1.0 - t)));
            }

            for (strip_top, strip_bottom, opacity) in strips {
                let strip_top = strip_top.max(visible_top);
                let strip_bottom = strip_bottom.min(visible_bottom);
                if strip_bottom <= strip_top {
                    continue;
                }
                let from = (strip_top - top) / placed.height;
                let to = (strip_bottom - top) / placed.height;
                let texture_rect = TextureRect::new(
                    TextureCoord::new(
                        sprite.coords.origin.x as f32 / texture_width,
                        (sprite.coords.origin.y as f32 + from * sprite_height) / texture_height,
                    ),
                    TextureSize::new(
                        sprite.coords.size.width as f32 / texture_width,
                        (to - from) * sprite_height / texture_height,
                    ),
                );
                let mut quad = layers.allocate(0)?;
                quad.set_position(
                    origin_x + x,
                    origin_y + strip_top,
                    origin_x + x + placed.width,
                    origin_y + strip_bottom,
                );
                quad.set_hsv(hsv);
                quad.set_fg_color(LinearRgba::with_components(1., 1., 1., opacity));
                quad.set_texture(texture_rect);
                // Drawn as a background image is: smoothly scaled, and
                // with the opacity given above
                quad.set_is_background_image();
            }

            // A thin outline, so that an image with the pane's background
            // color still has an edge. It stops where the fade starts.
            let outline = pos.pane.palette().foreground.to_linear().mul_alpha(0.25);
            let sides_bottom = visible_bottom.min(fade_top).max(visible_top);
            let sides_height = sides_bottom - visible_top;
            let mut edges = vec![
                euclid::rect(x, visible_top, 1., sides_height),
                euclid::rect(x + placed.width - 1., visible_top, 1., sides_height),
            ];
            if top >= pane_top {
                edges.push(euclid::rect(x, top, placed.width, 1.));
            }
            if bottom <= pane_bottom && fade == 0.0 {
                edges.push(euclid::rect(x, bottom - 1., placed.width, 1.));
            }
            for edge in edges {
                self.filled_rectangle(layers, 0, edge, outline)
                    .context("filled_rectangle")?;
            }

            self.ui_items.push(UIItem {
                x: x.max(0.) as usize,
                y: visible_top.max(0.) as usize,
                width: placed.width as usize,
                height: (visible_bottom - visible_top) as usize,
                item_type: UIItemType::ImagePreview {
                    path: placed.path.clone(),
                },
            });
        }
        Ok(())
    }
}

/// Shows `paths` in the macOS Quick Look panel, the first one in front.
pub fn open_quick_look(paths: Vec<PathBuf>) {
    // Not from inside the mouse event: showing the panel takes the key
    // window away, which calls back into the window.
    promise::spawn::spawn_into_main_thread(async move {
        ::window::os::macos::quick_look::show(&paths);
    })
    .detach();
}

#[cfg(test)]
mod test {
    use super::*;

    fn row(text: &str) -> RowText {
        RowText {
            text: text.to_string(),
            wrapped: false,
        }
    }

    fn texts(groups: &[CandidateGroup]) -> Vec<Vec<(usize, &str)>> {
        groups
            .iter()
            .map(|g| {
                g.readings
                    .iter()
                    .map(|c| (c.row, c.text.as_str()))
                    .collect()
            })
            .collect()
    }

    // Only tokens with an image extension are candidates; the sentence's
    // full stop and quotes are not part of the path.
    #[test]
    fn finds_image_paths_in_a_row() {
        let rows = [row(
            "saved /tmp/a.png and \"~/b.JPG\". See notes.txt, c.gif and shots/d.gif.",
        )];
        let groups = find_candidates(&rows);
        assert_eq!(
            texts(&groups),
            vec![
                vec![(0, "/tmp/a.png")],
                vec![(0, "~/b.JPG")],
                vec![(0, "shots/d.gif")]
            ]
        );
    }

    // A path the program broke over two rows is read joined first, and
    // the thumbnail goes under its last row at that row's indent.
    #[test]
    fn joins_a_hard_wrapped_path() {
        let rows = [
            row("  /private/tmp/claude-501/9e0a3bfc-1cd3-"),
            row("  4a34/scratchpad/voyager-details.png"),
            row(""),
        ];
        let groups = find_candidates(&rows);
        assert_eq!(
            groups[0].readings,
            vec![Candidate {
                row: 1,
                col: 2,
                text: "/private/tmp/claude-501/9e0a3bfc-1cd3-4a34/scratchpad/voyager-details.png"
                    .to_string()
            }]
        );
        // The second row's start is also read by itself, marked as the
        // start of a row so that it can be skipped once the join is used.
        assert_eq!(
            texts(&groups[1..]),
            vec![vec![(1, "4a34/scratchpad/voyager-details.png")]]
        );
        assert_eq!(groups[1].head_of, Some(1));
    }

    // A full row that ends in a complete path is read joined with the
    // next row and by itself, so the real file is found either way.
    #[test]
    fn full_row_is_read_both_ways() {
        let rows = [
            row("/private/tmp/claude-501/shots/slowbo.png"),
            row("/tmp/next.png"),
        ];
        let groups = find_candidates(&rows);
        assert_eq!(
            texts(&groups),
            vec![
                vec![
                    (1, "/private/tmp/claude-501/shots/slowbo.png/tmp/next.png"),
                    (0, "/private/tmp/claude-501/shots/slowbo.png")
                ],
                vec![(1, "/tmp/next.png")]
            ]
        );
    }

    // A soft-wrapped path carries on at column 0 of the next row.
    #[test]
    fn joins_a_soft_wrapped_path() {
        let mut first = row("see /tmp/shots/voyag");
        first.wrapped = true;
        let rows = [first, row("er.png now")];
        let groups = find_candidates(&rows);
        assert_eq!(
            texts(&groups[..1]),
            vec![vec![(1, "/tmp/shots/voyager.png")]]
        );
    }

    #[test]
    fn resolves_paths() {
        let cwd = Path::new("/work");
        let home = Path::new("/home/me");
        assert_eq!(
            resolve_path("/tmp/a.png", None, None),
            Some(PathBuf::from("/tmp/a.png"))
        );
        assert_eq!(
            resolve_path("~/a.png", Some(cwd), Some(home)),
            Some(PathBuf::from("/home/me/a.png"))
        );
        assert_eq!(
            resolve_path("shots/a.png", Some(cwd), Some(home)),
            Some(PathBuf::from("/work/shots/a.png"))
        );
        assert_eq!(resolve_path("shots/a.png", None, Some(home)), None);
    }

    // An image wider than the space is scaled down to it. If that
    // leaves it too tall, its top is shown. A smaller image keeps its
    // own size.
    #[test]
    fn sizes_thumbnails() {
        // Fits: 800 wide makes it 400 tall, under 8 rows of 100px
        assert_eq!(
            thumb_size(1600, 800, 1600, 100.0, 8, 800.0),
            (800.0, 400.0, 5, 1.0)
        );
        // Too tall: 1000px at full width, cut to 8 rows of 20px less the
        // 2px gap, which is the top 15.8% of the image
        assert_eq!(
            thumb_size(1600, 2000, 1600, 20.0, 8, 800.0),
            (800.0, 158.0, 8, 0.158)
        );
        // Narrower than the space: its own size
        assert_eq!(thumb_size(40, 30, 40, 20.0, 8, 800.0), (40.0, 30.0, 2, 1.0));
        // Stored at 1600 wide from a 3000 wide file: it can fill a
        // 2000px space, since the file is wider than that
        assert_eq!(
            thumb_size(1600, 80, 3000, 20.0, 8, 2000.0),
            (2000.0, 100.0, 6, 1.0)
        );
    }

    fn placed(line_idx: usize, rows: usize) -> PlacedThumb {
        PlacedThumb {
            line_idx,
            col: 0,
            rows,
            width: 10.0,
            height: 10.0,
            shown: 1.0,
            path: PathBuf::from(format!("/tmp/{line_idx}.png")),
            thumb: Arc::new(Thumbnail {
                image: Arc::new(ImageData::with_data(ImageDataType::new_single_frame(
                    1,
                    1,
                    vec![0, 0, 0, 0],
                ))),
                width: 1,
                height: 1,
                source_width: 1,
            }),
        }
    }

    // Thumbnails share half the pane, down to three rows each.
    #[test]
    fn thumbnails_share_half_the_pane() {
        assert_eq!(rows_per_thumb(8, 1, 40), 8);
        assert_eq!(rows_per_thumb(8, 3, 37), 6);
        assert_eq!(rows_per_thumb(8, 10, 40), 3);
        assert_eq!(rows_per_thumb(2, 1, 40), 2);
    }

    // With room below the text, rows under a thumbnail move down and
    // the rows above it stay put.
    #[test]
    fn layout_pushes_rows_down() {
        let layout = PreviewLayout::build(vec![placed(2, 3)], 10, 5);
        assert_eq!(layout.visual_row(0), 0);
        assert_eq!(layout.visual_row(2), 2);
        assert_eq!(layout.visual_row(3), 6);
        assert_eq!(layout.thumb_visual_row(0), 3);
        // Positions over the thumbnail belong to the row above it
        assert_eq!(layout.row_at_visual(2), 2);
        assert_eq!(layout.row_at_visual(4), 2);
        assert_eq!(layout.row_at_visual(5), 2);
        assert_eq!(layout.row_at_visual(6), 3);
        assert_eq!(layout.row_at_visual(12), 9);
        assert_eq!(layout.row_at_visual(13), 10);
    }

    // With text down to the last row, everything moves up instead so
    // that the last row stays on screen.
    #[test]
    fn layout_moves_up_when_full() {
        let layout = PreviewLayout::build(vec![placed(2, 3), placed(6, 2)], 10, 9);
        assert_eq!(layout.visual_row(9), 9);
        assert_eq!(layout.visual_row(6), 4);
        assert_eq!(layout.visual_row(2), -3);
        assert_eq!(layout.visual_row(0), -5);
        assert_eq!(layout.thumb_visual_row(1), 5);
        // The first thumbnail's last row is still in view at the top
        assert_eq!(layout.row_at_visual(0), 2);
        assert_eq!(layout.row_at_visual(1), 3);
        assert_eq!(layout.row_at_visual(9), 9);
        assert_eq!(layout.row_at_visual(5), 6);
    }

    // Without thumbnails nothing moves.
    #[test]
    fn empty_layout_is_identity() {
        let layout = PreviewLayout::build(vec![], 10, 9);
        assert!(layout.is_empty());
        assert_eq!(layout.visual_row(4), 4);
        assert_eq!(layout.row_at_visual(4), 4);
    }
}
