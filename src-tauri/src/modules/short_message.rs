use super::short_message_table_packs::{active_pack, fallback_alphabets, ValidatedTablePack};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// Fixed-size 32-byte short-message frame with a two-byte "HS" prefix.
const FRAME_SIZE: usize = 32;
const HEX_FRAME_LEN: usize = FRAME_SIZE * 2;
const MAX_TEXT_CHARS: usize = 512;
const PAYLOAD_MAX: usize = 27;

const MAGIC_0: u8 = 0x48;
const MAGIC_1: u8 = 0x53;
const FORMAT_VERSION: u8 = 0x07;

const DICT_SHIFT: u8 = 5;
const VERSION_MASK: u8 = 0x07;
const MODE_MASK: u8 = 0x18;

const MODE_DICT: u8 = 0x00;
const MODE_RAW: u8 = 0x08;
const MODE_6BIT: u8 = 0x10;
const MODE_5BIT: u8 = 0x18;

const PAYLOAD_LEN_INDEX: usize = 3;
const PAYLOAD_INDEX: usize = 4;
const CRC_INDEX: usize = FRAME_SIZE - 1;
const DICT_LITERAL_ESCAPE: u8 = 255;
const DICT_DIGIT_RUN_BASE: u8 = 0x80;
const DICT_NUMERIC_RUN_BASE: u8 = 0xc0;
const DICT_DIGIT_RUN_LEN_MASK: u8 = 0x3f;

fn is_printable_ascii(byte: u8) -> bool {
    (32..=126).contains(&byte)
}

fn is_compact_punctuation(byte: u8) -> bool {
    matches!(byte, b',' | b'.' | b'!' | b'?' | b';' | b':')
}

fn is_sentence_boundary(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '¿' | '\n' | '\r')
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if (crc & 0x80) != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[derive(Debug, Clone)]
struct Candidate {
    mode: u8,
    dict_idx: u8,
    payload: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct DictionaryRuntime {
    pub(crate) normalized: Vec<String>,
    pub(crate) match_order: Vec<u8>,
    suffix_order: Vec<u8>,
    pub(crate) name: String,
}

pub(crate) struct ActiveRuntime {
    runtimes: Vec<DictionaryRuntime>,
    suffix_set: std::collections::HashSet<String>,
    acronym_set: std::collections::HashSet<String>,
    dict_names: Vec<String>,
    #[allow(dead_code)]
    origin: super::short_message_table_packs::TablePackOrigin,
    #[allow(dead_code)]
    pack_name: String,
    #[allow(dead_code)]
    pack_version: String,
    pack_fingerprint: String,
}

static ACTIVE_RUNTIME: OnceLock<std::sync::Mutex<Option<Arc<ActiveRuntime>>>> = OnceLock::new();

fn runtime_cache_cell() -> &'static std::sync::Mutex<Option<Arc<ActiveRuntime>>> {
    ACTIVE_RUNTIME.get_or_init(|| std::sync::Mutex::new(None))
}

fn current_runtime() -> Arc<ActiveRuntime> {
    let view = active_runtime_view();
    let mut guard = runtime_cache_cell().lock().expect("runtime cache poisoned");
    let needs_refresh = match guard.as_ref() {
        Some(cached) => cached.pack_fingerprint != view.pack_fingerprint,
        None => true,
    };
    if needs_refresh {
        *guard = Some(Arc::new(view));
    }
    Arc::clone(guard.as_ref().expect("just set"))
}

fn is_suffix_phrase(phrase: &str) -> bool {
    let runtime = current_runtime();
    runtime.suffix_set.contains(phrase)
}

fn build_dictionary_runtimes(pack: &ValidatedTablePack) -> Vec<DictionaryRuntime> {
    let suffix_set: std::collections::HashSet<&str> =
        pack.suffixes.iter().map(|s| s.as_str()).collect();

    pack.dictionaries
        .iter()
        .enumerate()
        .map(|(dict_idx, dict)| {
            let normalized: Vec<String> = dict.clone();
            let mut match_order: Vec<u8> = normalized
                .iter()
                .enumerate()
                .filter_map(|(idx, phrase)| {
                    (idx != DICT_LITERAL_ESCAPE as usize && !phrase.is_empty()).then_some(idx as u8)
                })
                .collect();
            match_order.sort_by(|a, b| {
                normalized[*b as usize]
                    .len()
                    .cmp(&normalized[*a as usize].len())
                    .then((*a).cmp(b))
            });
            let mut suffix_order: Vec<u8> = match_order
                .iter()
                .copied()
                .filter(|tok| suffix_set.contains(normalized[*tok as usize].as_str()))
                .collect();
            suffix_order.sort_by(|a, b| {
                normalized[*b as usize]
                    .len()
                    .cmp(&normalized[*a as usize].len())
                    .then((*a).cmp(b))
            });
            let name = pack
                .dictionary_names
                .get(dict_idx)
                .cloned()
                .unwrap_or_else(|| ((b'A' + dict_idx as u8) as char).to_string());
            DictionaryRuntime {
                normalized,
                match_order,
                suffix_order,
                name,
            }
        })
        .collect()
}

fn active_runtime_view() -> ActiveRuntime {
    let pack = active_pack();
    let runtimes = build_dictionary_runtimes(&pack);
    let suffix_set: std::collections::HashSet<String> = pack.suffixes.iter().cloned().collect();
    let acronym_set: std::collections::HashSet<String> = pack.acronyms.iter().cloned().collect();
    let dict_names: Vec<String> = pack.dictionary_names.clone();
    ActiveRuntime {
        runtimes,
        suffix_set,
        acronym_set,
        dict_names,
        origin: pack.origin,
        pack_name: pack.name.clone(),
        pack_version: pack.version.clone(),
        pack_fingerprint: pack.fingerprint_sha256.clone(),
    }
}

pub(crate) fn normalize_text_for_autocomplete(text: &str) -> String {
    normalize(text).0
}

pub(crate) fn restore_case_pub(raw: &str) -> String {
    restore_case(raw)
}

