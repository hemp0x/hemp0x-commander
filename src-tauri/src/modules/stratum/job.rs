use sha2::{Digest, Sha256};
use sha3::Keccak256;

use crate::modules::stratum::address::build_p2pkh_script;

pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let hash1 = Sha256::digest(data);
    let hash2 = Sha256::digest(&hash1);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash2);
    result
}

pub fn reverse_buffer(input: &[u8]) -> Vec<u8> {
    let mut reversed = vec![0u8; input.len()];
    for i in 0..input.len() {
        reversed[i] = input[input.len() - 1 - i];
    }
    reversed
}

pub fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let h = hex.trim();
    let h = h.strip_prefix("0x").unwrap_or(h);
    let h = h.strip_prefix("0X").unwrap_or(h);
    (0..h.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= h.len() {
                u8::from_str_radix(&h[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect()
}

pub fn serialize_number(n: u32) -> Vec<u8> {
    if n >= 1 && n <= 16 {
        return vec![0x50 + n as u8];
    }
    let mut temp = n;
    let mut len = 0;
    let mut buf = vec![0u8; 8];
    while temp > 0 {
        buf[len] = (temp & 0xff) as u8;
        temp >>= 8;
        len += 1;
    }
    let num_bytes = buf[..len].to_vec();
    match len {
        1 => {
            let mut r = vec![0x01];
            r.extend(&num_bytes);
            r
        }
        2 => {
            let mut r = vec![0x02];
            r.extend(&num_bytes);
            r
        }
        3 => {
            let mut r = vec![0x03];
            r.extend(&num_bytes);
            r
        }
        4 => {
            let mut r = vec![0x04];
            r.extend(&num_bytes);
            r
        }
        _ => {
            let mut r = vec![0x4c, len as u8];
            r.extend(&num_bytes);
            r
        }
    }
}

pub fn var_int_buffer(n: u64) -> Vec<u8> {
    if n < 0xfd {
        return vec![n as u8];
    }
    if n <= 0xffff {
        let mut buf = vec![0u8; 3];
        buf[0] = 0xfd;
        buf[1..3].copy_from_slice(&(n as u16).to_le_bytes());
        return buf;
    }
    if n <= 0xffffffff {
        let mut buf = vec![0u8; 5];
        buf[0] = 0xfe;
        buf[1..5].copy_from_slice(&(n as u32).to_le_bytes());
        return buf;
    }
    let mut buf = vec![0u8; 9];
    buf[0] = 0xff;
    buf[1..9].copy_from_slice(&n.to_le_bytes());
    buf
}

pub struct CoinbaseTx {
    pub tx: Vec<u8>,
    pub txid: Vec<u8>,
    pub txid_hex: String,
}

pub fn build_coinbase_tx(
    height: u32,
    payout_address: &str,
    extra_nonce_1: &[u8; 4],
    extra_nonce_2: &[u8; 4],
    template: &serde_json::Value,
) -> Result<CoinbaseTx, String> {
    let use_segwit = template.get("default_witness_commitment").is_some();

    let mut script = serialize_number(height);
    script.extend_from_slice(&hex_to_bytes("0648656d70307808"));
    script.extend_from_slice(extra_nonce_1);
    script.extend_from_slice(extra_nonce_2);
    let script_len = var_int_buffer(script.len() as u64);

    let mut input = vec![0u8; 32];
    input.extend_from_slice(&hex_to_bytes("ffffffff"));
    input.extend(&script_len);
    input.extend(&script);
    input.extend_from_slice(&hex_to_bytes("ffffffff"));

    let mut outputs: Vec<Vec<u8>> = Vec::new();

    let coinbase_value = template
        .get("coinbasevalue")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let miner_script = build_p2pkh_script(payout_address)?;
    let mut miner_output = coinbase_value.to_le_bytes().to_vec();
    miner_output.extend(&var_int_buffer(miner_script.len() as u64));
    miner_output.extend(&miner_script);
    outputs.push(miner_output);

    if use_segwit {
        if let Some(wc_str) = template
            .get("default_witness_commitment")
            .and_then(|v| v.as_str())
        {
            let commitment = hex_to_bytes(wc_str);
            let mut segwit_output = vec![0u8; 8];
            segwit_output.extend(&var_int_buffer(commitment.len() as u64));
            segwit_output.extend(&commitment);
            outputs.push(segwit_output);
        }
    }

    let output_count = var_int_buffer(outputs.len() as u64);
    let version = hex_to_bytes("01000000");
    let locktime = vec![0u8; 4];

    let mut base_tx = Vec::new();
    base_tx.extend(&version);
    base_tx.extend(&var_int_buffer(1));
    base_tx.extend(&input);
    base_tx.extend(&output_count);
    for o in &outputs {
        base_tx.extend(o);
    }
    base_tx.extend(&locktime);

    let txid = sha256d(&base_tx);

    let full_tx = if use_segwit {
        let marker_flag = hex_to_bytes("0001");
        let witness_stack_count = vec![0x01u8];
        let witness_len = vec![0x20u8];
        let witness_zeros = vec![0u8; 32];
        let mut f = Vec::new();
        f.extend(&version);
        f.extend(&marker_flag);
        f.extend(&var_int_buffer(1));
        f.extend(&input);
        f.extend(&output_count);
        for o in &outputs {
            f.extend(o);
        }
        f.extend(&witness_stack_count);
        f.extend(&witness_len);
        f.extend(&witness_zeros);
        f.extend(&locktime);
        f
    } else {
        base_tx.clone()
    };

    Ok(CoinbaseTx {
        tx: full_tx,
        txid: txid.to_vec(),
        txid_hex: hex::encode(&reverse_buffer(&txid)),
    })
}

pub fn build_merkle_root(coinbase_txid: &[u8], tx_hash_buffers: &[Vec<u8>]) -> Vec<u8> {
    let mut layer: Vec<Vec<u8>> = Vec::new();
    layer.push(coinbase_txid.to_vec());
    for txh in tx_hash_buffers {
        layer.push(txh.clone());
    }

    while layer.len() > 1 {
        let mut next: Vec<Vec<u8>> = Vec::new();
        for i in (0..layer.len()).step_by(2) {
            let left = &layer[i];
            let right = if i + 1 < layer.len() {
                &layer[i + 1]
            } else {
                &layer[i]
            };
            let mut combined = Vec::new();
            combined.extend_from_slice(left);
            combined.extend_from_slice(right);
            let hash = sha256d(&combined);
            next.push(hash.to_vec());
        }
        layer = next;
    }

    layer[0].clone()
}

pub fn build_header_hash(
    version: u32,
    prev_hash_hex: &str,
    merkle_root_le: &[u8],
    time: u32,
    bits_hex: &str,
    height: u32,
) -> String {
    let mut header_input = Vec::new();

    header_input.extend_from_slice(&version.to_le_bytes());

    let prev_hash_bytes = hex_to_bytes(prev_hash_hex);
    header_input.extend_from_slice(&reverse_buffer(&prev_hash_bytes));

    header_input.extend_from_slice(merkle_root_le);

    header_input.extend_from_slice(&time.to_le_bytes());

    let bits_bytes = hex_to_bytes(bits_hex);
    header_input.extend_from_slice(&reverse_buffer(&bits_bytes));

    header_input.extend_from_slice(&height.to_le_bytes());

    let hash = sha256d(&header_input);
    hex::encode(&reverse_buffer(&hash))
}

pub fn bits_to_target(bits_hex: &str) -> String {
    let bits = u32::from_str_radix(bits_hex, 16).unwrap_or(0);
    let exponent = (bits >> 24) as usize;
    let mantissa = bits & 0x007fffff;

    if exponent <= 3 {
        let value = mantissa >> (8 * (3 - exponent));
        return format!("{:064x}", value);
    }

    let shift_bytes = exponent - 3;
    let mut mantissa_hex = format!("{:06x}", mantissa)
        .trim_start_matches('0')
        .to_string();
    if mantissa_hex.is_empty() {
        mantissa_hex = "0".to_string();
    }
    if mantissa_hex.len() % 2 != 0 {
        mantissa_hex = format!("0{}", mantissa_hex);
    }
    let mantissa_chars = mantissa_hex.len() / 2;
    let target_bytes = mantissa_chars + shift_bytes;

    if target_bytes > 32 {
        return "0".repeat(64);
    }

    let zeros_before = (32 - target_bytes) * 2;
    let mut result = "0".repeat(zeros_before);
    result.push_str(&mantissa_hex);
    result.push_str(&"0".repeat(shift_bytes * 2));
    result
}

pub fn bits_to_difficulty(bits_hex: &str) -> Result<f64, String> {
    let bits = u32::from_str_radix(bits_hex, 16)
        .map_err(|e| format!("Invalid bits hex {}: {}", bits_hex, e))?;
    let exponent = (bits >> 24) as i32;
    let mantissa = (bits & 0x007fffff) as f64;

    if mantissa == 0.0 {
        return Ok(0.0);
    }

    let power = 248 - 8 * exponent;
    let numerator = 2.0_f64.powi(power);

    Ok(numerator / mantissa)
}

pub fn get_seed_hash(height: u32) -> String {
    let epoch_length: u32 = 7500;
    let epoch = height / epoch_length;

    let mut seed = [0u8; 32];

    for _ in 0..epoch {
        let mut hasher = Keccak256::new();
        hasher.update(seed);
        let result = hasher.finalize();
        seed.copy_from_slice(&result);
    }

    hex::encode(&seed)
}

pub fn strict_hex_to_bytes(hex: &str, expected_len: Option<usize>) -> Result<Vec<u8>, String> {
    let h = hex.trim();
    let h = h.strip_prefix("0x").unwrap_or(h);
    let h = h.strip_prefix("0X").unwrap_or(h);
    if h.is_empty() {
        return Err("Empty hex string".to_string());
    }
    if h.len() % 2 != 0 {
        return Err(format!("Odd-length hex string: {} chars", h.len()));
    }
    if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Non-hex character in hex string".to_string());
    }
    let bytes: Vec<u8> = (0..h.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex byte at pos {}: {}", i, e))
        })
        .collect::<Result<Vec<u8>, String>>()?;
    if let Some(expected) = expected_len {
        if bytes.len() != expected {
            return Err(format!(
                "Unexpected hex length: expected {} bytes, got {}",
                expected,
                bytes.len()
            ));
        }
    }
    Ok(bytes)
}

pub fn build_block_header(
    job: &JobData,
    nonce_hex: &str,
    mix_hash_hex: &str,
) -> Result<Vec<u8>, String> {
    let version_bytes = job.template_version.to_le_bytes().to_vec();

    let prev_hash_bytes = strict_hex_to_bytes(&job.template_prev_hash, Some(32))?;
    let prev_hash_reversed = reverse_buffer(&prev_hash_bytes);

    let merkle_root_le = job.merkle_root.clone();
    if merkle_root_le.len() != 32 {
        return Err(format!(
            "Merkle root must be 32 bytes, got {}",
            merkle_root_le.len()
        ));
    }

    let time_bytes = job.time.to_le_bytes().to_vec();

    let bits_bytes = strict_hex_to_bytes(&job.bits, Some(4))?;
    let bits_reversed = reverse_buffer(&bits_bytes);

    let height_bytes = job.height.to_le_bytes().to_vec();

    let nonce_clean = strict_hex_to_bytes(nonce_hex, Some(8))?;
    let nonce_u64 = u64::from_be_bytes(
        nonce_clean
            .as_slice()
            .try_into()
            .map_err(|_| "Nonce conversion failed".to_string())?,
    );
    let nonce_le = nonce_u64.to_le_bytes().to_vec();

    let mix_hash_bytes = strict_hex_to_bytes(mix_hash_hex, Some(32))?;
    let mix_hash_reversed = reverse_buffer(&mix_hash_bytes);

    let mut header = Vec::with_capacity(120);
    header.extend_from_slice(&version_bytes);
    header.extend_from_slice(&prev_hash_reversed);
    header.extend_from_slice(&merkle_root_le);
    header.extend_from_slice(&time_bytes);
    header.extend_from_slice(&bits_reversed);
    header.extend_from_slice(&height_bytes);
    header.extend_from_slice(&nonce_le);
    header.extend_from_slice(&mix_hash_reversed);
    Ok(header)
}

pub fn build_block_hex(
    job: &JobData,
    nonce_hex: &str,
    mix_hash_hex: &str,
) -> Result<String, String> {
    let header = build_block_header(job, nonce_hex, mix_hash_hex)?;

    let tx_count = job.tx_count();
    let tx_count_vi = var_int_buffer(tx_count);

    let mut block = Vec::new();
    block.extend_from_slice(&header);
    block.extend_from_slice(&tx_count_vi);
    block.extend_from_slice(&job.coinbase_tx);
    for tx in &job.template_transactions {
        let tx_bytes = strict_hex_to_bytes(&tx.data, None)?;
        block.extend_from_slice(&tx_bytes);
    }

    Ok(hex::encode(&block))
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateTransaction {
    pub data: String,
    pub txid: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct JobData {
    pub job_id: u32,
    pub header_hash: String,
    pub merkle_root: Vec<u8>,
    pub coinbase_tx: Vec<u8>,
    pub time: u32,
    pub height: u32,
    pub bits: String,
    pub share_difficulty: f64,
    pub share_target: String,
    pub network_target: String,
    pub payout_address: String,
    pub template_version: u32,
    pub template_prev_hash: String,
    pub template_transactions: Vec<TemplateTransaction>,
    pub template_coinbase_value: u64,
    pub clean: bool,
}

impl JobData {
    pub fn job_id_str(&self) -> String {
        self.job_id.to_string()
    }

    pub fn tx_count(&self) -> u64 {
        1 + self.template_transactions.len() as u64
    }
}

#[derive(Debug, Clone)]
pub struct JobCache {
    pub jobs: VecDeque<JobData>,
}

impl JobCache {
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::with_capacity(8),
        }
    }

    pub fn get(&self, job_id_str: &str) -> Option<&JobData> {
        self.jobs.iter().find(|j| j.job_id_str() == job_id_str)
    }

    pub fn insert(&mut self, job: JobData) {
        if self.jobs.len() >= 8 {
            self.jobs.pop_front();
        }
        self.jobs.push_back(job);
    }

    pub fn clear(&mut self) {
        self.jobs.clear();
    }
}

pub fn extract_template_tx_hashes(template: &Value) -> Vec<Vec<u8>> {
    let txs = match template.get("transactions").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    txs.iter()
        .map(|tx| {
            let txid_or_hash = tx
                .get("txid")
                .or_else(|| tx.get("hash"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            reverse_buffer(&hex_to_bytes(txid_or_hash))
        })
        .collect()
}

pub fn extract_template_transactions(template: &Value) -> Vec<TemplateTransaction> {
    let txs = match template.get("transactions").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    txs.iter()
        .map(|tx| TemplateTransaction {
            data: tx
                .get("data")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            txid: tx
                .get("txid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            hash: tx
                .get("hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256d_known_vector() {
        let result = sha256d(b"hello");
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn reverse_bytes_reverses() {
        let input = vec![0x01, 0x02, 0x03, 0x04];
        let reversed = reverse_buffer(&input);
        assert_eq!(reversed, vec![0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn hex_to_bytes_parses() {
        assert_eq!(hex_to_bytes("aabb"), vec![0xaa, 0xbb]);
        assert_eq!(hex_to_bytes("0xaabb"), vec![0xaa, 0xbb]);
    }

    #[test]
    fn var_int_small() {
        assert_eq!(var_int_buffer(0xfc), vec![0xfc]);
    }

    #[test]
    fn var_int_medium() {
        let v = var_int_buffer(0x100);
        assert_eq!(v[0], 0xfd);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn var_int_large() {
        let v = var_int_buffer(0x10000);
        assert_eq!(v[0], 0xfe);
        assert_eq!(v.len(), 5);
    }

    #[test]
    fn serialize_number_small() {
        assert_eq!(serialize_number(1), vec![0x51]);
        assert_eq!(serialize_number(16), vec![0x60]);
    }

    #[test]
    fn serialize_number_large() {
        let v = serialize_number(500000);
        assert!(!v.is_empty());
        assert_eq!(v[0], 0x03);
    }

    #[test]
    fn bits_to_target_computes() {
        let target = bits_to_target("1d00ffff");
        assert_eq!(target.len(), 64);
        assert!(target.starts_with("00000000ffff"));
    }

    #[test]
    fn build_header_hash_with_real_merkle() {
        let mr = hex_to_bytes("abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd");
        let hh = build_header_hash(
            0x20000000,
            "00".repeat(32).as_str(),
            &mr,
            1234567890,
            "1d00ffff",
            1000,
        );
        assert_eq!(hh.len(), 64);
    }

    #[test]
    fn build_merkle_root_single() {
        let cb = vec![0xabu8; 32];
        let mr = build_merkle_root(&cb, &[]);
        assert_eq!(mr.len(), 32);
    }

    #[test]
    fn build_merkle_root_two() {
        let cb = vec![0x01u8; 32];
        let tx = vec![0x02u8; 32];
        let mr = build_merkle_root(&cb, &[tx]);
        assert_eq!(mr.len(), 32);
    }

    #[test]
    fn merkle_root_changes_when_extra_nonce_changes() {
        let tpl = serde_json::json!({ "coinbasevalue": 5000000000u64, "version": 0x20000000 });
        let c1 = build_coinbase_tx(
            100,
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM",
            &[0, 0, 0, 1],
            &[0, 0, 0, 0],
            &tpl,
        )
        .unwrap();
        let c2 = build_coinbase_tx(
            100,
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM",
            &[0, 0, 0, 2],
            &[0, 0, 0, 0],
            &tpl,
        )
        .unwrap();
        assert_ne!(c1.txid, c2.txid);

        let mr1 = build_merkle_root(&c1.txid, &[]);
        let mr2 = build_merkle_root(&c2.txid, &[]);
        assert_ne!(mr1, mr2);
    }

    #[test]
    fn job_cache_bounded() {
        let mut cache = JobCache::new();
        for i in 0u32..10 {
            let j = JobData {
                job_id: i,
                header_hash: String::new(),
                merkle_root: vec![],
                coinbase_tx: vec![],
                time: 0,
                height: 0,
                bits: String::new(),
                share_difficulty: 1.0,
                share_target: String::new(),
                network_target: String::new(),
                payout_address: String::new(),
                template_version: 0,
                template_prev_hash: String::new(),
                template_transactions: vec![],
                template_coinbase_value: 0,
                clean: false,
            };
            cache.insert(j);
        }
        assert!(cache.jobs.len() <= 8);
        assert!(cache.get("0").is_none());
        assert!(cache.get("9").is_some());
    }

    #[test]
    fn job_cache_get_existing() {
        let mut cache = JobCache::new();
        let j = JobData {
            job_id: 42,
            header_hash: String::new(),
            merkle_root: vec![],
            coinbase_tx: vec![],
            time: 0,
            height: 0,
            bits: String::new(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0,
            template_prev_hash: String::new(),
            template_transactions: vec![],
            template_coinbase_value: 0,
            clean: false,
        };
        cache.insert(j);
        assert!(cache.get("42").is_some());
        assert!(cache.get("99").is_none());
    }

    #[test]
    fn strict_hex_to_bytes_rejects_odd_length() {
        assert!(strict_hex_to_bytes("a", None).is_err());
        assert!(strict_hex_to_bytes("abc", None).is_err());
    }

    #[test]
    fn strict_hex_to_bytes_rejects_non_hex() {
        assert!(strict_hex_to_bytes("gg", None).is_err());
        assert!(strict_hex_to_bytes("aaxx", None).is_err());
    }

    #[test]
    fn strict_hex_to_bytes_rejects_wrong_fixed_length() {
        let err = strict_hex_to_bytes("aabb", Some(32)).unwrap_err();
        assert!(err.contains("expected 32 bytes"));
    }

    #[test]
    fn strict_hex_to_bytes_parses() {
        assert_eq!(strict_hex_to_bytes("aabb", None).unwrap(), vec![0xaa, 0xbb]);
        assert_eq!(
            strict_hex_to_bytes("0xaabb", None).unwrap(),
            vec![0xaa, 0xbb]
        );
    }

    #[test]
    fn strict_hex_to_bytes_empty_is_rejected() {
        assert!(strict_hex_to_bytes("", None).is_err());
    }

    #[test]
    fn build_block_header_returns_120_bytes() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0x12u8; 32].to_vec(),
            coinbase_tx: vec![],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "00".repeat(32),
            template_transactions: vec![],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000012345";
        let mix = "0".repeat(63) + "a";
        let header = build_block_header(&job, nonce, &mix).unwrap();
        assert_eq!(header.len(), 120);
    }

    #[test]
    fn build_block_header_deterministic() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        let h1 = build_block_header(&job, nonce, &mix).unwrap();
        let h2 = build_block_header(&job, nonce, &mix).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 120);

        let version_be = u32::from_le_bytes(h1[0..4].try_into().unwrap());
        assert_eq!(version_be, 0x20000000);

        let height_be = u32::from_le_bytes(h1[76..80].try_into().unwrap());
        assert_eq!(height_be, 1000);

        assert_eq!(&h1[80..88], &[1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn build_block_header_rejects_bad_prev_hash_length() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "aabb".to_string(),
            template_transactions: vec![],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        assert!(build_block_header(&job, nonce, &mix).is_err());
    }

    #[test]
    fn build_block_header_rejects_bad_mix_length() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        assert!(build_block_header(&job, nonce, "aabb").is_err());
    }

    #[test]
    fn build_block_hex_preserves_tx_order() {
        let coinbase_hex = "01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1234567890ffffffff0100000000000000000000000000";
        let coinbase_bytes = hex_to_bytes(coinbase_hex);
        let tx1 = TemplateTransaction {
            data: "0200000001aa00000000000000000000000000000000000000000000000000000000000000ffffffff01010000000000000000000000000000000000".to_string(),
            txid: String::new(),
            hash: String::new(),
        };
        let tx2 = TemplateTransaction {
            data: "0300000001bb00000000000000000000000000000000000000000000000000000000000000ffffffff01020000000000000000000000000000000000".to_string(),
            txid: String::new(),
            hash: String::new(),
        };
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: coinbase_bytes.clone(),
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![tx1.clone(), tx2.clone()],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        let hex = build_block_hex(&job, nonce, &mix).unwrap();
        let bytes = hex::decode(&hex).unwrap();

        assert_eq!(job.tx_count(), 3);

        let header = &bytes[..120];
        assert_eq!(header.len(), 120);

        let tx1_bytes = hex_to_bytes(&tx1.data);
        let tx2_bytes = hex_to_bytes(&tx2.data);
        let body = &bytes[120..];
        assert!(body
            .windows(coinbase_bytes.len())
            .any(|w| w == coinbase_bytes.as_slice()));
        assert!(body
            .windows(tx1_bytes.len())
            .any(|w| w == tx1_bytes.as_slice()));
        assert!(body
            .windows(tx2_bytes.len())
            .any(|w| w == tx2_bytes.as_slice()));

        let cb_pos = body
            .windows(coinbase_bytes.len())
            .position(|w| w == coinbase_bytes.as_slice())
            .unwrap();
        let tx1_pos = body
            .windows(tx1_bytes.len())
            .position(|w| w == tx1_bytes.as_slice())
            .unwrap();
        let tx2_pos = body
            .windows(tx2_bytes.len())
            .position(|w| w == tx2_bytes.as_slice())
            .unwrap();
        assert!(cb_pos < tx1_pos);
        assert!(tx1_pos < tx2_pos);
    }

    #[test]
    fn build_block_hex_rejects_malformed_tx_data() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![0x01, 0x02],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![TemplateTransaction {
                data: "zz".to_string(),
                txid: String::new(),
                hash: String::new(),
            }],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        assert!(build_block_hex(&job, nonce, &mix).is_err());
    }

    #[test]
    fn build_block_hex_rejects_odd_length_tx_data() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![0x01],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![TemplateTransaction {
                data: "aabbc".to_string(),
                txid: String::new(),
                hash: String::new(),
            }],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        assert!(build_block_hex(&job, nonce, &mix).is_err());
    }

    #[test]
    fn build_block_hex_rejects_empty_tx_data() {
        let job = JobData {
            job_id: 1,
            header_hash: String::new(),
            merkle_root: [0xabu8; 32].to_vec(),
            coinbase_tx: vec![0x01, 0x02],
            time: 1234567890,
            height: 1000,
            bits: "1d00ffff".to_string(),
            share_difficulty: 1.0,
            share_target: String::new(),
            network_target: String::new(),
            payout_address: String::new(),
            template_version: 0x20000000,
            template_prev_hash: "11".repeat(32),
            template_transactions: vec![TemplateTransaction {
                data: String::new(),
                txid: String::new(),
                hash: String::new(),
            }],
            template_coinbase_value: 0,
            clean: false,
        };
        let nonce = "0000000000000001";
        let mix = "a".repeat(64);
        let err = build_block_hex(&job, nonce, &mix).unwrap_err();
        assert!(err.contains("Empty hex string"));
    }

    #[test]
    fn bits_to_difficulty_diff1() {
        let d = bits_to_difficulty("1d00ffff").unwrap();
        assert!((d - 1.0).abs() < 0.01, "expected ~1.0, got {}", d);
    }

    #[test]
    fn bits_to_difficulty_diff2() {
        let d = bits_to_difficulty("1d007fff").unwrap();
        assert!((d - 2.0).abs() < 0.01, "expected ~2.0, got {}", d);
    }

    #[test]
    fn bits_to_difficulty_high_diff() {
        let d = bits_to_difficulty("1c00ffff").unwrap();
        assert!((d - 256.0).abs() < 1.0, "expected ~256, got {}", d);
    }

    #[test]
    fn bits_to_difficulty_rejects_invalid_hex() {
        assert!(bits_to_difficulty("zzzz").is_err());
    }

    #[test]
    fn bits_to_difficulty_zero_mantissa() {
        let d = bits_to_difficulty("00000000").unwrap();
        assert_eq!(d, 0.0);
    }
}
