use super::short_message_tables::{
    ACRONYMS, ALPHABET_5BIT, ALPHABET_6BIT, DICTIONARIES,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

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

#[derive(Debug, Clone)]
struct Candidate {
    mode: u8,
    dict_idx: u8,
    payload: Vec<u8>,
}

pub(crate) struct DictionaryRuntime {
    pub(crate) normalized: Vec<String>,
    pub(crate) match_order: Vec<u8>,
}

static DICTIONARY_RUNTIMES: OnceLock<Vec<DictionaryRuntime>> = OnceLock::new();
static ALPHABET_5BIT_REV: OnceLock<HashMap<u8, u8>> = OnceLock::new();
static ALPHABET_6BIT_REV: OnceLock<HashMap<u8, u8>> = OnceLock::new();

fn is_printable_ascii(byte: u8) -> bool {
    (32..=126).contains(&byte)
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

fn normalize_phrase(phrase: &str) -> String {
    phrase.chars().flat_map(|c| c.to_lowercase()).collect()
}

fn dictionary_runtimes() -> &'static [DictionaryRuntime] {
    DICTIONARY_RUNTIMES.get_or_init(|| {
        DICTIONARIES
            .iter()
            .map(|(_, dict)| {
                let normalized: Vec<String> =
                    dict.iter().map(|phrase| normalize_phrase(phrase)).collect();
                let mut match_order: Vec<u8> = normalized
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, phrase)| (!phrase.is_empty()).then_some(idx as u8))
                    .collect();
                match_order.sort_by(|a, b| {
                    normalized[*b as usize]
                        .len()
                        .cmp(&normalized[*a as usize].len())
                        .then((*a).cmp(b))
                });
                DictionaryRuntime {
                    normalized,
                    match_order,
                }
            })
            .collect()
    })
}

pub(crate) fn get_dictionary_runtimes() -> &'static [DictionaryRuntime] {
    dictionary_runtimes()
}

pub(crate) fn normalize_text_for_autocomplete(text: &str) -> String {
    normalize(text).0
}

pub(crate) fn restore_case_pub(raw: &str) -> String {
    restore_case(raw)
}

fn alphabet_5bit_rev() -> &'static HashMap<u8, u8> {
    ALPHABET_5BIT_REV.get_or_init(|| build_rev_map(&ALPHABET_5BIT))
}

fn alphabet_6bit_rev() -> &'static HashMap<u8, u8> {
    ALPHABET_6BIT_REV.get_or_init(|| build_rev_map(&ALPHABET_6BIT))
}