fn fallback_alphabet_rev(alphabet: &[u8]) -> HashMap<u8, u8> {
    let mut map = HashMap::with_capacity(alphabet.len());
    for (idx, &ch) in alphabet.iter().enumerate() {
        map.insert(ch, idx as u8);
    }
    map
}

fn alphabet_5bit_rev() -> HashMap<u8, u8> {
    let (a5, _a6) = fallback_alphabets();
    fallback_alphabet_rev(a5)
}

fn alphabet_6bit_rev() -> HashMap<u8, u8> {
    let (_a5, a6) = fallback_alphabets();
    fallback_alphabet_rev(a6)
}

fn normalize(input: &str) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let mut saw_control = false;
    let mut out = String::with_capacity(input.len());

    for ch in input.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{2192}' => out.push_str("->"),
            '\u{2190}' => out.push_str("<-"),
            c if c.is_control() => {
                saw_control = true;
                out.push(' ');
            }
            c if c.is_whitespace() => out.push(' '),
            c => {
                for lower in c.to_lowercase() {
                    out.push(lower);
                }
            }
        }
    }

    if saw_control {
        warnings.push("Unsupported control characters were replaced with spaces".to_string());
    }

    (collapse(&out), warnings)
}

fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut saw_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            saw_space = true;
            continue;
        }

        if matches!(ch, ',' | '.' | '!' | '?' | ';' | ':') {
            if out.ends_with(' ') {
                out.pop();
            }
            out.push(ch);
            saw_space = false;
            continue;
        }

        if saw_space && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(ch);
        saw_space = false;
    }

    out.trim().to_string()
}

fn should_extend_word(ch: char, prev_was_word: bool) -> bool {
    ch.is_alphanumeric() || (ch == '\'' && prev_was_word)
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().to_string();
    out.push_str(chars.as_str());
    out
}

fn render_word(word: &str, sentence_start: bool) -> String {
    let lower = word.to_lowercase();
    if lower == "i" {
        "I".to_string()
    } else if lower.starts_with("i'") {
        let mut chars = lower.chars();
        let _ = chars.next();
        format!("I{}", chars.as_str())
    } else if lower == "hemp0x" {
        lower
    } else if current_runtime().acronym_set.contains(&lower) {
        lower.to_uppercase()
    } else if sentence_start {
        capitalize_word(&lower)
    } else {
        lower
    }
}

fn flush_word(word: &mut String, sentence_start: bool, out: &mut String) -> bool {
    if word.is_empty() {
        return sentence_start;
    }

    out.push_str(&render_word(word, sentence_start));
    word.clear();
    false
}

fn restore_case(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(raw.len());
    let mut word = String::new();
    let mut sentence_start = true;

    let mut prev_was_word = false;

    for ch in raw.chars() {
        if should_extend_word(ch, prev_was_word) {
            word.push(ch);
            prev_was_word = ch.is_alphanumeric();
            continue;
        }

        sentence_start = flush_word(&mut word, sentence_start, &mut out);

        out.push(ch);
        if is_sentence_boundary(ch) {
            sentence_start = true;
        }
        prev_was_word = false;
    }

    let _ = flush_word(&mut word, sentence_start, &mut out);

    out
}

#[derive(Serialize, Debug, Clone)]
pub struct ShortMessageEncodeResult {
    pub hex: String,
    pub decoded_preview: String,
    pub normalized_text: String,
    pub raw_len: usize,
    pub encoded_payload_len: usize,
    pub encoding_mode: String,
    pub dictionary_index: Option<u8>,
    pub dictionary_name: Option<String>,
    pub fits: bool,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ShortMessageDecodeResult {
    pub is_short_message: bool,
    pub text: Option<String>,
    pub version: Option<u8>,
    pub warnings: Vec<String>,
}

fn find_dict_match(
    bytes: &[u8],
    offset: usize,
    runtime: &DictionaryRuntime,
) -> Option<(u8, usize)> {
    let remaining = &bytes[offset..];
    for &tok in &runtime.match_order {
        let phrase_bytes = runtime.normalized[tok as usize].as_bytes();
        if remaining.starts_with(phrase_bytes) {
            return Some((tok, phrase_bytes.len()));
        }

        // Allow terminal phrases stored with a trailing space to also match at
        // end-of-message. The decoded text is collapsed later, so this recovers
        // codebook utility without needing duplicate no-space dictionary terms.
        if phrase_bytes.len() > 1
            && phrase_bytes.ends_with(b" ")
            && remaining == &phrase_bytes[..phrase_bytes.len() - 1]
        {
            return Some((tok, remaining.len()));
        }

        // Normalization removes spaces before punctuation, but most word and
        // phrase dictionary entries intentionally carry trailing spaces. Let
        // those entries still match directly before punctuation so "thank you!"
        // can encode as "thank you " + "!" instead of falling back to literals.
        if phrase_bytes.len() > 1
            && phrase_bytes.ends_with(b" ")
            && remaining.len() >= phrase_bytes.len()
            && remaining[..phrase_bytes.len() - 1] == phrase_bytes[..phrase_bytes.len() - 1]
            && is_compact_punctuation(remaining[phrase_bytes.len() - 1])
        {
            return Some((tok, phrase_bytes.len() - 1));
        }
    }
    None
}

fn suffix_match_len(remaining: &[u8], suffix_bytes: &[u8]) -> Option<usize> {
    if remaining.starts_with(suffix_bytes) {
        return Some(suffix_bytes.len());
    }

    if suffix_bytes.len() <= 1 || !suffix_bytes.ends_with(b" ") {
        return None;
    }

    let suffix_without_space = &suffix_bytes[..suffix_bytes.len() - 1];
    if remaining == suffix_without_space {
        return Some(remaining.len());
    }

    if remaining.len() >= suffix_bytes.len()
        && remaining[..suffix_without_space.len()] == *suffix_without_space
        && is_compact_punctuation(remaining[suffix_without_space.len()])
    {
        return Some(suffix_without_space.len());
    }

    None
}

fn find_stem_suffix_match(
    bytes: &[u8],
    offset: usize,
    runtime: &DictionaryRuntime,
) -> Option<(u8, u8, usize)> {
    let remaining = &bytes[offset..];
    for &stem_tok in &runtime.match_order {
        let stem_bytes = runtime.normalized[stem_tok as usize].as_bytes();
        if stem_bytes.len() <= 1 || !stem_bytes.ends_with(b" ") {
            continue;
        }

        let stem_without_space = &stem_bytes[..stem_bytes.len() - 1];
        if !remaining.starts_with(stem_without_space) {
            continue;
        }

        let suffix_offset = stem_without_space.len();
        let suffix_remaining = &remaining[suffix_offset..];
        if suffix_remaining.is_empty() {
            continue;
        }

        for &suffix_tok in &runtime.suffix_order {
            let suffix_bytes = runtime.normalized[suffix_tok as usize].as_bytes();
            if let Some(suffix_len) = suffix_match_len(suffix_remaining, suffix_bytes) {
                return Some((stem_tok, suffix_tok, suffix_offset + suffix_len));
            }
        }
    }
    None
}

fn push_ascii_literal(payload: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    if bytes.is_empty() || bytes.iter().any(|&byte| !is_printable_ascii(byte)) {
        return None;
    }

    if bytes.len() == 1 {
        if payload.len() + 2 > PAYLOAD_MAX {
            return None;
        }
        payload.push(DICT_LITERAL_ESCAPE);
        payload.push(bytes[0]);
        return Some(());
    }

    if bytes.len() > u8::MAX as usize || payload.len() + 2 + bytes.len() > PAYLOAD_MAX {
        return None;
    }
    payload.push(DICT_LITERAL_ESCAPE);
    payload.push(bytes.len() as u8);
    payload.extend_from_slice(bytes);
    Some(())
}

fn push_digit_literal(payload: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    if bytes.len() < 3
        || bytes.len() > DICT_DIGIT_RUN_LEN_MASK as usize
        || bytes.iter().any(|byte| !byte.is_ascii_digit())
    {
        return None;
    }

    let packed_len = bytes.len().div_ceil(2);
    if payload.len() + 2 + packed_len > PAYLOAD_MAX {
        return None;
    }

    payload.push(DICT_LITERAL_ESCAPE);
    payload.push(DICT_DIGIT_RUN_BASE | bytes.len() as u8);
    for chunk in bytes.chunks(2) {
        let hi = chunk[0] - b'0';
        let lo = chunk.get(1).map_or(0x0f, |byte| byte - b'0');
        payload.push((hi << 4) | lo);
    }
    Some(())
}

fn numeric_symbol_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'.' => Some(10),
        b'/' => Some(11),
        b':' => Some(12),
        b'-' => Some(13),
        b',' => Some(14),
        _ => None,
    }
}

