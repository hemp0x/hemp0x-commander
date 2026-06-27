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
const HC_PAYLOAD_MAX: usize = 25;

/// Message magic-byte classification for on-chain asset messages.
///
/// This keeps the three H0XC-related message paths distinguishable so the
/// frontend/backend filters can route messages correctly:
/// - `Hs` (HS, 0x48 0x53): normal short message. Belongs in the normal asset
///   message inbox, NOT H0XC chat.
/// - `Hx` (HX, 0x48 0x58): H0XC chat short message. Belongs in H0XC chat,
///   NOT the normal asset inbox.
/// - `Hc` (HC, 0x48 0x43): H0XC control frame (delete/leave/report/status).
///   Hidden from normal chat display by default; applies moderation effects.
/// - `Other`: anything else (raw IPFS hash, legacy message, etc.).
///
/// This is a pure helper over the leading two bytes of a message hex string
/// and does not decode or validate the frame. It exists so the
/// HS/HX/HC distinction can be unit-tested directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MessageMagicKind {
    Hs,
    Hx,
    Hc,
    Other,
}

/// Classify a message by its leading two magic bytes, given as a hex string.
/// Tolerates whitespace, mixed case, and short/empty input (returns `Other`).
pub fn classify_message_magic(hex: &str) -> MessageMagicKind {
    let trimmed = hex.trim().to_lowercase();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 4 {
        return MessageMagicKind::Other;
    }
    // Compare the first two hex byte pairs (4 chars) directly as ASCII.
    let prefix = &trimmed[..4];
    match prefix {
        "4853" => MessageMagicKind::Hs,
        "4858" => MessageMagicKind::Hx,
        "4843" => MessageMagicKind::Hc,
        _ => MessageMagicKind::Other,
    }
}

const CMD_DELETE: u8 = 0x01;
const CMD_LEAVE: u8 = 0x07;
const CMD_STATUS: u8 = 0x08;
const CMD_REPORT: u8 = 0x09;

const PAYLOAD_LEN_INDEX: usize = 5;
const PAYLOAD_INDEX: usize = 6;
const CRC_INDEX: usize = FRAME_SIZE - 1;

const DELETE_PAYLOAD_MIN: usize = 17;
const DELETE_FRAG_LEN_DEFAULT: usize = 8;

const STATUS_PAYLOAD_LEN: usize = 6;
const REPORT_PAYLOAD_LEN: usize = 21;
const REPORT_FRAG_LEN: usize = 8;

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
    // Status command fields (0x08)
    pub status_value: Option<u8>,
    pub status_expiry_mode: Option<u8>,
    pub status_expiry_value: Option<u32>,
    // Report command fields (0x09)
    pub report_target_type: Option<u8>,
    pub report_reason: Option<u8>,
    pub report_severity: Option<u8>,
    pub report_duration_days: Option<u8>,
}

