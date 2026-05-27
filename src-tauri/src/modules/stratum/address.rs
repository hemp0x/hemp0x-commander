use sha2::{Digest, Sha256};

const ADDRESS_VERSION_BYTE: u8 = 0x3c;
const ADDRESS_VALID_CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn sha256d(data: &[u8]) -> [u8; 32] {
    let hash1 = Sha256::digest(data);
    let hash2 = Sha256::digest(&hash1);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash2);
    result
}

fn is_valid_address_format(addr: &str) -> bool {
    if addr.len() < 26 || addr.len() > 35 {
        return false;
    }
    let bytes = addr.as_bytes();
    if bytes[0] != b'R' {
        return false;
    }
    for &b in &bytes[1..] {
        if !ADDRESS_VALID_CHARS.contains(&b) {
            return false;
        }
    }
    true
}

fn has_valid_base58_check(addr: &str) -> bool {
    let decoded = match bs58::decode(addr).into_vec() {
        Ok(v) => v,
        Err(_) => return false,
    };
    if decoded.len() != 25 {
        return false;
    }
    let payload = &decoded[..21];
    let checksum = &decoded[21..25];
    let hash = sha256d(payload);
    hash[0..4] == checksum[0..4]
}

pub fn is_routable_address(addr: &str) -> bool {
    if !is_valid_address_format(addr) {
        return false;
    }
    if !has_valid_base58_check(addr) {
        return false;
    }
    true
}

pub fn address_to_pubkey_hash(address: &str) -> Result<Vec<u8>, String> {
    let decoded = bs58::decode(address)
        .into_vec()
        .map_err(|e| format!("Base58 decode failed: {}", e))?;
    if decoded.len() != 25 {
        return Err(format!(
            "Invalid decoded address length: expected 25, got {}",
            decoded.len()
        ));
    }
    let payload = &decoded[..21];
    let checksum = &decoded[21..25];
    let hash = sha256d(payload);
    if hash[0..4] != checksum[0..4] {
        return Err("Invalid address checksum".to_string());
    }
    Ok(payload[1..21].to_vec())
}

pub fn build_p2pkh_script(address: &str) -> Result<Vec<u8>, String> {
    let pubkey_hash = address_to_pubkey_hash(address)?;
    let mut script = vec![0x76, 0xa9, 0x14];
    script.extend_from_slice(&pubkey_hash);
    script.extend_from_slice(&[0x88, 0xac]);
    Ok(script)
}

pub fn validate_address_detailed(addr: &str) -> Result<(), String> {
    if addr.len() < 26 || addr.len() > 35 {
        return Err("Address length out of range (26-35 chars required)".to_string());
    }
    let bytes = addr.as_bytes();
    if bytes.is_empty() || bytes[0] != b'R' {
        return Err("Address must start with 'R'".to_string());
    }
    for (i, &b) in bytes.iter().enumerate().skip(1) {
        if !ADDRESS_VALID_CHARS.contains(&b) {
            return Err(format!(
                "Invalid character '{}' at position {}",
                b as char, i
            ));
        }
    }
    let decoded = bs58::decode(addr)
        .into_vec()
        .map_err(|e| format!("Base58 decode failed: {}", e))?;
    if decoded.len() != 25 {
        return Err(format!(
            "Invalid decoded address length: expected 25, got {}",
            decoded.len()
        ));
    }
    if decoded[0] != ADDRESS_VERSION_BYTE {
        return Err(format!(
            "Invalid address version byte: expected 0x{:02x}, got 0x{:02x}",
            ADDRESS_VERSION_BYTE, decoded[0]
        ));
    }
    let payload = &decoded[..21];
    let checksum = &decoded[21..25];
    let hash = sha256d(payload);
    if hash[0..4] != checksum[0..4] {
        return Err("Invalid address checksum".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_address_passes() {
        assert!(is_routable_address("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
    }

    #[test]
    fn rejects_too_short() {
        assert!(!is_routable_address("Rabc123"));
    }

    #[test]
    fn rejects_too_long() {
        assert!(!is_routable_address(
            "R1234567890123456789012345678901234567890"
        ));
    }

    #[test]
    fn rejects_no_r_prefix() {
        assert!(!is_routable_address("XvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
    }

    #[test]
    fn rejects_bad_checksum() {
        assert!(!is_routable_address("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgZ"));
    }

    #[test]
    fn rejects_illegal_chars() {
        assert!(!is_routable_address("R0vTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
        assert!(!is_routable_address("ROvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
        assert!(!is_routable_address("RIvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
        assert!(!is_routable_address("RlvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM"));
    }

    #[test]
    fn validate_detailed_good_address() {
        assert!(validate_address_detailed("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM").is_ok());
    }

    #[test]
    fn validate_detailed_bad_checksum() {
        let err = validate_address_detailed("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgZ").unwrap_err();
        assert!(err.contains("checksum"));
    }

    #[test]
    fn validate_detailed_bad_length() {
        let err = validate_address_detailed("Rshort").unwrap_err();
        assert!(err.contains("length"));
    }

    #[test]
    fn validate_detailed_bad_version() {
        let decoded = bs58::decode("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
            .into_vec()
            .unwrap();
        assert_eq!(decoded[0], 0x00);
        let err = validate_address_detailed("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap_err();
        assert!(err.contains("start with 'R'") || err.contains("format"));
    }

    #[test]
    fn address_to_pubkey_hash_valid() {
        let result = address_to_pubkey_hash("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 20);
    }

    #[test]
    fn address_to_pubkey_hash_rejects_invalid() {
        assert!(address_to_pubkey_hash("badaddress").is_err());
        assert!(address_to_pubkey_hash("").is_err());
    }

    #[test]
    fn build_p2pkh_script_valid() {
        let result = build_p2pkh_script("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM");
        assert!(result.is_ok());
        let script = result.unwrap();
        assert_eq!(&script[0..3], &[0x76, 0xa9, 0x14]);
        assert_eq!(script.len(), 25);
    }

    #[test]
    fn build_p2pkh_script_rejects_invalid() {
        assert!(build_p2pkh_script("badaddress").is_err());
    }
}