fn numeric_symbol_byte(nibble: u8) -> Option<u8> {
    match nibble {
        0..=9 => Some(b'0' + nibble),
        10 => Some(b'.'),
        11 => Some(b'/'),
        12 => Some(b':'),
        13 => Some(b'-'),
        14 => Some(b','),
        _ => None,
    }
}

fn push_numeric_literal(payload: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    if bytes.len() < 3
        || bytes.len() > DICT_DIGIT_RUN_LEN_MASK as usize
        || !bytes.iter().any(|byte| byte.is_ascii_digit())
        || bytes
            .iter()
            .any(|&byte| numeric_symbol_nibble(byte).is_none())
    {
        return None;
    }

    let packed_len = bytes.len().div_ceil(2);
    if payload.len() + 2 + packed_len > PAYLOAD_MAX {
        return None;
    }

    payload.push(DICT_LITERAL_ESCAPE);
    payload.push(DICT_NUMERIC_RUN_BASE | bytes.len() as u8);
    for chunk in bytes.chunks(2) {
        let hi = numeric_symbol_nibble(chunk[0])?;
        let lo = chunk
            .get(1)
            .map_or(0x0f, |byte| numeric_symbol_nibble(*byte).unwrap_or(0x0f));
        payload.push((hi << 4) | lo);
    }
    Some(())
}

fn digit_run_len(bytes: &[u8], offset: usize) -> usize {
    bytes[offset..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count()
}

fn numeric_run_len(bytes: &[u8], offset: usize) -> usize {
    let mut has_digit = false;
    let mut len = 0;
    for &byte in &bytes[offset..] {
        if numeric_symbol_nibble(byte).is_none() {
            break;
        }
        has_digit |= byte.is_ascii_digit();
        len += 1;
    }
    if has_digit {
        len
    } else {
        0
    }
}

fn push_dict_literal(payload: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    if bytes.is_empty() || bytes.iter().any(|&byte| !is_printable_ascii(byte)) {
        return None;
    }

    let mut i = 0;
    while i < bytes.len() {
        let numeric = numeric_run_len(bytes, i);
        let digits = digit_run_len(bytes, i);
        if numeric >= 3 && numeric > digits {
            push_numeric_literal(payload, &bytes[i..i + numeric])?;
            i += numeric;
            continue;
        }
        if digits >= 3 {
            push_digit_literal(payload, &bytes[i..i + digits])?;
            i += digits;
            continue;
        }

        let literal_start = i;
        i += digits.max(1);
        while i < bytes.len() {
            let next_numeric = numeric_run_len(bytes, i);
            if next_numeric >= 3 {
                break;
            }
            let next_digits = digit_run_len(bytes, i);
            if next_digits >= 3 {
                break;
            }
            i += next_digits.max(1);
        }
        push_ascii_literal(payload, &bytes[literal_start..i])?;
    }
    Some(())
}

fn encode_dict(text: &str, runtime: &DictionaryRuntime) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut payload = Vec::with_capacity(PAYLOAD_MAX);
    let mut i = 0;

    while i < bytes.len() {
        if let Some((tok, len)) = find_dict_match(bytes, i, runtime) {
            debug_assert!(len > 0, "dictionary token made no progress");
            if payload.len() + 1 > PAYLOAD_MAX {
                return None;
            }
            payload.push(tok);
            i += len;
            continue;
        }

        if let Some((stem_tok, suffix_tok, len)) = find_stem_suffix_match(bytes, i, runtime) {
            debug_assert!(len > 0, "stem+suffix match made no progress");
            if payload.len() + 2 > PAYLOAD_MAX {
                return None;
            }
            payload.push(stem_tok);
            payload.push(suffix_tok);
            i += len;
            continue;
        }

        // Keep compact number-like strings together. Without this, a value
        // like "10.50" can split around the dictionary "." token and lose the
        // packed numeric-run encoding.
        let numeric = numeric_run_len(bytes, i);
        let digits = digit_run_len(bytes, i);
        if numeric >= 3 && numeric > digits {
            push_numeric_literal(&mut payload, &bytes[i..i + numeric])?;
            i += numeric;
            continue;
        }
        if digits >= 3 {
            push_digit_literal(&mut payload, &bytes[i..i + digits])?;
            i += digits;
            continue;
        }

        let literal_start = i;
        while i < bytes.len() {
            if !is_printable_ascii(bytes[i]) {
                return None;
            }
            if i > literal_start
                && (find_dict_match(bytes, i, runtime).is_some()
                    || find_stem_suffix_match(bytes, i, runtime).is_some())
            {
                break;
            }
            i += 1;
        }
        push_dict_literal(&mut payload, &bytes[literal_start..i])?;
    }

    Some(payload)
}