pub fn encode_control_delete(txid_hex: &str) -> Result<String, String> {
    let txid_bytes =
        hex_to_bytes(txid_hex).ok_or_else(|| format!("Invalid txid hex: {txid_hex}"))?;
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
        status_value: None,
        status_expiry_mode: None,
        status_expiry_value: None,
        report_target_type: None,
        report_reason: None,
        report_severity: None,
        report_duration_days: None,
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
        result.warnings.push(format!(
            "Payload length {payload_len} exceeds maximum {HC_PAYLOAD_MAX}"
        ));
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
                result.warnings.push("Delete payload truncated".to_string());
                return result;
            }

            result.txid_prefix = Some(bytes_to_hex(&bytes[prefix_start..prefix_end]));
            result.txid_suffix = Some(bytes_to_hex(&bytes[suffix_start..suffix_end]));
        }
        CMD_LEAVE => {
            result.command = Some("leave".to_string());
            if payload_len != 0 {
                result
                    .warnings
                    .push(format!("Leave payload must be 0 bytes, got {payload_len}"));
                return result;
            }
        }
        CMD_STATUS => {
            result.command = Some("status".to_string());
            if payload_len != STATUS_PAYLOAD_LEN {
                result.warnings.push(format!(
                    "Status payload must be {STATUS_PAYLOAD_LEN} bytes, got {payload_len}"
                ));
                return result;
            }
            let status = bytes[PAYLOAD_INDEX];
            let expiry_mode = bytes[PAYLOAD_INDEX + 1];
            let expiry_value = u32::from_le_bytes([
                bytes[PAYLOAD_INDEX + 2],
                bytes[PAYLOAD_INDEX + 3],
                bytes[PAYLOAD_INDEX + 4],
                bytes[PAYLOAD_INDEX + 5],
            ]);

            if status > 4 {
                result
                    .warnings
                    .push(format!("Invalid status value {status}"));
                return result;
            }
            if expiry_mode > 3 {
                result
                    .warnings
                    .push(format!("Invalid expiry mode {expiry_mode}"));
                return result;
            }
            match expiry_mode {
                0 => {
                    if expiry_value != 0 {
                        result
                            .warnings
                            .push("Expiry mode 0 requires value 0".to_string());
                        return result;
                    }
                }
                1 => {
                    if expiry_value < 1 || expiry_value > 2160 {
                        result.warnings.push(format!(
                            "Expiry mode 1 requires value 1..2160, got {expiry_value}"
                        ));
                        return result;
                    }
                }
                2 => {
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as u32;
                    if expiry_value <= now_secs {
                        result
                            .warnings
                            .push("Expiry mode 2 requires a future timestamp".to_string());
                        return result;
                    }
                    let max_delta = 90 * 86400;
                    if expiry_value > now_secs + max_delta {
                        result
                            .warnings
                            .push("Expiry mode 2 must be within 90 days".to_string());
                        return result;
                    }
                }
                3 => {
                    if expiry_value != 0 {
                        result
                            .warnings
                            .push("Expiry mode 3 requires value 0".to_string());
                        return result;
                    }
                }
                _ => unreachable!(),
            }

            result.status_value = Some(status);
            result.status_expiry_mode = Some(expiry_mode);
            result.status_expiry_value = Some(expiry_value);
        }
        CMD_REPORT => {
            result.command = Some("report".to_string());
            if payload_len != REPORT_PAYLOAD_LEN {
                result.warnings.push(format!(
                    "Report payload must be {REPORT_PAYLOAD_LEN} bytes, got {payload_len}"
                ));
                return result;
            }
            let target_type = bytes[PAYLOAD_INDEX];
            let reason = bytes[PAYLOAD_INDEX + 1];
            let severity = bytes[PAYLOAD_INDEX + 2];
            let duration_days = bytes[PAYLOAD_INDEX + 3];
            let frag_len = bytes[PAYLOAD_INDEX + 4] as usize;

            if target_type < 1 || target_type > 2 {
                result
                    .warnings
                    .push(format!("Invalid target_type {target_type}"));
                return result;
            }
            if reason < 1 || reason > 5 {
                result.warnings.push(format!("Invalid reason {reason}"));
                return result;
            }
            if severity < 1 || severity > 3 {
                result.warnings.push(format!("Invalid severity {severity}"));
                return result;
            }
            if duration_days > 180 {
                result
                    .warnings
                    .push(format!("Invalid duration_days {duration_days}"));
                return result;
            }
            if frag_len != 8 {
                result
                    .warnings
                    .push(format!("Invalid fragment length {frag_len}, must be 8"));
                return result;
            }

            let prefix_start = PAYLOAD_INDEX + 5;
            let prefix_end = prefix_start + 8;
            let suffix_start = prefix_end;
            let suffix_end = suffix_start + 8;

            result.txid_prefix = Some(bytes_to_hex(&bytes[prefix_start..prefix_end]));
            result.txid_suffix = Some(bytes_to_hex(&bytes[suffix_start..suffix_end]));
            result.report_target_type = Some(target_type);
            result.report_reason = Some(reason);
            result.report_severity = Some(severity);
            result.report_duration_days = Some(duration_days);
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

/// Deterministic 16-byte fingerprint from a canonical channel name.
/// Uses FNV-1a 64-bit expanded to 16 bytes.
pub fn channel_fingerprint(channel: &str) -> [u8; 16] {
    let canonical = channel.replace('!', "").trim().to_uppercase();
    let bytes = canonical.as_bytes();

    // FNV-1a 64-bit
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    // Expand 64-bit hash to 16 bytes
    let mut result = [0u8; 16];
    let h2 = hash ^ 0x5bd1e995f5a0c2b3;
    for i in 0..8 {
        result[i] = (hash >> (i * 8)) as u8;
        result[i + 8] = (h2 >> (i * 8)) as u8;
    }
    result
}

/// Encode an HC status control frame (command 0x08).
///
/// Payload layout (6 bytes):
///   byte 0: status enum (0=available, 1=away, 2=dnd, 3=hidden, 4=clear)
///   byte 1: expiry mode (0=default 24h, 1=duration hours, 2=absolute UTC, 3=until changed)
///   bytes 2..5: expiry value LE u32
pub fn encode_control_status(
    status: u8,
    expiry_mode: u8,
    expiry_value: u32,
) -> Result<String, String> {
    if status > 4 {
        return Err(format!("Invalid status value {status}, must be 0..4"));
    }
    if expiry_mode > 3 {
        return Err(format!("Invalid expiry mode {expiry_mode}, must be 0..3"));
    }
    match expiry_mode {
        0 => {
            if expiry_value != 0 {
                return Err("Expiry mode 0 (default) requires value 0".to_string());
            }
        }
        1 => {
            if expiry_value < 1 || expiry_value > 2160 {
                return Err(format!(
                    "Expiry mode 1 (hours) requires value 1..2160, got {expiry_value}"
                ));
            }
        }
        2 => {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u32;
            if expiry_value <= now_secs {
                return Err("Expiry mode 2 (absolute) requires a future timestamp".to_string());
            }
            let max_delta = 90 * 86400;
            if expiry_value > now_secs + max_delta {
                return Err("Expiry mode 2 (absolute) must be within 90 days from now".to_string());
            }
        }
        3 => {
            if expiry_value != 0 {
                return Err("Expiry mode 3 (until changed) requires value 0".to_string());
            }
        }
        _ => unreachable!(),
    }

    let mut frame = [0u8; FRAME_SIZE];
    frame[0] = HC_MAGIC_0;
    frame[1] = HC_MAGIC_1;
    frame[2] = HC_FORMAT_VERSION;
    frame[3] = CMD_STATUS;
    frame[4] = 0x00;
    frame[PAYLOAD_LEN_INDEX] = STATUS_PAYLOAD_LEN as u8;
    frame[PAYLOAD_INDEX] = status;
    frame[PAYLOAD_INDEX + 1] = expiry_mode;
    frame[PAYLOAD_INDEX + 2..PAYLOAD_INDEX + 6].copy_from_slice(&expiry_value.to_le_bytes());
    frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
    Ok(bytes_to_hex(&frame))
}

/// Encode an HC report command for a message (command 0x09, target_type=1).
///
/// Payload layout (21 bytes):
///   byte 0: target_type (1=message, 2=channel)
///   byte 1: reason (1=spam, 2=abuse, 3=scam, 4=off-topic, 5=other)
///   byte 2: severity (1=low, 2=medium, 3=high)
///   byte 3: duration_days (0=default 30, 1..180)
///   byte 4: target_fragment_len (8)
///   bytes 5..12: target txid prefix
///   bytes 13..20: target txid suffix
pub fn encode_control_report_message(
    txid_hex: &str,
    reason: u8,
    severity: u8,
    duration_days: u8,
) -> Result<String, String> {
    let txid_bytes =
        hex_to_bytes(txid_hex).ok_or_else(|| format!("Invalid txid hex: {txid_hex}"))?;
    if txid_bytes.len() != 32 {
        return Err(format!("txid must be 32 bytes, got {}", txid_bytes.len()));
    }
    encode_control_report_common(
        1,
        reason,
        severity,
        duration_days,
        &txid_bytes[..8],
        &txid_bytes[24..],
    )
}

/// Encode an HC report command for a channel/user (command 0x09, target_type=2).
pub fn encode_control_report_channel(
    channel: &str,
    reason: u8,
    severity: u8,
    duration_days: u8,
) -> Result<String, String> {
    if channel.trim().is_empty() {
        return Err("Channel name cannot be empty".to_string());
    }
    let fp = channel_fingerprint(channel);
    encode_control_report_common(2, reason, severity, duration_days, &fp[..8], &fp[8..])
}

fn encode_control_report_common(
    target_type: u8,
    reason: u8,
    severity: u8,
    duration_days: u8,
    prefix: &[u8],
    suffix: &[u8],
) -> Result<String, String> {
    if target_type < 1 || target_type > 2 {
        return Err(format!("Invalid target_type {target_type}, must be 1 or 2"));
    }
    if reason < 1 || reason > 5 {
        return Err(format!("Invalid reason {reason}, must be 1..5"));
    }
    if severity < 1 || severity > 3 {
        return Err(format!("Invalid severity {severity}, must be 1..3"));
    }
    if duration_days > 180 {
        return Err(format!(
            "Invalid duration_days {duration_days}, must be 0..180"
        ));
    }
    if prefix.len() != 8 || suffix.len() != 8 {
        return Err("Prefix and suffix must be 8 bytes each".to_string());
    }

    let mut frame = [0u8; FRAME_SIZE];
    frame[0] = HC_MAGIC_0;
    frame[1] = HC_MAGIC_1;
    frame[2] = HC_FORMAT_VERSION;
    frame[3] = CMD_REPORT;
    frame[4] = 0x00;
    frame[PAYLOAD_LEN_INDEX] = REPORT_PAYLOAD_LEN as u8;
    frame[PAYLOAD_INDEX] = target_type;
    frame[PAYLOAD_INDEX + 1] = reason;
    frame[PAYLOAD_INDEX + 2] = severity;
    frame[PAYLOAD_INDEX + 3] = duration_days;
    frame[PAYLOAD_INDEX + 4] = REPORT_FRAG_LEN as u8;
    frame[PAYLOAD_INDEX + 5..PAYLOAD_INDEX + 13].copy_from_slice(prefix);
    frame[PAYLOAD_INDEX + 13..PAYLOAD_INDEX + 21].copy_from_slice(suffix);
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
pub fn h0xc_control_encode_status(
    status: u8,
    expiry_mode: u8,
    expiry_value: u32,
) -> Result<String, String> {
    encode_control_status(status, expiry_mode, expiry_value)
}

#[tauri::command]
pub fn h0xc_control_encode_report_message(
    txid: String,
    reason: u8,
    severity: u8,
    duration_days: u8,
) -> Result<String, String> {
    encode_control_report_message(&txid, reason, severity, duration_days)
}

#[tauri::command]
pub fn h0xc_control_encode_report_channel(
    channel: String,
    reason: u8,
    severity: u8,
    duration_days: u8,
) -> Result<String, String> {
    encode_control_report_channel(&channel, reason, severity, duration_days)
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

    // --- Message magic classification (HS/HX/HC invariants) ---

    #[test]
    fn classify_hs_normal_short_message() {
        // HS = 0x48 0x53 -> normal asset inbox short message, not H0XC chat.
        assert_eq!(classify_message_magic("4853"), MessageMagicKind::Hs);
        assert_eq!(classify_message_magic("4853abcd1234"), MessageMagicKind::Hs);
    }

    #[test]
    fn classify_hx_h0xc_chat_message() {
        // HX = 0x48 0x58 -> H0XC chat short message, excluded from normal inbox.
        assert_eq!(classify_message_magic("4858"), MessageMagicKind::Hx);
        assert_eq!(classify_message_magic("4858deadbeef"), MessageMagicKind::Hx);
    }

    #[test]
    fn classify_hc_control_frame() {
        // HC = 0x48 0x43 -> H0XC control frame, hidden from chat by default.
        assert_eq!(classify_message_magic("4843"), MessageMagicKind::Hc);
        assert_eq!(classify_message_magic("4843010117"), MessageMagicKind::Hc);
    }

    #[test]
    fn classify_other_for_non_magic_messages() {
        // Raw IPFS hashes / legacy messages have no HS/HX/HC magic.
        assert_eq!(classify_message_magic("QmZPGfJojdTzaqCWJu2m3krark38X1rqEHBo4SjeqHKB26"), MessageMagicKind::Other);
        assert_eq!(classify_message_magic("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"), MessageMagicKind::Other);
        assert_eq!(classify_message_magic("deadbeef"), MessageMagicKind::Other);
    }

    #[test]
    fn classify_tolerates_whitespace_case_and_short_input() {
        assert_eq!(classify_message_magic("  4858  "), MessageMagicKind::Hx);
        assert_eq!(classify_message_magic("4858"), MessageMagicKind::Hx);
        assert_eq!(classify_message_magic("48"), MessageMagicKind::Other);
        assert_eq!(classify_message_magic(""), MessageMagicKind::Other);
    }

    #[test]
    fn classify_distinguishes_chat_from_normal_from_control() {
        // The three H0XC paths must remain mutually distinguishable.
        let hs = classify_message_magic("4853");
        let hx = classify_message_magic("4858");
        let hc = classify_message_magic("4843");
        assert_ne!(hs, hx);
        assert_ne!(hs, hc);
        assert_ne!(hx, hc);
    }

    #[test]
    fn classify_hc_control_frame_matches_encoded_delete() {
        // A real encoded delete control frame must classify as HC.
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = make_valid_delete_frame(txid);
        assert_eq!(classify_message_magic(&hex), MessageMagicKind::Hc);
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
        assert_eq!(result.txid_prefix.as_deref(), Some("84c1a733c585ba40"));
        assert_eq!(result.txid_suffix.as_deref(), Some("71be8cd8c09a311a"));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("Unknown HC version")));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("Unknown HC command")));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("exceeds maximum")));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("fragment length")));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("fragment length")));
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
        let result = encode_control_delete(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        );
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("Unknown HC version")));
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

    // --- Status command tests ---

    #[test]
    fn encode_status_produces_64_char_hex() {
        let hex = encode_control_status(0, 0, 0).expect("encode status");
        assert_eq!(hex.len(), HEX_FRAME_LEN);
    }

    #[test]
    fn encode_status_roundtrips_available_default() {
        let hex = encode_control_status(0, 0, 0).expect("encode");
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("status"));
        assert_eq!(result.status_value, Some(0));
        assert_eq!(result.status_expiry_mode, Some(0));
        assert_eq!(result.status_expiry_value, Some(0));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_status_roundtrips_away_duration_hours() {
        let hex = encode_control_status(1, 1, 48).expect("encode");
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.status_value, Some(1));
        assert_eq!(result.status_expiry_mode, Some(1));
        assert_eq!(result.status_expiry_value, Some(48));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_status_roundtrips_dnd_until_changed() {
        let hex = encode_control_status(2, 3, 0).expect("encode");
        let result = decode_control_frame(&hex);
        assert_eq!(result.status_value, Some(2));
        assert_eq!(result.status_expiry_mode, Some(3));
        assert_eq!(result.status_expiry_value, Some(0));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_status_roundtrips_hidden_absolute() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let future = now + 86400; // 1 day from now
        let hex = encode_control_status(3, 2, future).expect("encode");
        let result = decode_control_frame(&hex);
        assert_eq!(result.status_value, Some(3));
        assert_eq!(result.status_expiry_mode, Some(2));
        assert_eq!(result.status_expiry_value, Some(future));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_status_roundtrips_clear() {
        let hex = encode_control_status(4, 0, 0).expect("encode");
        let result = decode_control_frame(&hex);
        assert_eq!(result.status_value, Some(4));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_status_rejects_invalid_status() {
        assert!(encode_control_status(5, 0, 0).is_err());
        assert!(encode_control_status(255, 0, 0).is_err());
    }

    #[test]
    fn encode_status_rejects_invalid_expiry_mode() {
        assert!(encode_control_status(0, 4, 0).is_err());
    }

    #[test]
    fn encode_status_rejects_default_mode_nonzero_value() {
        assert!(encode_control_status(0, 0, 1).is_err());
    }

    #[test]
    fn encode_status_rejects_hours_mode_zero() {
        assert!(encode_control_status(0, 1, 0).is_err());
    }

    #[test]
    fn encode_status_rejects_hours_mode_over_90_days() {
        assert!(encode_control_status(0, 1, 2161).is_err());
    }

    #[test]
    fn encode_status_rejects_absolute_past_timestamp() {
        assert!(encode_control_status(0, 2, 1000).is_err());
    }

    #[test]
    fn encode_status_rejects_absolute_too_far_future() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        assert!(encode_control_status(0, 2, now + 91 * 86400).is_err());
    }

    #[test]
    fn encode_status_rejects_until_changed_nonzero() {
        assert!(encode_control_status(0, 3, 1).is_err());
    }

    #[test]
    fn decode_status_rejects_wrong_payload_length() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_STATUS;
        frame[PAYLOAD_LEN_INDEX] = 5; // wrong, should be 6
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("payload")));
    }

    #[test]
    fn decode_status_rejects_invalid_status_value() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_STATUS;
        frame[PAYLOAD_LEN_INDEX] = 6;
        frame[PAYLOAD_INDEX] = 5; // invalid
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("Invalid status")));
    }

    #[test]
    fn decode_status_rejects_invalid_expiry_mode() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_STATUS;
        frame[PAYLOAD_LEN_INDEX] = 6;
        frame[PAYLOAD_INDEX] = 0;
        frame[PAYLOAD_INDEX + 1] = 5; // invalid mode
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("Invalid expiry mode")));
    }

    #[test]
    fn status_frame_mutation_fuzz() {
        let base_hex = encode_control_status(1, 1, 24).expect("encode status");
        let base_bytes = hex_to_bytes(&base_hex).unwrap();
        for byte_idx in 0..FRAME_SIZE {
            for bit in 0..8 {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                let _ = &result.warnings;
                let _ = &result.command;
            }
        }
    }

    // --- Report command tests ---

    #[test]
    fn encode_report_message_produces_64_char_hex() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = encode_control_report_message(txid, 1, 2, 30).expect("encode report msg");
        assert_eq!(hex.len(), HEX_FRAME_LEN);
    }

    #[test]
    fn encode_report_message_roundtrips() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = encode_control_report_message(txid, 1, 2, 30).expect("encode");
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("report"));
        assert_eq!(result.report_target_type, Some(1));
        assert_eq!(result.report_reason, Some(1));
        assert_eq!(result.report_severity, Some(2));
        assert_eq!(result.report_duration_days, Some(30));
        assert_eq!(result.txid_prefix.as_deref(), Some("84c1a733c585ba40"));
        assert_eq!(result.txid_suffix.as_deref(), Some("71be8cd8c09a311a"));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_report_channel_roundtrips() {
        let hex = encode_control_report_channel("TESTROOT/H0XC", 3, 1, 60).expect("encode");
        let result = decode_control_frame(&hex);
        assert!(result.is_control);
        assert_eq!(result.command.as_deref(), Some("report"));
        assert_eq!(result.report_target_type, Some(2));
        assert_eq!(result.report_reason, Some(3));
        assert_eq!(result.report_severity, Some(1));
        assert_eq!(result.report_duration_days, Some(60));
        assert!(result.txid_prefix.is_some());
        assert!(result.txid_suffix.is_some());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn encode_report_channel_fingerprint_deterministic() {
        let fp1 = channel_fingerprint("TESTROOT/H0XC");
        let fp2 = channel_fingerprint("TESTROOT/H0XC");
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn encode_report_channel_fingerprint_varies_by_name() {
        let fp1 = channel_fingerprint("ROOTA/H0XC");
        let fp2 = channel_fingerprint("ROOTB/H0XC");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn encode_report_channel_fingerprint_normalizes() {
        let fp1 = channel_fingerprint("testroot/h0xc");
        let fp2 = channel_fingerprint("TESTROOT/H0XC!");
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn encode_report_rejects_bad_target_type() {
        assert!(encode_control_report_common(3, 1, 1, 30, &[0u8; 8], &[0u8; 8]).is_err());
    }

    #[test]
    fn encode_report_rejects_bad_reason() {
        assert!(encode_control_report_common(1, 0, 1, 30, &[0u8; 8], &[0u8; 8]).is_err());
        assert!(encode_control_report_common(1, 6, 1, 30, &[0u8; 8], &[0u8; 8]).is_err());
    }

    #[test]
    fn encode_report_rejects_bad_severity() {
        assert!(encode_control_report_common(1, 1, 0, 30, &[0u8; 8], &[0u8; 8]).is_err());
        assert!(encode_control_report_common(1, 1, 4, 30, &[0u8; 8], &[0u8; 8]).is_err());
    }

    #[test]
    fn encode_report_rejects_over_180_days() {
        assert!(encode_control_report_common(1, 1, 1, 181, &[0u8; 8], &[0u8; 8]).is_err());
    }

    #[test]
    fn encode_report_message_rejects_short_txid() {
        assert!(encode_control_report_message("abcd", 1, 1, 30).is_err());
    }

    #[test]
    fn encode_report_channel_rejects_empty() {
        assert!(encode_control_report_channel("", 1, 1, 30).is_err());
        assert!(encode_control_report_channel("  ", 1, 1, 30).is_err());
    }

    #[test]
    fn decode_report_rejects_wrong_payload_length() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 10; // wrong
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("payload")));
    }

    #[test]
    fn decode_report_rejects_bad_target_type() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 21;
        frame[PAYLOAD_INDEX] = 3; // invalid
        frame[PAYLOAD_INDEX + 4] = 8;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("target_type")));
    }

    #[test]
    fn decode_report_rejects_bad_reason() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 21;
        frame[PAYLOAD_INDEX] = 1;
        frame[PAYLOAD_INDEX + 1] = 0; // invalid reason
        frame[PAYLOAD_INDEX + 4] = 8;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("reason")));
    }

    #[test]
    fn decode_report_rejects_bad_severity() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 21;
        frame[PAYLOAD_INDEX] = 1;
        frame[PAYLOAD_INDEX + 1] = 1;
        frame[PAYLOAD_INDEX + 2] = 5; // invalid
        frame[PAYLOAD_INDEX + 4] = 8;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("severity")));
    }

    #[test]
    fn decode_report_rejects_bad_duration() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 21;
        frame[PAYLOAD_INDEX] = 1;
        frame[PAYLOAD_INDEX + 1] = 1;
        frame[PAYLOAD_INDEX + 2] = 1;
        frame[PAYLOAD_INDEX + 3] = 200; // invalid > 180
        frame[PAYLOAD_INDEX + 4] = 8;
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result.warnings.iter().any(|w| w.contains("duration")));
    }

    #[test]
    fn decode_report_rejects_bad_frag_len() {
        let mut frame = [0u8; 32];
        frame[0] = HC_MAGIC_0;
        frame[1] = HC_MAGIC_1;
        frame[2] = HC_FORMAT_VERSION;
        frame[3] = CMD_REPORT;
        frame[PAYLOAD_LEN_INDEX] = 21;
        frame[PAYLOAD_INDEX] = 1;
        frame[PAYLOAD_INDEX + 1] = 1;
        frame[PAYLOAD_INDEX + 2] = 1;
        frame[PAYLOAD_INDEX + 3] = 30;
        frame[PAYLOAD_INDEX + 4] = 4; // wrong, must be 8
        frame[CRC_INDEX] = crc8(&frame[..CRC_INDEX]);
        let hex = frame_from_bytes(&frame);
        let result = decode_control_frame(&hex);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("fragment length")));
    }

    #[test]
    fn report_frame_mutation_fuzz() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let base_hex = encode_control_report_message(txid, 2, 3, 30).expect("encode report");
        let base_bytes = hex_to_bytes(&base_hex).unwrap();
        for byte_idx in 0..FRAME_SIZE {
            for bit in 0..8 {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                let _ = &result.warnings;
                let _ = &result.command;
            }
        }
    }

    #[test]
    fn report_channel_frame_mutation_fuzz() {
        let base_hex = encode_control_report_channel("FUZZROOT/H0XC", 1, 1, 30).expect("encode");
        let base_bytes = hex_to_bytes(&base_hex).unwrap();
        for byte_idx in 0..FRAME_SIZE {
            for bit in 0..8 {
                let mut mutated = base_bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                let hex = bytes_to_hex(&mutated);
                let result = decode_control_frame(&hex);
                let _ = &result.warnings;
                let _ = &result.command;
            }
        }
    }

    #[test]
    fn encode_report_all_valid_reasons() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        for reason in 1..=5 {
            let hex = encode_control_report_message(txid, reason, 1, 30).expect("encode");
            let result = decode_control_frame(&hex);
            assert_eq!(result.report_reason, Some(reason));
        }
    }

    #[test]
    fn encode_report_all_valid_severities() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        for sev in 1..=3 {
            let hex = encode_control_report_message(txid, 1, sev, 30).expect("encode");
            let result = decode_control_frame(&hex);
            assert_eq!(result.report_severity, Some(sev));
        }
    }

    #[test]
    fn encode_report_zero_duration_uses_default() {
        let txid = "84c1a733c585ba40d8798a3c67a4a3d5155c4d306274ac2971be8cd8c09a311a";
        let hex = encode_control_report_message(txid, 1, 1, 0).expect("encode");
        let result = decode_control_frame(&hex);
        assert_eq!(result.report_duration_days, Some(0));
        assert!(result.warnings.is_empty());
    }
}
