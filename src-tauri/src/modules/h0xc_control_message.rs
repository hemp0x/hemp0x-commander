// HC (Hemp/H0XC Commander) control message frame.
//
// This is a Commander-local application-level payload convention.
// It is NOT a Core consensus rule. Core treats it as opaque message data.
//
// HS = human short message frame (short_message.rs)
// HC = Commander/H0XC control frame (this module)
//
// 32-byte fixed frame:
//   byte 0:  'H' (0x48)
//   byte 1:  'C' (0x43)
//   byte 2:  control format version (start 0x01)
//   byte 3:  command type
//            0x01 = delete message
//   byte 4:  flags (0 reserved)
//   byte 5:  payload length in bytes (0..25)
//   bytes 6..30: command-specific payload, max 25 bytes
//   byte 31: CRC-8 over bytes 0..30 (same polynomial as HS frames)

use serde::Serialize;

const FRAME_SIZE: usize = 32;
const HEX_FRAME_LEN: usize = FRAME_SIZE * 2;

const HC_MAGIC_0: u8 = 0x48; // 'H'
const HC_MAGIC_1: u8 = 0x43; // 'C'
const HC_FORMAT_VERSION: u8 = 0x01;
#[allow(dead_code)]
const HC_FLAGS_RESERVED: u8 = 0x00;
#[allow(dead_code)]
const HC_PAYLOAD_MAX: usize = 25;

const CMD_DELETE: u8 = 0x01;
const CMD_LEAVE: u8 = 0x07;

const PAYLOAD_LEN_INDEX: usize = 5;
const PAYLOAD_INDEX: usize = 6;
const CRC_INDEX: usize = FRAME_SIZE - 1;

const DELETE_PAYLOAD_MIN: usize = 17;
const DELETE_FRAG_LEN_DEFAULT: usize = 8;

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

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let h = hex.get(i..i + 2)?;
        bytes.push(u8::from_str_radix(h, 16).ok()?);
    }
    Some(bytes)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct H0xCControlDecodeResult {
    pub is_control: bool,
    pub version: Option<u8>,
    pub command: Option<String>,
    pub txid_prefix: Option<String>,
    pub txid_suffix: Option<String>,
    pub warnings: Vec<String>,
}

pub fn encode_control_delete(txid_hex: &str) -> Result<String, String> {
    let txid_bytes = hex_to_bytes(txid_hex)
        .ok_or_else(|| format!("Invalid txid hex: {txid_hex}"))?;
    if txid_bytes.len() != 32 {
        return Err(format!("txid must be 32 bytes, got {}", txid_bytes.len()));
    }

    let payload_len = 1 + DELETE_FRAG_LEN_DEFAULT * 2;
    let prefix = &txid_bytes[..DELETE_FRAG_LEN_DEFAULT];
    let suffix = &txid_bytes[32 - DELETE_FRAG_LEN_DEFAULT..];

    let mut frame = [0u8; FRAME_SIZE];
    frame[0] = HC_MAGIC_0;
    frame[1] = HC_MAGIC_1;
    frame[2] = HC_FORMAT_VERSION;
    frame[3] = CMD_DELETE;
    frame[4] = 0x00;
    frame[PAYLOAD_LEN_INDEX] = payload_len as u8;
    frame[PAYLOAD_INDEX] = DELETE_FRAG_LEN_DEFAULT as u8;
    frame[PAYLOAD_INDEX + 1..PAYLOAD_INDEX + 1 + DELETE_FRAG_LEN_DEFAULT].copy_from_slice(prefix);
    frame[PAYLOAD_INDEX + 1 + DELETE_FRAG_LEN_DEFAULT
        ..PAYLOAD_INDEX + 1 + DELETE_FRAG_LEN_DEFAULT * 2]
        .copy_from_slice(suffix);

    frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);

    Ok(bytes_to_hex(&frame))
}