fn encode_raw(text: &str) -> Option<Vec<u8>> {
    if text.len() > PAYLOAD_MAX || !text.is_ascii() {
        return None;
    }
    if text.as_bytes().iter().any(|&b| !is_printable_ascii(b)) {
        return None;
    }
    Some(text.as_bytes().to_vec())
}

fn encode_nbit(
    text: &str,
    rev: &HashMap<u8, u8>,
    bits_per: usize,
    alphabet_max_chars: usize,
) -> Option<Vec<u8>> {
    if text.len() > alphabet_max_chars || !text.is_ascii() {
        return None;
    }

    let idxs: Vec<u8> = text
        .as_bytes()
        .iter()
        .map(|c| rev.get(c).copied())
        .collect::<Option<_>>()?;
    let total_bits = idxs.len() * bits_per;
    let len = total_bits.div_ceil(8);
    let mut payload = vec![0u8; len];

    for (i, &idx) in idxs.iter().enumerate() {
        let off = i * bits_per;
        let byte = off / 8;
        let shift = off % 8;
        let v = (idx as u16) << shift;
        payload[byte] |= v as u8;
        if byte + 1 < len {
            payload[byte + 1] |= (v >> 8) as u8;
        }
    }

    Some(payload)
}

fn decode_dict(payload: &[u8], runtime: &DictionaryRuntime) -> Option<String> {
    let mut out = String::with_capacity(payload.len() * 4);
    let mut i = 0;
    while i < payload.len() {
        let tok = payload[i];
        if tok == DICT_LITERAL_ESCAPE {
            let marker = *payload.get(i + 1)?;
            if is_printable_ascii(marker) {
                out.push(marker as char);
                i += 2;
                continue;
            }

            if marker & DICT_NUMERIC_RUN_BASE == DICT_NUMERIC_RUN_BASE {
                let len = (marker & DICT_DIGIT_RUN_LEN_MASK) as usize;
                let packed_len = len.div_ceil(2);
                if len == 0 || i + 2 + packed_len > payload.len() {
                    return None;
                }

                for digit_idx in 0..len {
                    let packed = payload[i + 2 + digit_idx / 2];
                    let nibble = if digit_idx % 2 == 0 {
                        packed >> 4
                    } else {
                        packed & 0x0f
                    };
                    out.push(numeric_symbol_byte(nibble)? as char);
                }
                if len % 2 == 1 && (payload[i + 2 + packed_len - 1] & 0x0f) != 0x0f {
                    return None;
                }
                i += 2 + packed_len;
                continue;
            }

            if marker & DICT_DIGIT_RUN_BASE == DICT_DIGIT_RUN_BASE {
                let len = (marker & DICT_DIGIT_RUN_LEN_MASK) as usize;
                let packed_len = len.div_ceil(2);
                if len == 0 || i + 2 + packed_len > payload.len() {
                    return None;
                }

                for digit_idx in 0..len {
                    let packed = payload[i + 2 + digit_idx / 2];
                    let nibble = if digit_idx % 2 == 0 {
                        packed >> 4
                    } else {
                        packed & 0x0f
                    };
                    if nibble > 9 {
                        return None;
                    }
                    out.push((b'0' + nibble) as char);
                }
                if len % 2 == 1 && (payload[i + 2 + packed_len - 1] & 0x0f) != 0x0f {
                    return None;
                }
                i += 2 + packed_len;
                continue;
            }

            let len = marker as usize;
            if len == 0 || i + 2 + len > payload.len() {
                return None;
            }
            let literal = &payload[i + 2..i + 2 + len];
            if literal.iter().any(|&byte| !is_printable_ascii(byte)) {
                return None;
            }
            out.push_str(std::str::from_utf8(literal).ok()?);
            i += 2 + len;
            continue;
        }

        let phrase = runtime.normalized.get(tok as usize)?;
        if phrase.is_empty() {
            return None;
        }
        if is_suffix_phrase(phrase) && out.ends_with(' ') {
            out.pop();
        }
        out.push_str(phrase);
        i += 1;
    }
    Some(out)
}

fn decode_nbit(payload: &[u8], alphabet: &[u8], bits_per: usize) -> Option<String> {
    if payload.is_empty() {
        return Some(String::new());
    }

    let total_bits = payload.len() * 8;
    let count = total_bits / bits_per;
    let mut out = Vec::with_capacity(count);
    let mut buf: u32 = 0;
    let mut have: usize = 0;
    let mut pi = 0;

    while out.len() < count {
        if have < bits_per && pi < payload.len() {
            buf |= (payload[pi] as u32) << have;
            have += 8;
            pi += 1;
        }

        if have < bits_per {
            break;
        }

        let idx = (buf & ((1u32 << bits_per) - 1)) as usize;
        if idx >= alphabet.len() {
            return None;
        }

        out.push(alphabet[idx]);
        buf >>= bits_per;
        have -= bits_per;
    }

    while out.last() == Some(&b' ') {
        out.pop();
    }

    Some(String::from_utf8_lossy(&out).into_owned())
}

