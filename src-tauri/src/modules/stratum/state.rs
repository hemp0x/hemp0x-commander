use serde::Serialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl std::fmt::Display for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ServerState::Stopped => "STOPPED",
                ServerState::Starting => "STARTING",
                ServerState::Running => "RUNNING",
                ServerState::Stopping => "STOPPING",
                ServerState::Error => "ERROR",
            }
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    pub id: String,
    pub wallet: String,
    pub worker_name: String,
    pub mode: String,
    pub connected_at: u64,
    pub last_seen: u64,
    pub accepted_shares: u64,
    pub rejected_shares: u64,
    pub difficulty: f64,
    pub estimated_hashrate_hs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmissionRecord {
    pub id: u64,
    pub timestamp: u64,
    pub height: u32,
    pub digest: String,
    pub payout_address: String,
    pub worker_id: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StratumStatus {
    pub state: String,
    pub bind_address: String,
    pub port: u16,
    pub payout_address: String,
    pub worker_count: usize,
    pub accepted_shares: u64,
    pub rejected_shares: u64,
    pub blocks_found: u64,
    pub last_error: Option<String>,
    pub started_at: Option<u64>,
    pub workers: Vec<WorkerStatus>,
    pub current_height: Option<u32>,
    pub current_bits: Option<String>,
    pub current_target: Option<String>,
    pub template_age_secs: Option<u64>,
    pub last_job_id: Option<u32>,
    pub block_candidates: u64,
    pub template_error: Option<String>,
    pub last_block_candidate_height: Option<u32>,
    pub last_block_candidate_digest: Option<String>,
    pub last_submit_result: Option<String>,
    pub last_submitted_block_height: Option<u32>,
    pub last_submitted_block_digest: Option<String>,
    pub last_submitted_block_at: Option<u64>,
    pub shares_per_minute: f64,
    pub estimated_hashrate_hs: f64,
    pub submission_history: Vec<SubmissionRecord>,
    pub accepted_submissions: u64,
    pub stale_orphan_submissions: u64,
    pub inconclusive_submissions: u64,
    pub node_rpc_ok: bool,
}

#[derive(Debug, Clone)]
pub struct Worker {
    pub wallet: String,
    pub worker_name: String,
    pub mode: String,
    pub connected_at: u64,
    pub last_seen: u64,
    pub accepted_shares: u64,
    pub rejected_shares: u64,
    pub extra_nonce_1: [u8; 4],
    pub difficulty: f64,
    pub share_history: VecDeque<(u64, f64)>,
}

#[derive(Debug, Clone)]
pub struct ShareEvent {
    pub timestamp: u64,
    pub accepted: bool,
    pub difficulty: f64,
}

pub struct SharedState {
    pub state: ServerState,
    pub bind_address: String,
    pub port: u16,
    pub payout_address: String,
    pub accepted_shares: u64,
    pub rejected_shares: u64,
    pub blocks_found: u64,
    pub last_error: Option<String>,
    pub started_at: Option<u64>,
    pub workers: HashMap<String, Worker>,
    pub extra_nonce_counter: u32,
    pub stop_signal: bool,
    pub current_template: Option<serde_json::Value>,
    pub current_height: Option<u32>,
    pub current_bits: Option<String>,
    pub current_target: Option<String>,
    pub current_seed_hash: Option<String>,
    pub last_template_update: Option<u64>,
    pub current_job_id: u32,
    pub block_candidates: u64,
    pub template_error: Option<String>,
    pub template_generation: u64,
    pub template_clean: bool,
    pub template_wake_tx: Vec<mpsc::UnboundedSender<()>>,
    pub template_refresh_tx: Vec<mpsc::UnboundedSender<()>>,
    pub last_block_candidate_height: Option<u32>,
    pub last_block_candidate_digest: Option<String>,
    pub last_submit_result: Option<String>,
    pub last_submitted_block_height: Option<u32>,
    pub last_submitted_block_digest: Option<String>,
    pub last_submitted_block_at: Option<u64>,
    pub share_events: VecDeque<ShareEvent>,
    pub submission_history: VecDeque<SubmissionRecord>,
    pub node_rpc_ok: bool,
}

static GLOBAL: OnceLock<Mutex<SharedState>> = OnceLock::new();
static NEXT_SUBMISSION_ID: AtomicU64 = AtomicU64::new(1);

const ROLLING_WINDOW_SECS: u64 = 300;
const SHARES_PER_MIN_WINDOW_SECS: u64 = 60;

pub fn global_state() -> &'static Mutex<SharedState> {
    GLOBAL.get_or_init(|| {
        Mutex::new(SharedState {
            state: ServerState::Stopped,
            bind_address: String::new(),
            port: 3333,
            payout_address: String::new(),
            accepted_shares: 0,
            rejected_shares: 0,
            blocks_found: 0,
            last_error: None,
            started_at: None,
            workers: HashMap::new(),
            extra_nonce_counter: 0,
            stop_signal: false,
            current_template: None,
            current_height: None,
            current_bits: None,
            current_target: None,
            current_seed_hash: None,
            last_template_update: None,
            current_job_id: 0,
            block_candidates: 0,
            template_error: None,
            template_generation: 0,
            template_clean: false,
            template_wake_tx: Vec::new(),
            template_refresh_tx: Vec::new(),
            last_block_candidate_height: None,
            last_block_candidate_digest: None,
            last_submit_result: None,
            last_submitted_block_height: None,
            last_submitted_block_digest: None,
            last_submitted_block_at: None,
            share_events: VecDeque::new(),
            submission_history: VecDeque::new(),
            node_rpc_ok: true,
        })
    })
}

