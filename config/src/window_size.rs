//! The size, in cells, of the last window you resized, saved
//! so that `remember_window_size` can open new windows at that size.

use crate::DATA_DIR;
use std::path::PathBuf;
use std::sync::Mutex;

/// `None` until the file has been read; then the saved size, if any.
static REMEMBERED: Mutex<Option<Option<(u16, u16)>>> = Mutex::new(None);

fn size_file() -> PathBuf {
    DATA_DIR.join("window-size")
}

fn parse(text: &str) -> Option<(u16, u16)> {
    let mut parts = text.split_whitespace();
    let cols: u16 = parts.next()?.parse().ok()?;
    let rows: u16 = parts.next()?.parse().ok()?;
    if cols == 0 || rows == 0 {
        return None;
    }
    Some((cols, rows))
}

/// The saved (cols, rows), if there is one.
pub fn remembered_window_size() -> Option<(u16, u16)> {
    let mut remembered = REMEMBERED.lock().unwrap();
    *remembered.get_or_insert_with(|| {
        std::fs::read_to_string(size_file())
            .ok()
            .and_then(|text| parse(&text))
    })
}

/// Save (cols, rows) as the size for new windows. Does nothing when
/// the size hasn't changed, so it is cheap to call on every resize.
pub fn set_remembered_window_size(cols: u16, rows: u16) {
    if cols == 0 || rows == 0 {
        return;
    }
    if remembered_window_size() == Some((cols, rows)) {
        return;
    }
    REMEMBERED.lock().unwrap().replace(Some((cols, rows)));
    let path = size_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    if let Err(err) = std::fs::write(&path, format!("{cols} {rows}\n")) {
        log::warn!("Failed to save window size to {}: {err:#}", path.display());
    }
}

#[cfg(test)]
mod test {
    use super::parse;

    #[test]
    fn parse_saved_size() {
        assert_eq!(parse("120 40\n"), Some((120, 40)));
        assert_eq!(parse("0 40"), None);
        assert_eq!(parse("120"), None);
        assert_eq!(parse("abc def"), None);
    }
}