fn candidate_sort_key(candidate: &Candidate) -> (usize, u8, u8) {
    (
        candidate.payload.len(),
        mode_priority(candidate.mode),
        candidate.dict_idx,
    )
}

fn mode_priority(mode: u8) -> u8 {
    match mode {
        MODE_DICT => 0,
        MODE_RAW => 1,
        MODE_5BIT => 2,
        MODE_6BIT => 3,
        _ => 4,
    }
}

fn mode_label(mode: u8) -> &'static str {
    match mode {
        MODE_DICT => "dictionary",
        MODE_RAW => "raw",
        MODE_5BIT => "5bit",
        MODE_6BIT => "6bit",
        _ => "unknown",
    }
}

fn select_candidate(text: &str) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    let runtime = current_runtime();
    let runtimes = runtime.runtimes.clone();
    let rev5 = alphabet_5bit_rev();
    let rev6 = alphabet_6bit_rev();

    let mut consider = |candidate: Candidate| {
        let take = match &best {
            None => true,
            Some(current) => candidate_sort_key(&candidate) < candidate_sort_key(current),
        };

        if take {
            best = Some(candidate);
        }
    };

    for (dict_idx, rt) in runtimes.iter().enumerate() {
        if let Some(payload) = encode_dict(text, rt) {
            consider(Candidate {
                mode: MODE_DICT,
                dict_idx: dict_idx as u8,
                payload,
            });
        }
    }

    if let Some(payload) = encode_raw(text) {
        consider(Candidate {
            mode: MODE_RAW,
            dict_idx: 0,
            payload,
        });
    }

    if let Some(payload) = encode_nbit(text, &rev5, 5, PAYLOAD_MAX * 8 / 5) {
        consider(Candidate {
            mode: MODE_5BIT,
            dict_idx: 0,
            payload,
        });
    }

    if let Some(payload) = encode_nbit(text, &rev6, 6, PAYLOAD_MAX * 8 / 6) {
        consider(Candidate {
            mode: MODE_6BIT,
            dict_idx: 0,
            payload,
        });
    }

    best
}

fn build_header(dict_idx: u8, mode: u8) -> u8 {
    ((dict_idx & 0x07) << DICT_SHIFT) | (mode & MODE_MASK) | (FORMAT_VERSION & VERSION_MASK)
}

fn frame_hex(frame: &[u8; FRAME_SIZE]) -> String {
    frame.iter().map(|b| format!("{:02x}", b)).collect()
}

fn invalid_decode(version: Option<u8>, warning: impl Into<String>) -> ShortMessageDecodeResult {
    ShortMessageDecodeResult {
        is_short_message: false,
        text: None,
        version,
        warnings: vec![warning.into()],
    }
}

fn not_short_message() -> ShortMessageDecodeResult {
    ShortMessageDecodeResult {
        is_short_message: false,
        text: None,
        version: None,
        warnings: vec![],
    }
}

fn decode_payload(header: u8, payload: &[u8]) -> Option<String> {
    let mode = header & MODE_MASK;
    let dict_idx = (header >> DICT_SHIFT) as usize;
    let (a5, a6) = fallback_alphabets();

    match mode {
        MODE_DICT => current_runtime()
            .runtimes
            .get(dict_idx)
            .and_then(|runtime| decode_dict(payload, runtime)),
        MODE_RAW => {
            if payload.iter().all(|&b| is_printable_ascii(b)) {
                Some(String::from_utf8_lossy(payload).into_owned())
            } else {
                None
            }
        }
        MODE_5BIT => decode_nbit(payload, a5, 5),
        MODE_6BIT => decode_nbit(payload, a6, 6),
        _ => None,
    }
}

pub fn encode(text: &str) -> Result<ShortMessageEncodeResult, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Message is empty".to_string());
    }
    if trimmed.len() > MAX_TEXT_CHARS {
        return Err(format!("Message exceeds {} characters", MAX_TEXT_CHARS));
    }

    let (normalized_text, warnings) = normalize(trimmed);
    if normalized_text.is_empty() {
        return Err("Message is empty after normalization".to_string());
    }

    let candidate = select_candidate(&normalized_text)
        .ok_or_else(|| "Message too long for any encoding mode".to_string())?;

    if candidate.payload.len() > PAYLOAD_MAX {
        return Err(format!(
            "Message too long: {} bytes required, max is {}",
            candidate.payload.len(),
            PAYLOAD_MAX
        ));
    }

    let mut frame = [0u8; FRAME_SIZE];
    frame[0] = MAGIC_0;
    frame[1] = MAGIC_1;
    frame[2] = build_header(candidate.dict_idx, candidate.mode);
    frame[PAYLOAD_LEN_INDEX] = candidate.payload.len() as u8;
    frame[PAYLOAD_INDEX..PAYLOAD_INDEX + candidate.payload.len()]
        .copy_from_slice(&candidate.payload);
    frame[CRC_INDEX] = crc8(&frame[0..CRC_INDEX]);

    let hex = frame_hex(&frame);
    let decoded_raw = decode_payload(frame[2], &candidate.payload).unwrap_or_default();
    let decoded_preview = restore_case(&collapse(&decoded_raw));

    Ok(ShortMessageEncodeResult {
        hex,
        decoded_preview,
        normalized_text,
        raw_len: text.len(),
        encoded_payload_len: candidate.payload.len(),
        encoding_mode: mode_label(candidate.mode).to_string(),
        dictionary_index: (candidate.mode == MODE_DICT).then_some(candidate.dict_idx),
        dictionary_name: (candidate.mode == MODE_DICT).then(|| {
            let runtime = current_runtime();
            runtime
                .dict_names
                .get(candidate.dict_idx as usize)
                .cloned()
                .unwrap_or_else(|| {
                    runtime
                        .runtimes
                        .get(candidate.dict_idx as usize)
                        .map(|r| r.name.clone())
                        .unwrap_or_default()
                })
        }),
        fits: true,
        warnings,
    })
}

pub(crate) fn warm_runtime_cache() {
    let _ = current_runtime();
}