/// Decode an HC control frame from a hex string.
/// Never panics on any input. Returns structured warnings for malformed frames.
pub fn decode_control_frame(hex: &str) -> H0xCControlDecodeResult {
    let mut result = H0xCControlDecodeResult {
        is_control: false,
        version: None,
        command: None,
        txid_prefix: None,
        txid_suffix: None,
        warnings: Vec::new(),
    };

    if hex.len() != HEX_FRAME_LEN {
        return result;
    }

    let bytes = match hex_to_bytes(hex) {
        Some(b) => b,
        None => return result,
    };

    debug_assert_eq!(bytes.len(), FRAME_SIZE);

    if bytes[0] != HC_MAGIC_0 || bytes[1] != HC_MAGIC_1 {
        return result;
    }

    result.is_control = true;

    let expected_crc = crc8(&bytes[..CRC_INDEX]);
    if bytes[CRC_INDEX] != expected_crc {
        result
            .warnings
            .push("CRC mismatch, frame may be corrupted".to_string());
        return result;
    }

    let version = bytes[2];
    result.version = Some(version);

    if version != HC_FORMAT_VERSION {
        result
            .warnings
            .push(format!("Unknown HC version {version}, ignoring"));
        return result;
    }

    if bytes[4] != 0 {
        result
            .warnings
            .push(format!("Unexpected flags byte 0x{:02x}", bytes[4]));
        return result;
    }

    let cmd = bytes[3];
    let payload_len = bytes[PAYLOAD_LEN_INDEX] as usize;

    if payload_len > HC_PAYLOAD_MAX {
        result
            .warnings
            .push(format!("Payload length {payload_len} exceeds maximum {HC_PAYLOAD_MAX}"));
        return result;
    }

    if PAYLOAD_INDEX + payload_len > CRC_INDEX {
        result
            .warnings
            .push(format!("Payload extends past frame boundary: index {PAYLOAD_INDEX} + len {payload_len} > {CRC_INDEX}"));
        return result;
    }

    match cmd {
        CMD_DELETE => {
            result.command = Some("delete".to_string());

            if payload_len < DELETE_PAYLOAD_MIN {
                result.warnings.push(format!(
                    "Delete payload too short: {payload_len} bytes (need {DELETE_PAYLOAD_MIN})"
                ));
                return result;
            }

            let frag_len = bytes[PAYLOAD_INDEX] as usize;
            if frag_len < 1 || frag_len > 16 {
                result
                    .warnings
                    .push(format!("Invalid fragment length: {frag_len}"));
                return result;
            }

            let required_payload = 1 + frag_len * 2;
            if payload_len < required_payload {
                result.warnings.push(format!(
                    "Delete payload too short for fragment length {frag_len}: {payload_len} bytes (need {required_payload})"
                ));
                return result;
            }

            let prefix_start = PAYLOAD_INDEX + 1;
            let prefix_end = prefix_start + frag_len;
            let suffix_start = prefix_end;
            let suffix_end = suffix_start + frag_len;

            if suffix_end > PAYLOAD_INDEX + payload_len {
                result
                    .warnings
                    .push("Delete payload truncated".to_string());
                return result;
            }

            result.txid_prefix = Some(bytes_to_hex(&bytes[prefix_start..prefix_end]));
            result.txid_suffix = Some(bytes_to_hex(&bytes[suffix_start..suffix_end]));
        }
        CMD_LEAVE => {
            result.command = Some("leave".to_string());
            if payload_len != 0 {
                result.warnings.push(format!("Leave payload must be 0 bytes, got {payload_len}"));
                return result;
            }
        }
        0x00 => {
            result
                .warnings
                .push("Command type 0x00 is reserved/invalid".to_string());
        }
        _ => {
            result
                .warnings
                .push(format!("Unknown HC command type 0x{cmd:02x}"));
        }
    }

    result
}

