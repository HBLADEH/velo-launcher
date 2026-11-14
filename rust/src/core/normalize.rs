//! Frozen Go Unicode rules keep token boundaries and case independent of Rust's
//! Unicode version (notably final sigma, dotted I, new letters and numeric marks).

const LOWER: &[u8] = include_bytes!("search_data/lower.bin");
const WORD: &[u8] = include_bytes!("search_data/word.bin");
const HAN: &[u8] = include_bytes!("search_data/han.bin");
const PINYIN: &[u8] = include_bytes!("search_data/pinyin.bin");
const SYLLABLES: &[u8] = include_bytes!("search_data/syllables.bin");

fn number(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn entry(table: &[u8], c: char) -> Option<usize> {
    let count = table.len() / 8;
    // Tables contain sorted pairs of u32 values, with no runtime map allocation.
    let mut low = 0;
    let mut high = count;
    while low < high {
        let mid = low + (high - low) / 2;
        if number(table, mid * 8) < c as u32 {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    (low < count && number(table, low * 8) == c as u32).then_some(low * 8)
}

pub fn lower(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
    entry(LOWER, c)
        .and_then(|at| char::from_u32(number(LOWER, at + 4)))
        .unwrap_or(c)
}

fn in_ranges(table: &[u8], c: char) -> bool {
    let mut low = 0;
    let mut high = table.len() / 8;
    while low < high {
        let mid = low + (high - low) / 2;
        if number(table, mid * 8 + 4) < c as u32 {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low < table.len() / 8 && number(table, low * 8) <= c as u32
}

pub(crate) fn word(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric();
    }
    in_ranges(WORD, c)
}
pub(crate) fn han(c: char) -> bool {
    in_ranges(HAN, c)
}

pub(crate) fn pronunciation(c: char) -> Option<&'static str> {
    let at = entry(PINYIN, c)?;
    let tail = &SYLLABLES[number(PINYIN, at + 4) as usize..];
    let end = tail.iter().position(|b| *b == 0)?;
    std::str::from_utf8(&tail[..end]).ok()
}

pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .map(|word| word.chars().map(lower).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}