pub fn decode(hex: &str) -> ShortMessageDecodeResult {
    let trimmed = hex.trim();
    let trimmed = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        &trimmed[2..]
    } else {
        trimmed
    };

    if trimmed.len() != HEX_FRAME_LEN {
        return invalid_decode(
            None,
            format!("Hex string must be exactly {} characters", HEX_FRAME_LEN),
        );
    }

    let bytes = match hex::decode(trimmed) {
        Ok(bytes) if bytes.len() == FRAME_SIZE => bytes,
        _ => return invalid_decode(None, "Invalid hex input"),
    };

    if bytes[0] != MAGIC_0 || bytes[1] != MAGIC_1 {
        return not_short_message();
    }

    let header = bytes[2];
    let version = header & VERSION_MASK;
    if version != FORMAT_VERSION {
        return invalid_decode(
            Some(version),
            format!("Unknown short message version: {}", version),
        );
    }

    let mode = header & MODE_MASK;
    if !matches!(mode, MODE_DICT | MODE_RAW | MODE_5BIT | MODE_6BIT) {
        return invalid_decode(Some(version), format!("Unknown mode: {:#04x}", mode));
    }

    let plen = bytes[PAYLOAD_LEN_INDEX] as usize;
    if plen > PAYLOAD_MAX || PAYLOAD_INDEX + plen > CRC_INDEX {
        return invalid_decode(
            Some(version),
            format!("Payload length {} exceeds max {}", plen, PAYLOAD_MAX),
        );
    }

    if crc8(&bytes[0..CRC_INDEX]) != bytes[CRC_INDEX] {
        return invalid_decode(Some(version), "CRC-8 mismatch");
    }

    let payload = &bytes[PAYLOAD_INDEX..PAYLOAD_INDEX + plen];
    let raw = match decode_payload(header, payload) {
        Some(raw) => raw,
        None => return invalid_decode(Some(version), "Payload decode failed"),
    };

    ShortMessageDecodeResult {
        is_short_message: true,
        text: Some(restore_case(&collapse(&raw))),
        version: Some(version),
        warnings: vec![],
    }
}

#[tauri::command]
pub fn short_message_encode(text: String) -> Result<ShortMessageEncodeResult, String> {
    encode(&text)
}