pub fn encode_control_leave() -> Result<String, String> {
    let payload_len = 0;
    let mut frame = [0u8; FRAME_SIZE];
    frame[0] = HC_MAGIC_0;
    frame[1] = HC_MAGIC_1;
    frame[2] = HC_FORMAT_VERSION;
    frame[3] = CMD_LEAVE;
    frame[4] = 0x00;
    frame[PAYLOAD_LEN_INDEX] = payload_len;
    frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
    Ok(bytes_to_hex(&frame))
}

#[tauri::command]
pub fn h0xc_control_encode_delete(txid: String) -> Result<String, String> {
    encode_control_delete(&txid)
}

#[tauri::command]
pub fn h0xc_control_encode_leave() -> Result<String, String> {
    encode_control_leave()
}

#[tauri::command]
pub fn h0xc_control_decode(hex: String) -> H0xCControlDecodeResult {
    decode_control_frame(&hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_delete_frame(txid_hex: &str) -> String {
        encode_control_delete(txid_hex).expect("encode")
    }

    fn frame_from_bytes(frame: &[u8; 32]) -> String {
        bytes_to_hex(frame)
    }

    #[test]
    fn encode_delete_produces_64_char_hex() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        assert_eq!(hex.len(), HEX_FRAME_LEN);
    }

    #[test]
    fn encode_delete_frame_starts_with_hc_magic() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        let bytes = hex_to_bytes(&hex).unwrap();
        assert_eq!(bytes[0], HC_MAGIC_0);
        assert_eq!(bytes[1], HC_MAGIC_1);
    }

    #[test]
    fn encode_delete_frame_has_correct_version_and_cmd() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        let bytes = hex_to_bytes(&hex).unwrap();
        assert_eq!(bytes[2], HC_FORMAT_VERSION);
        assert_eq!(bytes[3], CMD_DELETE);
        assert_eq!(bytes[4], 0x00);
        assert_eq!(bytes[PAYLOAD_LEN_INDEX], 17);
    }

    #[test]
    fn encode_delete_roundtrips_prefix_and_suffix() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.version, Some(1));
        assert_eq!(result.command.as_deref(), Some("delete"));
        assert_eq!(
            result.txid_prefix.as_deref(),
            Some("84c1a733c585ba40")
        );
        assert_eq!(
            result.txid_suffix.as_deref(),
            Some("71be8cd8c09a311a")
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_delete_frame_has_valid_crc() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        let bytes = hex_to_bytes(&hex).unwrap();
        let expected_crc = crc8(&bytes[..CRC_INDEX]);
        assert_eq!(bytes[CRC_INDEX], expected_crc);
    }

    #[test]
    fn decode_rejects_wrong_magic() {
        let mut frame = [0u8; 32];
        frame[0] = 0xAB;
        frame[1] = 0xCD;
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(!result.is_control);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn decode_warns_on_crc_mismatch() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let mut hex = make_valid_delete_frame(txid);
        let last = hex.len() - 1;
        let ch = hex.as_bytes()[last];
        hex.replace_range(last.., if ch == b'0' { "1" } else { "0" });
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("CRC")));
    }

    #[test]
    fn decode_warns_on_unknown_version() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = 0x99; // unknown version
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.version, Some(0x99));
        assert!(result.warnings.iter().any(|w| w.contains("Unknown HC version")));
    }

    #[test]
    fn decode_warns_on_unknown_command() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = 0xFF;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.command.is_none());
        assert!(result.warnings.iter().any(|w| w.contains("Unknown HC command")));
    }

    #[test]
    fn decode_warns_on_nonzero_flags() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[4] = 0x01; // nonzero flags
        frame[PAYLOAD_LEN_INDEX] = DELETE_PAYLOAD_MIN as u8;
        frame[PAYLOAD_INDEX] = DELETE_FRAG_LEN_DEFAULT as u8;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("flags")));
    }

    #[test]
    fn decode_warns_on_payload_len_exceeds_max() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[4] = 0x00;
        frame[PAYLOAD_LEN_INDEX] = 30; // > HC_PAYLOAD_MAX
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("exceeds maximum")));
    }

    #[test]
    fn decode_warns_on_command_type_0x00() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = 0x00; // reserved
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("reserved")));
    }

    #[test]
    fn decode_warns_on_delete_payload_too_short() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[PAYLOAD_LEN_INDEX] = 10; // < DELETE_PAYLOAD_MIN (17)
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("delete"));
        assert!(result.warnings.iter().any(|w| w.contains("too short")));
    }

    #[test]
    fn decode_warns_on_oversized_frag_len() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[PAYLOAD_LEN_INDEX] = DELETE_PAYLOAD_MIN as u8;
        frame[PAYLOAD_INDEX] = 20; // > 16
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("fragment length")));
    }

    #[test]
    fn decode_warns_on_frag_len_zero() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[PAYLOAD_LEN_INDEX] = DELETE_PAYLOAD_MIN as u8;
        frame[PAYLOAD_INDEX] = 0; // < 1
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("fragment length")));
    }

    #[test]
    fn decode_warns_on_truncated_delete_payload() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_DELETE;
        frame[PAYLOAD_LEN_INDEX] = 17; // enough for frag_len=8
        frame[PAYLOAD_INDEX] = 8;
        // But only fill 8 bytes of prefix, leave suffix area with zeros
        // suffix_end would be PAYLOAD_INDEX + 1 + 16 = 23 which is <= PAYLOAD_INDEX + 17 = 23
        // So this is actually valid, just has zero suffix
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("delete"));
    }

    #[test]
    fn encode_rejects_short_txid() {
        let result = encode_control_delete("abcd1234");
        assert!(result.is_err());
    }

    #[test]
    fn encode_rejects_odd_length_hex() {
        let result = encode_control_delete("abc");
        assert!(result.is_err());
    }

    #[test]
    fn encode_rejects_invalid_hex_chars() {
        let result = encode_control_delete("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
        assert!(result.is_err());
    }

    #[test]
    fn decode_empty_string() {
        let result = decode_control_frame("");
        assert!(!result.is_control);
    }

    #[test]
    fn decode_too_short() {
        let result = decode_control_frame("4843");
        assert!(!result.is_control);
    }

    #[test]
    fn decode_too_long() {
        let hex = "4843".repeat(17); // 68 chars
        let result = decode_control_frame(&hex);
        assert!(!result.is_control);
    }

    #[test]
    fn decode_odd_length_hex() {
        let result = decode_control_frame(&"0".repeat(63));
        assert!(!result.is_control);
    }

    #[test]
    fn decode_non_hex_chars() {
        let result = decode_control_frame(&"zz".repeat(32));
        assert!(!result.is_control);
    }

    #[test]
    fn decode_zero_bytes_not_magic() {
        let hex = "00".repeat(32);
        let result = decode_control_frame(&hex);
        assert!(!result.is_control);
    }

    #[test]
    fn all_zero_frame_not_control() {
        let mut frame = [0u8; 32];
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(!result.is_control);
    }

    #[test]
    fn mutation_fuzz_32_byte_frames() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let base_hex = make_valid_delete_frame(txid);
        let base_bytes = hex_to_bytes(&base_hex).unwrap();

        for byte_idx in 0..FRAME_SIZE {
            for bit in 0..8 {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                // Must never panic
                let _ = &result.warnings;
                let _ = &result.command;
                let _ = result.txid_prefix;
                let _ = result.txid_suffix;
            }
        }

        for byte_idx in 0..FRAME_SIZE {
            for replacement in [0x00u8, 0xFF, 0xAA, 0x55] {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] = replacement;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                let _ = &result.warnings;
            }
        }
    }

    #[test]
    fn fuzz_random_32_byte_frames() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        for seed in 0u64..500 {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            let h = hasher.finish();

            let mut frame = [0u8; 32];
            for i in 0..32 {
                let shift = (i as u64) % 58;
                let mult = (seed).wrapping_mul((i as u64) + 1);
                frame[i] = ((h >> shift) ^ mult) as u8;
            }
            let hex = bytes_to_hex(&frame);
            let result = decode_control_frame(&hex);
            let _ = &result.warnings;
            let _ = &result.command;
            let _ = result.txid_prefix;
            let _ = result.txid_suffix;
        }
    }

    #[test]
    fn encode_rejects_txid_not_32_bytes() {
        for len in [0, 1, 15, 16, 31, 33, 64] {
            let hex = "aa".repeat(len);
            let result = encode_control_delete(&hex);
            if len != 32 {
                assert!(result.is_err(), "expected error for {len}-byte txid");
            }
        }
    }

    #[test]
    fn decode_preserves_all_warnings_for_valid_frame_with_unknown_cmd_and_nonzero_flags() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = 0x42;
        frame[4] = 0x01; // nonzero flags
        frame[PAYLOAD_LEN_INDEX] = 5;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("flags")));
    }

    // --- Leave command tests ---

    #[test]
    fn encode_leave_produces_64_char_hex() {
        let hex = encode_control_leave().expect("encode leave");
        assert_eq!(hex.len(), HEX_FRAME_LEN);
    }

    #[test]
    fn encode_leave_frame_starts_with_hc_magic() {
        let hex = encode_control_leave().expect("encode leave");
        let bytes = hex_to_bytes(&hex).unwrap();
        assert_eq!(bytes[0], HC_MAGIC_0);
        assert_eq!(bytes[1], HC_MAGIC_1);
    }

    #[test]
    fn encode_leave_frame_has_correct_version_and_cmd() {
        let hex = encode_control_leave().expect("encode leave");
        let bytes = hex_to_bytes(&hex).unwrap();
        assert_eq!(bytes[2], HC_FORMAT_VERSION);
        assert_eq!(bytes[3], CMD_LEAVE);
        assert_eq!(bytes[4], 0x00);
        assert_eq!(bytes[PAYLOAD_LEN_INDEX], 0);
    }

    #[test]
    fn encode_leave_roundtrips() {
        let hex = encode_control_leave().expect("encode leave");
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.version, Some(1));
        assert_eq!(result.command.as_deref(), Some("leave"));
        assert!(result.txid_prefix.is_none());
        assert!(result.txid_suffix.is_none());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn decode_leave_warns_on_nonzero_payload() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_LEAVE;
        frame[4] = 0x00;
        frame[PAYLOAD_LEN_INDEX] = 5; // nonzero payload
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("leave"));
        assert!(result.warnings.iter().any(|w| w.contains("Leave payload")));
    }

    #[test]
    fn decode_leave_rejects_nonzero_flags() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_LEAVE;
        frame[4] = 0x01; // nonzero flags
        frame[PAYLOAD_LEN_INDEX] = 0;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("flags")));
    }

    #[test]
    fn decode_leave_rejects_unknown_version() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = 0x99; // unknown version
        frame[3] = CMD_LEAVE;
        frame[4] = 0x00;
        frame[PAYLOAD_LEN_INDEX] = 0;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert!(result.warnings.iter().any(|w| w.contains("Unknown HC version")));
    }

    #[test]
    fn decode_leave_malformed_frame_does_not_panic() {
        let hex = "00".repeat(32);
        let result = decode_control_frame(&hex);
        assert!(!result.is_control);
    }

    #[test]
    fn leave_frame_mutation_fuzz() {
        let base_hex = encode_control_leave().expect("encode leave");
        let base_bytes = hex_to_bytes(&base_hex).unwrap();
        for byte_idx in 0..FRAME_SIZE {
            for bit in 0..8 {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                let _ = &result.warnings;
                let _ = &result.command;
                let _ = result.txid_prefix;
                let _ = result.txid_suffix;
            }
        }
    }
}
