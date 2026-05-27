use std::collections::VecDeque;

pub const VARDIFF_MIN_DIFF: f64 = 0.2;
pub const VARDIFF_MAX_DIFF: f64 = 1024.0;
pub const VARDIFF_TARGET_TIME: f64 = 15.0;
pub const VARDIFF_RETARGET_TIME_SECS: u64 = 30;
pub const VARDIFF_VARIANCE: f64 = 0.4;
pub const HASHRATE_WINDOW_SECS: u64 = 300;
pub const MIN_HASHRATE_SAMPLE_SECS: f64 = 15.0;
pub const DIFF1_BASE: f64 = 4_294_967_296.0; // 2^32 = expected hashes per diff-1 share

const VARDIFF_PRECISION: u64 = 100_000_000;

pub fn difficulty_to_target(difficulty: f64) -> Result<String, String> {
    if difficulty <= 0.0 {
        return Err("Difficulty must be positive".to_string());
    }
    let scaled = (difficulty * VARDIFF_PRECISION as f64).floor() as u64;
    let scaled = scaled.max(1);

    let diff1_bytes =
        hex::decode("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .map_err(|e| format!("Hex decode error: {}", e))?;

    let mut product = vec![0u8; 40];
    let mut carry: u64 = 0;
    for i in (0..32).rev() {
        let val = diff1_bytes[i] as u64 * VARDIFF_PRECISION + carry;
        product[i + 8] = (val % 256) as u8;
        carry = val / 256;
    }
    for j in (0..8).rev() {
        if carry == 0 {
            break;
        }
        product[j] = (carry % 256) as u8;
        carry /= 256;
    }

    let mut quotient = vec![0u8; 40];
    let mut remainder: u64 = 0;
    for i in 0..40 {
        let val = remainder * 256 + product[i] as u64;
        quotient[i] = (val / scaled) as u8;
        remainder = val % scaled;
    }

    let start = quotient
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(quotient.len());
    let target_bytes = &quotient[start..];

    let mut result = vec![0u8; 32];
    let keep = target_bytes.len().min(32);
    result[(32 - keep)..].copy_from_slice(&target_bytes[target_bytes.len() - keep..]);

    Ok(hex::encode(&result))
}

pub struct VardiffResult {
    pub changed: bool,
    pub new_diff: f64,
}

pub fn compute_vardiff_adjustment(
    current_diff: f64,
    share_history: &VecDeque<(u64, f64)>,
    retarget_deadline: u64,
) -> VardiffResult {
    if share_history.len() < 10 {
        return VardiffResult {
            changed: false,
            new_diff: current_diff,
        };
    }

    let now = crate::modules::stratum::state::current_timestamp();
    if now < retarget_deadline {
        return VardiffResult {
            changed: false,
            new_diff: current_diff,
        };
    }

    let recent: Vec<&(u64, f64)> = share_history.iter().rev().take(10).collect();
    if recent.len() < 2 {
        return VardiffResult {
            changed: false,
            new_diff: current_diff,
        };
    }

    let mut times = Vec::with_capacity(recent.len() - 1);
    for i in 1..recent.len() {
        let elapsed = (recent[i - 1].0.saturating_sub(recent[i].0)) as f64;
        times.push(elapsed.max(0.1));
    }

    let avg_time = times.iter().sum::<f64>() / times.len() as f64;

    let low_bound = VARDIFF_TARGET_TIME * (1.0 - VARDIFF_VARIANCE);
    let high_bound = VARDIFF_TARGET_TIME * (1.0 + VARDIFF_VARIANCE);

    let mut new_diff = if avg_time < low_bound {
        current_diff * 1.5
    } else if avg_time > high_bound {
        current_diff / 1.5
    } else {
        return VardiffResult {
            changed: false,
            new_diff: current_diff,
        };
    };

    new_diff = new_diff.clamp(VARDIFF_MIN_DIFF, VARDIFF_MAX_DIFF);

    VardiffResult {
        changed: (new_diff - current_diff).abs() > f64::EPSILON,
        new_diff,
    }
}

pub fn compute_worker_hashrate_hs(share_history: &VecDeque<(u64, f64)>, now: u64) -> f64 {
    if share_history.is_empty() {
        return 0.0;
    }

    let cutoff = now.saturating_sub(HASHRATE_WINDOW_SECS);
    let sum_diff: f64 = share_history
        .iter()
        .filter(|(ts, _)| *ts >= cutoff)
        .map(|(_, d)| *d)
        .sum();

    if sum_diff == 0.0 {
        return 0.0;
    }

    let first_ts = share_history
        .iter()
        .filter(|(ts, _)| *ts >= cutoff)
        .map(|(ts, _)| *ts)
        .min()
        .unwrap_or(now);

    let elapsed = (now.saturating_sub(first_ts)) as f64;
    let time_divisor = elapsed.max(MIN_HASHRATE_SAMPLE_SECS);

    (sum_diff * DIFF1_BASE) / time_divisor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_to_target_diff1() {
        let t = difficulty_to_target(1.0).unwrap();
        assert_eq!(
            t,
            "00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        );
    }

    #[test]
    fn difficulty_to_target_diff2() {
        let t = difficulty_to_target(2.0).unwrap();
        assert!(t.starts_with("000000007f"));
        assert_eq!(t.len(), 64);
    }

    #[test]
    fn difficulty_to_target_diff_half() {
        let t = difficulty_to_target(0.5).unwrap();
        assert!(t.starts_with("00000001ff"));
        assert_eq!(t.len(), 64);
    }

    #[test]
    fn difficulty_to_target_rejects_zero() {
        assert!(difficulty_to_target(0.0).is_err());
        assert!(difficulty_to_target(-1.0).is_err());
    }

    #[test]
    fn difficulty_to_target_1024() {
        let t = difficulty_to_target(1024.0).unwrap();
        assert_eq!(t.len(), 64);
    }

    fn make_history(entries: &[(u64, f64)]) -> VecDeque<(u64, f64)> {
        entries.iter().copied().collect()
    }

    #[test]
    fn vardiff_too_few_shares() {
        let h = make_history(&[(100, 1.0), (110, 1.0), (120, 1.0), (130, 1.0)]);
        let r = compute_vardiff_adjustment(1.0, &h, 0);
        assert!(!r.changed);
    }

    #[test]
    fn vardiff_fast_raises() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 40, 1.0),
            (now - 39, 1.0),
            (now - 38, 1.0),
            (now - 37, 1.0),
            (now - 36, 1.0),
            (now - 35, 1.0),
            (now - 34, 1.0),
            (now - 33, 1.0),
            (now - 32, 1.0),
            (now - 31, 1.0),
        ]);
        let r = compute_vardiff_adjustment(1.0, &h, 0);
        assert!(r.changed);
        assert!(r.new_diff > 1.0);
    }

    #[test]
    fn vardiff_slow_lowers() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 300, 8.0),
            (now - 270, 8.0),
            (now - 240, 8.0),
            (now - 210, 8.0),
            (now - 180, 8.0),
            (now - 150, 8.0),
            (now - 120, 8.0),
            (now - 90, 8.0),
            (now - 60, 8.0),
            (now - 30, 8.0),
        ]);
        let r = compute_vardiff_adjustment(8.0, &h, 0);
        assert!(r.changed);
        assert!(r.new_diff < 8.0);
    }

    #[test]
    fn vardiff_stable_zone() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 150, 1.0),
            (now - 135, 1.0),
            (now - 120, 1.0),
            (now - 105, 1.0),
            (now - 90, 1.0),
            (now - 75, 1.0),
            (now - 60, 1.0),
            (now - 45, 1.0),
            (now - 30, 1.0),
            (now - 15, 1.0),
        ]);
        let r = compute_vardiff_adjustment(1.0, &h, 0);
        assert!(!r.changed);
    }

    #[test]
    fn vardiff_clamps_min() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 300, 0.25),
            (now - 270, 0.25),
            (now - 240, 0.25),
            (now - 210, 0.25),
            (now - 180, 0.25),
            (now - 150, 0.25),
            (now - 120, 0.25),
            (now - 90, 0.25),
            (now - 60, 0.25),
            (now - 30, 0.25),
        ]);
        let r = compute_vardiff_adjustment(0.25, &h, 0);
        assert!(r.changed);
        assert_eq!(r.new_diff, VARDIFF_MIN_DIFF);
    }

    #[test]
    fn vardiff_clamps_max() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 40, 800.0),
            (now - 39, 800.0),
            (now - 38, 800.0),
            (now - 37, 800.0),
            (now - 36, 800.0),
            (now - 35, 800.0),
            (now - 34, 800.0),
            (now - 33, 800.0),
            (now - 32, 800.0),
            (now - 31, 800.0),
        ]);
        let r = compute_vardiff_adjustment(800.0, &h, 0);
        assert!(r.changed);
        assert_eq!(r.new_diff, VARDIFF_MAX_DIFF);
    }

    #[test]
    fn vardiff_deadline_blocks() {
        let now = crate::modules::stratum::state::current_timestamp();
        let h = make_history(&[
            (now - 40, 1.0),
            (now - 39, 1.0),
            (now - 38, 1.0),
            (now - 37, 1.0),
            (now - 36, 1.0),
            (now - 35, 1.0),
            (now - 34, 1.0),
            (now - 33, 1.0),
            (now - 32, 1.0),
            (now - 31, 1.0),
        ]);
        let r = compute_vardiff_adjustment(1.0, &h, now + 60);
        assert!(!r.changed);
    }

    #[test]
    fn hashrate_empty() {
        let h = VecDeque::new();
        assert_eq!(compute_worker_hashrate_hs(&h, 0), 0.0);
    }

    #[test]
    fn hashrate_single_warmup() {
        let h = make_history(&[(1000, 0.2)]);
        let hr = compute_worker_hashrate_hs(&h, 1001);
        assert!(hr > 50_000_000.0 && hr < 60_000_000.0);
    }

    #[test]
    fn hashrate_two_shares() {
        let h = make_history(&[(970, 0.2), (1000, 0.2)]);
        let hr = compute_worker_hashrate_hs(&h, 1000);
        let expected = (0.4 * DIFF1_BASE) / 30.0;
        assert!((hr - expected).abs() < 0.1);
    }

    #[test]
    fn hashrate_ignores_old_shares() {
        let h = make_history(&[
            (500, 1000.0),
            (550, 1000.0),
            (600, 1000.0),
            (970, 0.2),
            (1000, 0.2),
        ]);
        let hr = compute_worker_hashrate_hs(&h, 1000);
        let expected = (0.4 * DIFF1_BASE) / 30.0;
        assert!((hr - expected).abs() < 0.1);
    }
}