#[tauri::command]
pub fn short_message_decode(hex: String) -> ShortMessageDecodeResult {
    decode(&hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::short_message_table_packs::built_in_table_pack;

    fn decode_frame(text: &str) -> ShortMessageDecodeResult {
        let enc = encode(text).expect("encode");
        assert_eq!(enc.hex.len(), 64);
        decode(&enc.hex)
    }

    fn frame_bytes(text: &str) -> Vec<u8> {
        hex::decode(encode(text).expect("encode").hex).expect("hex")
    }

    fn dict_payload(dict_idx: usize, text: &str) -> Vec<u8> {
        let (normalized, _) = normalize(text);
        encode_dict(&normalized, &current_runtime().runtimes[dict_idx]).unwrap_or_else(|| {
            panic!("dictionary payload failed for dict {dict_idx}: {normalized:?}")
        })
    }

    fn best_dict_payload(text: &str) -> (usize, Vec<u8>) {
        let (normalized, _) = normalize(text);
        current_runtime()
            .runtimes
            .iter()
            .enumerate()
            .filter_map(|(idx, runtime)| {
                encode_dict(&normalized, runtime).map(|payload| (idx, payload))
            })
            .min_by_key(|(_, payload)| payload.len())
            .unwrap_or_else(|| panic!("no dictionary payload for {normalized:?}"))
    }

    fn decode_dict_preview(dict_idx: usize, payload: &[u8]) -> String {
        let decoded =
            decode_dict(payload, &current_runtime().runtimes[dict_idx]).expect("decode dict");
        restore_case(&collapse(&decoded))
    }

    fn header_mode(bytes: &[u8]) -> u8 {
        bytes[2] & MODE_MASK
    }

    fn header_dict(bytes: &[u8]) -> usize {
        (bytes[2] >> DICT_SHIFT) as usize
    }

    #[test]
    fn dictionary_shapes() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let pack = built_in_table_pack();
        assert!(
            pack.dictionaries.len() <= 8,
            "header stores dictionary index in three bits"
        );

        for (idx, dict) in pack.dictionaries.iter().enumerate() {
            let name = ((b'A' + idx as u8) as char).to_string();
            assert!(
                dict.len() <= 256,
                "dictionary {name} exceeds one-byte token space"
            );
            for (tok, phrase) in dict.iter().enumerate() {
                if tok == DICT_LITERAL_ESCAPE as usize {
                    assert!(
                        phrase.is_empty(),
                        "dictionary {name} token 255 must stay empty"
                    );
                } else {
                    assert!(
                        !phrase.is_empty(),
                        "dictionary {name} token {tok} is unexpectedly empty"
                    );
                }
            }
        }
    }

    #[test]
    fn roundtrip_general_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("i think the hemp token launch is soon");
        assert!(dec.is_short_message);
        assert_eq!(
            dec.text.as_deref(),
            Some("I think the HEMP token launch is soon")
        );
    }

    #[test]
    fn restores_case_and_acronyms() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("payment sent. check cid and ipfs.");
        assert_eq!(
            dec.text.as_deref(),
            Some("Payment sent. Check CID and IPFS.")
        );

        let dec = decode_frame("this is a test of the hemp0x on chain message system.");
        assert_eq!(
            dec.text.as_deref(),
            Some("This is a test of the hemp0x on chain message system.")
        );
    }

    #[test]
    fn selects_business_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("invoice paid payment received amount due");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 3);
    }

    #[test]
    fn encode_result_reports_active_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let enc = encode("invoice paid payment received amount due").expect("encode");
        assert_eq!(enc.encoding_mode, "dictionary");
        assert_eq!(enc.dictionary_index, Some(3));
        assert_eq!(enc.dictionary_name.as_deref(), Some("D"));
    }

    #[test]
    fn dictionary_mode_allows_short_literal_gaps() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let payload = dict_payload(1, "payment sent bob");
        assert!(payload.contains(&DICT_LITERAL_ESCAPE));
        assert_eq!(decode_dict_preview(1, &payload), "Payment sent bob");
    }

    #[test]
    fn dictionary_mode_packs_literal_runs() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let payload = dict_payload(1, "payment sent bob123");
        assert!(payload.contains(&DICT_LITERAL_ESCAPE));
        assert_eq!(decode_dict_preview(1, &payload), "Payment sent bob123");
    }

    #[test]
    fn dictionary_mode_packs_digit_runs() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let payload = dict_payload(1, "payment sent invoice 123456789");
        let digit_marker = DICT_DIGIT_RUN_BASE | 9;
        assert!(payload
            .windows(2)
            .any(|window| window == [DICT_LITERAL_ESCAPE, digit_marker]));
        assert_eq!(
            decode_dict_preview(1, &payload),
            "Payment sent invoice 123456789"
        );
    }

    #[test]
    fn dictionary_mode_packs_numeric_symbol_runs() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let payload = dict_payload(1, "payment sent amount 10.50");
        let numeric_marker = DICT_NUMERIC_RUN_BASE | 5;
        assert!(payload
            .windows(2)
            .any(|window| window == [DICT_LITERAL_ESCAPE, numeric_marker]));
        assert_eq!(
            decode_dict_preview(1, &payload),
            "Payment sent amount 10.50"
        );
    }

    #[test]
    fn dictionary_mode_prefers_full_numeric_symbol_runs() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let payload = dict_payload(1, "payment sent amount 1000.25");
        let numeric_marker = DICT_NUMERIC_RUN_BASE | 7;
        assert!(payload
            .windows(2)
            .any(|window| window == [DICT_LITERAL_ESCAPE, numeric_marker]));
        assert_eq!(
            decode_dict_preview(1, &payload),
            "Payment sent amount 1000.25"
        );
    }

    #[test]
    fn dictionary_phrases_match_before_punctuation() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let plain = dict_payload(1, "thank you");
        let excited = dict_payload(1, "thank you!");
        assert_eq!(plain.len(), 1);
        assert_eq!(excited.len(), 2);
        assert_eq!(decode_dict_preview(1, &excited), "Thank you!");

        let auto = encode("thank you!").expect("encode excited");
        assert_eq!(auto.encoded_payload_len, 2);
        assert_eq!(auto.encoding_mode, "dictionary");
        let dec = decode(&auto.hex);
        assert_eq!(dec.text.as_deref(), Some("Thank you!"));
    }

    #[test]
    fn dictionary_words_match_before_questions() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let plain = dict_payload(3, "payment");
        let question = dict_payload(3, "payment?");
        assert_eq!(plain.len(), 1);
        assert_eq!(question.len(), 2);
        assert_eq!(decode_dict_preview(3, &question), "Payment?");

        let auto = encode("payment?").expect("encode question");
        assert_eq!(auto.encoded_payload_len, 2);
        assert_eq!(auto.encoding_mode, "dictionary");
        let dec = decode(&auto.hex);
        assert_eq!(dec.text.as_deref(), Some("Payment?"));
    }

    #[test]
    fn dictionary_suffixes_attach_to_stems() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        for text in [
            "started", "starting", "working", "charged", "received", "updated",
        ] {
            let expected = capitalize_word(text);
            let (dict_idx, payload) = best_dict_payload(text);
            assert!(
                payload.len() <= 2,
                "{text} should encode as direct token or stem+suffix"
            );
            assert_eq!(decode_dict_preview(dict_idx, &payload), expected);

            let auto = encode(text).unwrap_or_else(|err| panic!("{text}: {err}"));
            assert_eq!(auto.encoding_mode, "dictionary", "{text}");
            assert!(auto.encoded_payload_len <= 2, "{text}");
            let dec = decode(&auto.hex);
            assert_eq!(dec.text.as_deref(), Some(expected.as_str()));
        }
    }

    #[test]
    fn dictionary_suffixes_attach_before_punctuation() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let (dict_idx, payload) = best_dict_payload("started!");
        assert_eq!(payload.len(), 3);
        assert_eq!(decode_dict_preview(dict_idx, &payload), "Started!");

        let auto = encode("started?").expect("encode");
        assert_eq!(auto.encoding_mode, "dictionary");
        assert_eq!(auto.encoded_payload_len, 3);
        let dec = decode(&auto.hex);
        assert_eq!(dec.text.as_deref(), Some("Started?"));
    }

    #[test]
    fn selects_workflow_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("assign project request status");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 2);
    }

    #[test]
    fn selects_general_chat_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("gm anon 😂");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 0);
    }

    #[test]
    fn selects_phrase_pack_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("are you ready");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 4);
    }

    #[test]
    fn selects_asset_holder_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("holder update ipfs file");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 5);
    }

    #[test]
    fn selects_logistics_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("shipment source verified received at");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 6);
    }

    #[test]
    fn selects_chain_wallet_dictionary() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("wallet unlock transaction confirmed");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 7);
    }

    #[test]
    fn raw_mode_for_ascii_noise() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("abc|def{ghi}");
        assert_eq!(header_mode(&bytes), MODE_RAW);
    }

    #[test]
    fn six_bit_mode_for_digits() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("abcdefghijklmnopqrstuvwxyz 012345");
        assert_eq!(header_mode(&bytes), MODE_6BIT);
    }

    #[test]
    fn five_bit_mode_for_simple_text() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("an easy short line for testing");
        assert_eq!(header_mode(&bytes), MODE_5BIT);
    }

    #[test]
    fn exact_frame_size_and_magic() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let bytes = frame_bytes("hello world");
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], MAGIC_0);
        assert_eq!(bytes[1], MAGIC_1);
        assert_eq!(bytes[2] & VERSION_MASK, FORMAT_VERSION);
    }

    #[test]
    fn bad_magic_rejected() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let mut bytes = frame_bytes("hello world");
        bytes[0] = 0;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn bad_crc_rejected() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let mut bytes = frame_bytes("hello world");
        bytes[CRC_INDEX] ^= 1;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn bad_version_rejected() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let mut bytes = frame_bytes("hello world");
        bytes[2] = (bytes[2] & !VERSION_MASK) | 0x02;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn prefix_0x_supported() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let enc = encode("short message").unwrap();
        let dec = decode(&format!("0x{}", enc.hex));
        assert!(dec.is_short_message);
    }

    #[test]
    fn control_chars_warn_and_roundtrip() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let enc = encode("Hello\x00world\t!").unwrap();
        assert!(enc
            .warnings
            .iter()
            .any(|w| w.contains("control characters")));
        let dec = decode(&enc.hex);
        assert!(dec.is_short_message);
    }

    #[test]
    fn emoji_roundtrip_in_dictionary_mode() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("gm anon 😂");
        assert_eq!(dec.text.as_deref(), Some("GM anon 😂"));
    }

    #[test]
    fn trailing_space_phrase_can_close_message() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("invoice paid payment received amount due");
        assert_eq!(
            dec.text.as_deref(),
            Some("Invoice paid payment received amount due")
        );
    }

    #[test]
    fn phrase_coverage() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let cases = [
            ("payment sent check cid", "Payment sent check CID"),
            ("reward paid to holders", "Reward paid to holders"),
            (
                "build failed after api change",
                "Build failed after API change",
            ),
            (
                "wallet balance and transaction fee",
                "Wallet balance and transaction fee",
            ),
            ("the launch update is live", "The launch update is live"),
            (
                "can you review the rust module",
                "Can you review the rust module",
            ),
            ("hemp token price right now", "HEMP token price right now"),
        ];

        for (input, expected) in cases {
            let dec = decode_frame(input);
            assert!(dec.is_short_message, "failed to decode {}", input);
            assert_eq!(dec.text.as_deref(), Some(expected), "failed: {}", input);
        }
    }

    #[test]
    fn empty_message_rejected() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        assert!(encode("   ").is_err());
    }

    #[test]
    fn long_message_rejected() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        assert!(encode(&"a".repeat(513)).is_err());
    }

    #[test]
    fn never_panics() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        for i in 0u8..=255 {
            let mut frame = [i; FRAME_SIZE];
            frame[0] = MAGIC_0;
            frame[1] = MAGIC_1;
            frame[2] = build_header(0, MODE_RAW);
            frame[3] = 0;
            frame[CRC_INDEX] = crc8(&frame[0..CRC_INDEX]);
            let _ = decode(&hex::encode(frame));
        }
    }

    #[test]
    fn normalized_text_is_lowercase() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let enc = encode("Hello World").unwrap();
        assert_eq!(enc.normalized_text, "hello world");
        assert_eq!(enc.decoded_preview, "Hello world");
    }

    #[test]
    fn long_dict_phrase_matches() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("i think the wallet reward sent");
        assert!(dec.text.unwrap().starts_with("I think"));
    }

    #[test]
    fn collapse_spacing_before_punctuation() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let dec = decode_frame("hello , world . how are you ?");
        assert_eq!(dec.text.as_deref(), Some("Hello, world. How are you?"));
    }

    fn make_custom_pack_dict(entries: &[&str]) -> Vec<String> {
        let mut dict = vec![String::new(); 256];
        for (i, e) in entries.iter().enumerate() {
            dict[i] = e.to_string();
        }
        dict
    }

    fn install_custom_pack(
        name: &str,
        version: &str,
        dict_entries: &[&str],
    ) -> crate::modules::short_message_table_packs::ValidatedTablePack {
        let dict = make_custom_pack_dict(dict_entries);
        let dicts = vec![
            dict,
            make_custom_pack_dict(&[" ", "!"]),
            make_custom_pack_dict(&[" ", "!"]),
        ];
        let file = crate::modules::short_message_table_packs::TablePackFile {
            magic: crate::modules::short_message_table_packs::TABLE_PACK_FILE_MAGIC.to_string(),
            file_version: crate::modules::short_message_table_packs::TABLE_PACK_FILE_VERSION,
            name: name.to_string(),
            version: version.to_string(),
            description: Some("integration test custom pack".to_string()),
            dictionary_titles: vec!["DICT_A: integration test dictionary.".to_string()],
            dictionaries: dicts,
            suffixes: vec!["ed ".to_string()],
            acronyms: vec!["lol".to_string()],
        };
        let (pack, _warnings) =
            crate::modules::short_message_table_packs::validate_table_pack(&file)
                .expect("custom pack must validate");
        crate::modules::short_message_table_packs::set_active_pack_for_tests(pack.clone());
        pack
    }

    #[test]
    fn custom_pack_roundtrips_custom_phrase() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let _pack = install_custom_pack(
            "Hemp0x-Test-Pack",
            "1.0",
            &[" ", "hemp ", "ops ", "alert ", "ping "],
        );

        let enc = encode("hemp ops alert ping").expect("encode with custom pack");
        assert_eq!(enc.encoding_mode, "dictionary");
        assert_eq!(enc.dictionary_index, Some(0));
        assert_eq!(enc.dictionary_name.as_deref(), Some("A"));
        assert!(enc.fits);
        assert_eq!(enc.decoded_preview, "Hemp ops alert ping");

        let dec = decode(&enc.hex);
        assert!(dec.is_short_message);
        assert_eq!(dec.text.as_deref(), Some("Hemp ops alert ping"));

        crate::modules::short_message_table_packs::reset_to_built_in_table_pack()
            .expect("reset to built-in");
    }

    #[test]
    fn reset_returns_to_built_in_pack() {
        let _guard = crate::modules::short_message_table_packs::test_serialize_lock()
            .lock()
            .expect("test lock poisoned");
        let _pack = install_custom_pack("Temp-Pack", "1.0", &[" ", "alpha ", "beta "]);

        let active_before = crate::modules::short_message_table_packs::active_table_pack_summary();
        assert_eq!(active_before.name, "Temp-Pack");

        crate::modules::short_message_table_packs::reset_to_built_in_table_pack()
            .expect("reset to built-in");

        let active_after = crate::modules::short_message_table_packs::active_table_pack_summary();
        let builtin = crate::modules::short_message_table_packs::built_in_summary();
        assert_eq!(active_after.fingerprint_sha256, builtin.fingerprint_sha256);
        assert_eq!(active_after.name, builtin.name);
        assert_eq!(active_after.version, builtin.version);

        let enc = encode("thank you payment sent").expect("encode after reset");
        assert!(enc.fits);
    }
}