pub fn record_share_event(state: &mut SharedState, accepted: bool, difficulty: f64) {
    let now = current_timestamp();
    state.share_events.push_back(ShareEvent {
        timestamp: now,
        accepted,
        difficulty,
    });
    while state.share_events.len() > 10000 {
        state.share_events.pop_front();
    }
}

pub fn compute_shares_per_minute(state: &SharedState) -> f64 {
    let now = current_timestamp();
    let cutoff = now.saturating_sub(SHARES_PER_MIN_WINDOW_SECS);
    let count = state
        .share_events
        .iter()
        .rev()
        .take_while(|e| e.timestamp >= cutoff)
        .count();
    if count == 0 {
        return 0.0;
    }
    count as f64 / (SHARES_PER_MIN_WINDOW_SECS as f64 / 60.0)
}

pub fn compute_estimated_hashrate(state: &SharedState) -> f64 {
    let now = current_timestamp();
    let cutoff = now.saturating_sub(ROLLING_WINDOW_SECS);

    let window_events: Vec<&ShareEvent> = state
        .share_events
        .iter()
        .rev()
        .take_while(|e| e.timestamp >= cutoff)
        .filter(|e| e.accepted)
        .collect();

    if window_events.is_empty() {
        return 0.0;
    }

    let sum_diff: f64 = window_events.iter().map(|e| e.difficulty).sum();

    let first_ts = window_events.last().map(|e| e.timestamp).unwrap_or(now);
    let elapsed = (now.saturating_sub(first_ts)) as f64;
    let time_divisor = elapsed.max(crate::modules::stratum::vardiff::MIN_HASHRATE_SAMPLE_SECS);

    (sum_diff * crate::modules::stratum::vardiff::DIFF1_BASE) / time_divisor
}

pub fn compute_worker_hashrate_from_history(share_history: &VecDeque<(u64, f64)>, now: u64) -> f64 {
    crate::modules::stratum::vardiff::compute_worker_hashrate_hs(share_history, now)
}

pub fn build_status(state: &SharedState) -> StratumStatus {
    let now = current_timestamp();
    let mut workers: Vec<WorkerStatus> = state
        .workers
        .iter()
        .map(|(_key, w)| {
            let hr = compute_worker_hashrate_from_history(&w.share_history, now);
            WorkerStatus {
                id: format!("{}.{}", w.wallet, w.worker_name),
                wallet: w.wallet.clone(),
                worker_name: w.worker_name.clone(),
                mode: w.mode.clone(),
                connected_at: w.connected_at,
                last_seen: w.last_seen,
                accepted_shares: w.accepted_shares,
                rejected_shares: w.rejected_shares,
                difficulty: w.difficulty,
                estimated_hashrate_hs: hr,
            }
        })
        .collect();
    workers.sort_by(|a, b| a.id.cmp(&b.id));

    let template_age_secs = state
        .last_template_update
        .map(|ts| current_timestamp().saturating_sub(ts));

    let submission_history: Vec<SubmissionRecord> = state
        .submission_history
        .iter()
        .rev()
        .take(50)
        .cloned()
        .collect();

    let accepted_submissions = submission_history
        .iter()
        .filter(|r| r.status == "accepted")
        .count() as u64;
    let stale_orphan_submissions = submission_history
        .iter()
        .filter(|r| r.status == "stale_orphan")
        .count() as u64;
    let inconclusive_submissions = submission_history
        .iter()
        .filter(|r| r.status == "inconclusive")
        .count() as u64;

    StratumStatus {
        state: state.state.to_string(),
        bind_address: state.bind_address.clone(),
        port: state.port,
        payout_address: state.payout_address.clone(),
        worker_count: workers.len(),
        accepted_shares: state.accepted_shares,
        rejected_shares: state.rejected_shares,
        blocks_found: state.blocks_found,
        last_error: state.last_error.clone(),
        started_at: state.started_at,
        workers,
        current_height: state.current_height,
        current_bits: state.current_bits.clone(),
        current_target: state.current_target.clone(),
        template_age_secs,
        last_job_id: Some(state.current_job_id),
        block_candidates: state.block_candidates,
        template_error: state.template_error.clone(),
        last_block_candidate_height: state.last_block_candidate_height,
        last_block_candidate_digest: state.last_block_candidate_digest.clone(),
        last_submit_result: state.last_submit_result.clone(),
        last_submitted_block_height: state.last_submitted_block_height,
        last_submitted_block_digest: state.last_submitted_block_digest.clone(),
        last_submitted_block_at: state.last_submitted_block_at,
        shares_per_minute: compute_shares_per_minute(state),
        estimated_hashrate_hs: compute_estimated_hashrate(state),
        submission_history,
        accepted_submissions,
        stale_orphan_submissions,
        inconclusive_submissions,
        node_rpc_ok: state.node_rpc_ok,
    }
}

