pub fn normalize_hex(hex: &str) -> String {
    if hex.is_empty() {
        return String::new();
    }
    let h = hex.trim();
    let h = h.strip_prefix("0x").unwrap_or(h);
    let h = h.strip_prefix("0X").unwrap_or(h);
    h.to_lowercase()
}

pub fn normalize_nonce_64(hex: &str) -> Result<String, String> {
    let h = normalize_hex(hex);
    if h.is_empty() {
        return Err("Empty nonce".to_string());
    }
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Non-hex nonce".to_string());
    }
    if h.len() > 16 {
        return Err(format!("Nonce too long: {} chars (max 16)", h.len()));
    }
    Ok(format!("{:0>16}", h))
}

pub fn normalize_mix_hash(hex: &str) -> Option<String> {
    let h = normalize_hex(hex);
    if h.len() != 64 {
        return None;
    }
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(h)
}

pub fn compare_target_256(hash_hex: &str, target_hex: &str) -> bool {
    let h = hash_hex.trim().to_lowercase();
    let t = target_hex.trim().to_lowercase();

    if h.len() != 64 || t.len() != 64 {
        return false;
    }
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    if !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    h <= t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_hex_strips_prefix() {
        assert_eq!(normalize_hex("0xaabb"), "aabb");
        assert_eq!(normalize_hex("0XAABB"), "aabb");
        assert_eq!(normalize_hex("aabb"), "aabb");
    }

    #[test]
    fn normalize_hex_handles_empty() {
        assert_eq!(normalize_hex(""), "");
    }

    #[test]
    fn normalize_nonce_pads_to_16() {
        assert_eq!(normalize_nonce_64("1").unwrap(), "0000000000000001");
        assert_eq!(normalize_nonce_64("0x1").unwrap(), "0000000000000001");
    }

    #[test]
    fn normalize_nonce_rejects_empty() {
        assert!(normalize_nonce_64("").is_err());
    }

    #[test]
    fn normalize_nonce_rejects_non_hex() {
        assert!(normalize_nonce_64("ghijk").is_err());
    }

    #[test]
    fn normalize_nonce_rejects_overlong() {
        assert!(normalize_nonce_64("12345678901234567890").is_err());
    }

    #[test]
    fn normalize_nonce_accepts_16() {
        assert!(normalize_nonce_64("0123456789abcdef").is_ok());
    }

    #[test]
    fn normalize_mix_hash_valid() {
        let h = "000000000000000000000000000000000000000000000000000000000000abcd";
        assert_eq!(normalize_mix_hash(h), Some(h.to_string()));
    }

    #[test]
    fn normalize_mix_hash_rejects_wrong_length() {
        assert_eq!(normalize_mix_hash("00ff"), None);
    }

    #[test]
    fn normalize_mix_hash_rejects_non_hex() {
        assert_eq!(normalize_mix_hash(&"g".repeat(64)), None);
    }

    #[test]
    fn compare_target_lower_meets() {
        assert!(compare_target_256(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "00000000000000000000000000000000ffffffffffffffffffffffffffffffff",
        ));
    }

    #[test]
    fn compare_target_higher_fails() {
        assert!(!compare_target_256(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "00000000000000000000000000000000ffffffffffffffffffffffffffffffff",
        ));
    }

    #[test]
    fn compare_target_high_128_same_low_128_differs() {
        assert!(compare_target_256(
            "00000000ffffffffffffffffffffffff00000000000000000000000000000000",
            "00000000ffffffffffffffffffffffff00000000000000000000000000000001",
        ));
        assert!(!compare_target_256(
            "00000000ffffffffffffffffffffffff00000000000000000000000000000002",
            "00000000ffffffffffffffffffffffff00000000000000000000000000000001",
        ));
    }

    #[test]
    fn compare_target_rejects_invalid() {
        assert!(!compare_target_256(
            "short",
            "0000000000000000000000000000000000000000000000000000000000000001"
        ));
        assert!(!compare_target_256(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "short"
        ));
    }
}
