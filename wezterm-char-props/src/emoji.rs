use crate::emoji_variation::VARIATION_MAP;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub enum Presentation {
    Text,
    Emoji,
}

impl Presentation {
    /// Returns the default presentation followed
    /// by the explicit presentation if specified
    /// by a variation selector
    pub fn for_grapheme(s: &str) -> (Self, Option<Self>) {
        // A single byte is an ASCII character. No ASCII character has
        // emoji presentation, and every variation sequence ends in a
        // variation selector, so skip the map and table lookups. This
        // runs for every cell drawn.
        if s.len() == 1 {
            return (Self::Text, None);
        }
        Self::for_grapheme_lookup(s)
    }

    fn for_grapheme_lookup(s: &str) -> (Self, Option<Self>) {
        if let Some((a, b)) = VARIATION_MAP.get(s) {
            return (*a, Some(*b));
        }
        let mut presentation = Self::Text;
        for c in s.chars() {
            if Self::for_char(c) == Self::Emoji {
                presentation = Self::Emoji;
                break;
            }
            // Note that `c` may be some other combining
            // sequence that doesn't definitively indicate
            // that we're text, so we only positively
            // change presentation when we identify an
            // emoji char.
        }
        (presentation, None)
    }

    pub fn for_char(c: char) -> Self {
        if crate::emoji_presentation::EMOJI_PRESENTATION.contains_u32(c as u32) {
            Self::Emoji
        } else {
            Self::Text
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// The single-byte fast path must give the same answer as the full
    /// lookup for every ASCII character.
    #[test]
    fn ascii_fast_path_matches_lookup() {
        for b in 0u8..128 {
            let s = (b as char).to_string();
            assert_eq!(
                Presentation::for_grapheme(&s),
                Presentation::for_grapheme_lookup(&s),
                "{s:?}"
            );
        }
    }

    /// Longer graphemes still go through the lookups.
    #[test]
    fn non_ascii_still_looked_up() {
        assert_eq!(
            Presentation::for_grapheme("\u{1F600}"),
            (Presentation::Emoji, None)
        );
        assert_eq!(
            Presentation::for_grapheme("\u{2764}\u{FE0F}"),
            (Presentation::Text, Some(Presentation::Emoji))
        );
        assert_eq!(Presentation::for_grapheme("é"), (Presentation::Text, None));
    }
}