fn build_rev_map(alphabet: &[u8]) -> HashMap<u8, u8> {
    let mut map = HashMap::with_capacity(alphabet.len());
    for (idx, &ch) in alphabet.iter().enumerate() {
        map.insert(ch, idx as u8);
    }
    map
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

fn should_extend_word(ch: char, prev_was_alpha: bool) -> bool {
    ch.is_alphabetic() || (ch == '\'' && prev_was_alpha)
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
    } else if ACRONYMS.contains(&lower.as_str()) {
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

    let mut prev_was_alpha = false;

    for ch in raw.chars() {
        if should_extend_word(ch, prev_was_alpha) {
            word.push(ch);
            prev_was_alpha = ch.is_alphabetic();
            continue;
        }

        sentence_start = flush_word(&mut word, sentence_start, &mut out);

        out.push(ch);
        if is_sentence_boundary(ch) {
            sentence_start = true;
        }
        prev_was_alpha = false;
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

fn encode_dict(text: &str, runtime: &DictionaryRuntime) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut payload = Vec::with_capacity(PAYLOAD_MAX);
    let mut i = 0;

    while i < bytes.len() {
        let mut matched = false;
        for &tok in &runtime.match_order {
            let phrase_bytes = runtime.normalized[tok as usize].as_bytes();
            let remaining = &bytes[i..];
            if remaining.starts_with(phrase_bytes) {
                payload.push(tok);
                i += phrase_bytes.len();
                matched = true;
                break;
            }

            // Allow terminal phrases stored with a trailing space to also match at
            // end-of-message. The decoded text is collapsed later, so this
            // recovers codebook utility without changing the public wire format.
            if phrase_bytes.ends_with(b" ") && remaining == &phrase_bytes[..phrase_bytes.len() - 1]
            {
                payload.push(tok);
                i = bytes.len();
                matched = true;
                break;
            }
        }

        if !matched {
            return None;
        }

        if payload.len() > PAYLOAD_MAX {
            return None;
        }
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
    for &tok in payload {
        let phrase = runtime.normalized.get(tok as usize)?;
        if phrase.is_empty() {
            return None;
        }
        out.push_str(phrase);
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

fn select_candidate(
    text: &str,
    rev5: &HashMap<u8, u8>,
    rev6: &HashMap<u8, u8>,
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    let runtimes = dictionary_runtimes();

    let mut consider = |candidate: Candidate| {
        let take = match &best {
            None => true,
            Some(current) => candidate_sort_key(&candidate) < candidate_sort_key(current),
        };

        if take {
            best = Some(candidate);
        }
    };

    for (dict_idx, runtime) in runtimes.iter().enumerate() {
        if let Some(payload) = encode_dict(text, runtime) {
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

    if let Some(payload) = encode_nbit(text, rev5, 5, PAYLOAD_MAX * 8 / 5) {
        consider(Candidate {
            mode: MODE_5BIT,
            dict_idx: 0,
            payload,
        });
    }

    if let Some(payload) = encode_nbit(text, rev6, 6, PAYLOAD_MAX * 8 / 6) {
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

    match mode {
        MODE_DICT => dictionary_runtimes()
            .get(dict_idx)
            .and_then(|runtime| decode_dict(payload, runtime)),
        MODE_RAW => {
            if payload.iter().all(|&b| is_printable_ascii(b)) {
                Some(String::from_utf8_lossy(payload).into_owned())
            } else {
                None
            }
        }
        MODE_5BIT => decode_nbit(payload, &ALPHABET_5BIT, 5),
        MODE_6BIT => decode_nbit(payload, &ALPHABET_6BIT, 6),
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

    let candidate = select_candidate(&normalized_text, alphabet_5bit_rev(), alphabet_6bit_rev())
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
        fits: true,
        warnings,
    })
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
    use crate::modules::short_message_tables::{DICT_A, DICT_B, DICT_C, DICT_D, DICT_E};

    fn decode_frame(text: &str) -> ShortMessageDecodeResult {
        let enc = encode(text).expect("encode");
        assert_eq!(enc.hex.len(), 64);
        decode(&enc.hex)
    }

    fn frame_bytes(text: &str) -> Vec<u8> {
        hex::decode(encode(text).expect("encode").hex).expect("hex")
    }

    fn header_mode(bytes: &[u8]) -> u8 {
        bytes[2] & MODE_MASK
    }

    fn header_dict(bytes: &[u8]) -> usize {
        (bytes[2] >> DICT_SHIFT) as usize
    }

    #[test]
    fn dictionary_shapes() {
        let lengths = [
            DICT_A.len(),
            DICT_B.len(),
            DICT_C.len(),
            DICT_D.len(),
            DICT_E.len(),
        ];
        assert_eq!(lengths, [256, 256, 256, 256, 256]);

        for dict in [
            &DICT_A[..],
            &DICT_B[..],
            &DICT_C[..],
            &DICT_D[..],
            &DICT_E[..],
        ] {
            assert!(dict.iter().all(|phrase| !phrase.is_empty()));
        }
    }

    #[test]
    fn roundtrip_general_dictionary() {
        let dec = decode_frame("i think the hemp token launch is soon");
        assert!(dec.is_short_message);
        assert_eq!(
            dec.text.as_deref(),
            Some("I think the HEMP token launch is soon")
        );
    }

    #[test]
    fn restores_case_and_acronyms() {
        let dec = decode_frame("payment sent. check cid and ipfs.");
        assert_eq!(
            dec.text.as_deref(),
            Some("Payment sent. Check CID and IPFS.")
        );
    }

    #[test]
    fn selects_business_dictionary() {
        let bytes = frame_bytes("invoice paid payment received amount due");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 1);
    }

    #[test]
    fn selects_tech_dictionary() {
        let bytes = frame_bytes("build failed code review runtime cache");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 2);
    }

    #[test]
    fn selects_social_dictionary() {
        let bytes = frame_bytes("gm anon 😂");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 3);
    }

    #[test]
    fn selects_news_dictionary() {
        let bytes = frame_bytes("breaking news market update");
        assert_eq!(header_mode(&bytes), MODE_DICT);
        assert_eq!(header_dict(&bytes), 4);
    }

    #[test]
    fn raw_mode_for_ascii_noise() {
        let bytes = frame_bytes("abc|def{ghi}");
        assert_eq!(header_mode(&bytes), MODE_RAW);
    }

    #[test]
    fn six_bit_mode_for_digits() {
        let bytes = frame_bytes("abcdefghijklmnopqrstuvwxyz 012345");
        assert_eq!(header_mode(&bytes), MODE_6BIT);
    }

    #[test]
    fn five_bit_mode_for_simple_text() {
        let bytes = frame_bytes("an easy short line for testing");
        assert_eq!(header_mode(&bytes), MODE_5BIT);
    }

    #[test]
    fn exact_frame_size_and_magic() {
        let bytes = frame_bytes("hello world");
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], MAGIC_0);
        assert_eq!(bytes[1], MAGIC_1);
        assert_eq!(bytes[2] & VERSION_MASK, FORMAT_VERSION);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = frame_bytes("hello world");
        bytes[0] = 0;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn bad_crc_rejected() {
        let mut bytes = frame_bytes("hello world");
        bytes[CRC_INDEX] ^= 1;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn bad_version_rejected() {
        let mut bytes = frame_bytes("hello world");
        bytes[2] = (bytes[2] & !VERSION_MASK) | 0x02;
        assert!(!decode(&hex::encode(bytes)).is_short_message);
    }

    #[test]
    fn prefix_0x_supported() {
        let enc = encode("short message").unwrap();
        let dec = decode(&format!("0x{}", enc.hex));
        assert!(dec.is_short_message);
    }

    #[test]
    fn control_chars_warn_and_roundtrip() {
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
        let dec = decode_frame("gm anon 😂");
        assert_eq!(dec.text.as_deref(), Some("GM anon 😂"));
    }

    #[test]
    fn trailing_space_phrase_can_close_message() {
        let dec = decode_frame("invoice paid payment received amount due");
        assert_eq!(
            dec.text.as_deref(),
            Some("Invoice paid payment received amount due")
        );
    }

    #[test]
    fn phrase_coverage() {
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
        assert!(encode("   ").is_err());
    }

    #[test]
    fn long_message_rejected() {
        assert!(encode(&"a".repeat(513)).is_err());
    }

    #[test]
    fn never_panics() {
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
        let enc = encode("Hello World").unwrap();
        assert_eq!(enc.normalized_text, "hello world");
        assert_eq!(enc.decoded_preview, "Hello world");
    }

    #[test]
    fn long_dict_phrase_matches() {
        let dec = decode_frame("i think the wallet reward sent");
        assert!(dec.text.unwrap().starts_with("I think"));
    }

    #[test]
    fn collapse_spacing_before_punctuation() {
        let dec = decode_frame("hello , world . how are you ?");
        assert_eq!(dec.text.as_deref(), Some("Hello, world. How are you?"));
    }
}
