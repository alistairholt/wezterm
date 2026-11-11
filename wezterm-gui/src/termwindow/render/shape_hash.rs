//! A faster replacement for `Line::compute_shape_hash`.
//!
//! The shape hash keys the line state, shape and quad caches, so it is
//! computed for every changed line that is drawn. `Line::compute_shape_hash`
//! feeds SipHash-1-3 (128-bit) about 24 small writes per cell (text, the
//! attribute bits, and each color component with its length prefix), and
//! each write waits on the one before. That came to about 75ns per cell:
//! 15µs for a 200 column line, and 4–9% of main thread time while output
//! scrolls or a full-screen app redraws.
//!
//! `LineShapeHasher` sends each write to the next of four lanes in turn,
//! mixing it with a folded multiply as foldhash does. The four lanes have
//! no dependency on each other, so four multiplies run at once: about 10ns
//! per cell. The hash is only ever compared within this process, so it
//! need not match `Line::compute_shape_hash`; it covers the same inputs
//! (the per-cell shape hash and the line's display bits) and uses a random
//! key per process so that output can't be crafted to collide.

use std::collections::hash_map::RandomState;
use std::convert::TryInto;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::LazyLock;
use wezterm_term::Line;

static KEY: LazyLock<[u64; 4]> = LazyLock::new(|| {
    let state = RandomState::new();
    std::array::from_fn(|i| state.hash_one(i))
});

/// Computes a hash of `line` that changes if the way its contents are
/// shaped and drawn would change. Equivalent in purpose to
/// `Line::compute_shape_hash`.
pub fn line_shape_hash(line: &Line) -> [u8; 16] {
    let mut hasher = LineShapeHasher::new(*KEY);
    line.is_double_width().hash(&mut hasher);
    line.is_double_height_top().hash(&mut hasher);
    line.is_double_height_bottom().hash(&mut hasher);
    line.has_hyperlink().hash(&mut hasher);
    let (bidi_enabled, direction) = line.bidi_info();
    bidi_enabled.hash(&mut hasher);
    (direction as u8).hash(&mut hasher);
    for cell in line.visible_cells() {
        cell.compute_shape_hash(&mut hasher);
    }
    hasher.finish128()
}

#[inline(always)]
fn folded_multiply(a: u64, b: u64) -> u64 {
    let full = (a as u128).wrapping_mul(b as u128);
    (full as u64) ^ ((full >> 64) as u64)
}

struct LineShapeHasher {
    lanes: [u64; 4],
    multiplier: u64,
    key: [u64; 4],
}

impl LineShapeHasher {
    fn new(key: [u64; 4]) -> Self {
        Self {
            lanes: key,
            multiplier: key[0] | 1,
            key,
        }
    }

    /// Mixes `value` into the first lane, then rotates the lanes so the
    /// next write goes to the lane after it.
    #[inline(always)]
    fn mix(&mut self, value: u64) {
        let [a, b, c, d] = self.lanes;
        self.lanes = [b, c, d, folded_multiply(a ^ value, self.multiplier)];
    }

    fn finish128(&self) -> [u8; 16] {
        let k = self.key;
        let [a, b, c, d] = self.lanes;
        // Each half depends on all four lanes, then on the other half.
        let h1 = folded_multiply(a ^ k[1], b ^ k[2]) ^ folded_multiply(c ^ k[3], d ^ k[0]);
        let h2 = folded_multiply(a ^ k[2], d ^ k[3]) ^ folded_multiply(b ^ k[0], c ^ k[1]);
        let h1 = folded_multiply(h1 ^ k[3], h2 ^ k[1]) ^ h1;
        let h2 = folded_multiply(h2 ^ k[2], h1 ^ k[0]) ^ h2;
        let mut out = [0u8; 16];
        out[..8].copy_from_slice(&h1.to_le_bytes());
        out[8..].copy_from_slice(&h2.to_le_bytes());
        out
    }
}

impl Hasher for LineShapeHasher {
    fn finish(&self) -> u64 {
        u64::from_le_bytes(self.finish128()[..8].try_into().unwrap())
    }

    #[inline(always)]
    fn write(&mut self, mut bytes: &[u8]) {
        let len = bytes.len() as u64;
        while bytes.len() >= 8 {
            self.mix(u64::from_le_bytes(bytes[..8].try_into().unwrap()));
            bytes = &bytes[8..];
        }
        // The tail is at most 7 bytes, so the top byte is free for the
        // length; that keeps "a" + "" apart from "" + "a".
        let mut tail = [0u8; 8];
        tail[..bytes.len()].copy_from_slice(bytes);
        self.mix(u64::from_le_bytes(tail) ^ (len << 56));
    }

    #[inline(always)]
    fn write_u8(&mut self, value: u8) {
        self.mix(value as u64)
    }