pub fn next_extra_nonce(state: &mut SharedState) -> [u8; 4] {
    let counter = state.extra_nonce_counter;
    state.extra_nonce_counter = state.extra_nonce_counter.wrapping_add(1);
    counter.to_be_bytes()
}

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn next_submission_id() -> u64 {
    NEXT_SUBMISSION_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn reset_state_for_stop(state: &mut SharedState) {
    state.state = ServerState::Stopped;
    state.stop_signal = false;
    state.last_error = None;
    state.started_at = None;
    state.workers.clear();
    state.accepted_shares = 0;
    state.rejected_shares = 0;
    state.blocks_found = 0;
    state.current_template = None;
    state.current_height = None;
    state.current_bits = None;
    state.current_target = None;
    state.current_seed_hash = None;
    state.last_template_update = None;
    state.current_job_id = 0;
    state.block_candidates = 0;
    state.template_error = None;
    state.template_generation = 0;
    state.template_clean = false;
    state.last_block_candidate_height = None;
    state.last_block_candidate_digest = None;
    state.last_submit_result = None;
    state.last_submitted_block_height = None;
    state.last_submitted_block_digest = None;
    state.last_submitted_block_at = None;
    state.share_events.clear();
    state.submission_history.clear();
    state.template_wake_tx.clear();
    state.template_refresh_tx.clear();
    state.node_rpc_ok = true;
}

pub fn reset_state_stats(state: &mut SharedState) {
    state.accepted_shares = 0;
    state.rejected_shares = 0;
    state.blocks_found = 0;
    state.block_candidates = 0;
    state.last_block_candidate_height = None;
    state.last_block_candidate_digest = None;
    state.last_submit_result = None;
    state.last_submitted_block_height = None;
    state.last_submitted_block_digest = None;
    state.last_submitted_block_at = None;
    state.share_events.clear();
    state.submission_history.clear();
    for w in state.workers.values_mut() {
        w.accepted_shares = 0;
        w.rejected_shares = 0;
        w.share_history.clear();
    }
}

pub fn register_wake_tx(state: &mut SharedState, tx: mpsc::UnboundedSender<()>) {
    state.template_wake_tx.push(tx);
}

pub fn broadcast_template_wake(state: &mut SharedState) {
    state.template_wake_tx.retain(|tx| tx.send(()).is_ok());
}

pub fn register_template_refresh_tx(state: &mut SharedState, tx: mpsc::UnboundedSender<()>) {
    state.template_refresh_tx.push(tx);
}

pub fn request_template_refresh(state: &mut SharedState) {
    state.template_refresh_tx.retain(|tx| tx.send(()).is_ok());
}

pub fn insert_submission_pending(
    state: &mut SharedState,
    height: u32,
    digest: &str,
    payout_address: &str,
    worker_id: &str,
) -> u64 {
    let id = next_submission_id();
    let record = SubmissionRecord {
        id,
        timestamp: current_timestamp(),
        height,
        digest: digest.to_string(),
        payout_address: payout_address.to_string(),
        worker_id: worker_id.to_string(),
        status: "pending".to_string(),
        result: None,
        error: None,
    };
    state.submission_history.push_back(record);
    while state.submission_history.len() > 100 {
        state.submission_history.pop_front();
    }
    id
}

pub fn update_submission_result(
    state: &mut SharedState,
    submission_id: u64,
    status: &str,
    result: Option<&str>,
    error: Option<&str>,
) {
    for record in state.submission_history.iter_mut().rev() {
        if record.id == submission_id {
            record.status = status.to_string();
            record.result = result.map(|s| s.to_string());
            record.error = error.map(|s| s.to_string());
            record.timestamp = current_timestamp();
            break;
        }
    }
}

pub fn get_submission_snapshot(state: &SharedState, id: u64) -> Option<SubmissionRecord> {
    state
        .submission_history
        .iter()
        .rev()
        .find(|r| r.id == id)
        .cloned()
}