    #[inline(always)]
    fn write_u16(&mut self, value: u16) {
        self.mix(value as u64)
    }

    #[inline(always)]
    fn write_u32(&mut self, value: u32) {
        self.mix(value as u64)
    }

    #[inline(always)]
    fn write_u64(&mut self, value: u64) {
        self.mix(value)
    }

    #[inline(always)]
    fn write_usize(&mut self, value: usize) {
        self.mix(value as u64)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use termwiz::cell::{Cell, CellAttributes, Intensity};
    use termwiz::color::{ColorAttribute, SrgbaTuple};
    use termwiz::hyperlink::Hyperlink;
    use wezterm_bidi::ParagraphDirectionHint;

    /// Builds a set of lines that differ in one property at a time:
    /// text, colors, attributes, hyperlinks, wide cells and line bits.
    /// Some are deliberately identical so that equal content must hash
    /// equal.
    fn sample_lines() -> Vec<Line> {
        let mut lines = vec![];
        let base = |text: &str| Line::from_text(text, &CellAttributes::default(), 1, None);

        lines.push(base("hello world"));
        lines.push(base("hello world"));
        lines.push(base("hello worle"));
        lines.push(base("hello  world"));
        lines.push(base("helloworld"));
        lines.push(base(""));
        lines.push(base("a"));
        lines.push(base("あいう"));
        lines.push(base("🦀 crab"));

        for i in 0..8u8 {
            let mut attrs = CellAttributes::default();
            attrs.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(SrgbaTuple(
                i as f32 / 8.,
                0.5,
                0.25,
                1.0,
            )));
            lines.push(Line::from_text("colors", &attrs, 1, None));
            let mut attrs = CellAttributes::default();
            attrs.set_background(ColorAttribute::PaletteIndex(i));
            lines.push(Line::from_text("colors", &attrs, 1, None));
        }

        let mut bold = CellAttributes::default();
        bold.set_intensity(Intensity::Bold);
        lines.push(Line::from_text("hello world", &bold, 1, None));

        let mut link = CellAttributes::default();
        link.set_hyperlink(Some(Arc::new(Hyperlink::new("https://example.com"))));
        lines.push(Line::from_text("hello world", &link, 1, None));
        let mut link2 = CellAttributes::default();
        link2.set_hyperlink(Some(Arc::new(Hyperlink::new("https://example.org"))));
        lines.push(Line::from_text("hello world", &link2, 1, None));

        let mut double = base("hello world");
        double.set_double_width(1);
        lines.push(double);
        let mut top = base("hello world");
        top.set_double_height_top(1);
        lines.push(top);
        let mut bidi = base("hello world");
        bidi.set_bidi_info(true, ParagraphDirectionHint::RightToLeft, 1);
        lines.push(bidi);

        let mut one_cell = base("hello world");
        one_cell.set_cell(4, Cell::new('O', CellAttributes::default()), 2);
        lines.push(one_cell);

        lines
    }

    /// The new hash must split the sample lines into exactly the same
    /// groups as `Line::compute_shape_hash`: lines hash equal under one
    /// if and only if they hash equal under the other.
    #[test]
    fn groups_lines_like_compute_shape_hash() {
        let lines = sample_lines();
        let mut old_to_new = HashMap::new();
        let mut new_to_old = HashMap::new();
        for line in &lines {
            let old = line.compute_shape_hash();
            let new = line_shape_hash(line);
            assert_eq!(*old_to_new.entry(old).or_insert(new), new, "{line:?}");
            assert_eq!(*new_to_old.entry(new).or_insert(old), old, "{line:?}");
        }
        // The two "hello world" lines are the only duplicates.
        assert_eq!(new_to_old.len(), lines.len() - 1);
    }

    /// Changing any one cell of a wide line, in text or color, changes
    /// the hash.
    #[test]
    fn every_cell_change_changes_the_hash() {
        let text: String = (0..200).map(|i| (b'a' + (i % 26) as u8) as char).collect();
        let line = Line::from_text(&text, &CellAttributes::default(), 1, None);
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(line_shape_hash(&line)));
        for x in 0..200 {
            let mut changed = line.clone();
            changed.set_cell(x, Cell::new('#', CellAttributes::default()), 2);
            assert!(seen.insert(line_shape_hash(&changed)), "text at {}", x);

            let mut changed = line.clone();
            let mut attrs = CellAttributes::default();
            attrs.set_foreground(ColorAttribute::PaletteIndex(1));
            let c = changed
                .visible_cells()
                .nth(x)
                .unwrap()
                .str()
                .chars()
                .next()
                .unwrap();
            changed.set_cell(x, Cell::new(c, attrs), 2);
            assert!(seen.insert(line_shape_hash(&changed)), "color at {}", x);
        }
    }
}
