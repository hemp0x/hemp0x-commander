pub mod address;
pub mod job;
pub mod protocol;
pub mod server;
pub mod share;
pub mod state;
pub mod vardiff;

use crate::modules::stratum::address::is_routable_address;
use crate::modules::stratum::state::{
    build_status, current_timestamp, global_state, reset_state_for_stop, reset_state_stats,
    ServerState, StratumStatus,
};
use serde::Serialize;
use std::net::IpAddr;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

static STRATUM_RUNTIME: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

fn get_runtime_lock() -> &'static Mutex<Option<Runtime>> {
    STRATUM_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn default_bind_address(bind: Option<String>) -> String {
    bind.filter(|a| !a.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

fn default_port(port: Option<u16>) -> u16 {
    port.filter(|&p| p > 0).unwrap_or(3333)
}

fn is_private_lan_ip(addr: &IpAddr) -> bool {
    if addr.is_loopback() {
        return true;
    }
    match addr {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            if octets[0] == 10 {
                return true;
            }
            if octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31 {
                return true;
            }
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

fn is_valid_bind_address(addr: &str) -> bool {
    if addr.is_empty() {
        return false;
    }
    if addr == "0.0.0.0" || addr == "::" || addr == "::0" || addr == "0:0:0:0:0:0:0:0" {
        return false;
    }
    matches!(addr.parse::<IpAddr>(), Ok(ip) if is_private_lan_ip(&ip))
}

fn probe_node_ready_for_stratum(timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let mut last_error = String::new();

    while Instant::now() < deadline {
        match crate::modules::rpc::call_rpc_with_timeouts(
            "getblockchaininfo",
            &[],
            Duration::from_secs(3),
            Duration::from_secs(8),
        ) {
            Ok(info) => {
                let ibd = info
                    .get("initialblockdownload")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if ibd {
                    return Err(
                        "Node is still in initial block download. Wait for sync to finish."
                            .to_string(),
                    );
                }
                return Ok(());
            }
            Err(err) => {
                last_error = err;
                thread::sleep(Duration::from_millis(1_000));
            }
        }
    }

    if last_error.is_empty() {
        last_error = "no RPC response".to_string();
    }
    Err(format!("Core RPC is not ready for solo mining: {last_error}"))
}

#[derive(Debug, Clone, Serialize)]
pub struct BindCandidate {
    pub label: String,
    pub address: String,
    pub scope: String,
}

#[tauri::command]
pub fn get_stratum_bind_candidates() -> Result<Vec<BindCandidate>, String> {
    let mut candidates = vec![BindCandidate {
        label: "Localhost (loopback)".to_string(),
        address: "127.0.0.1".to_string(),
        scope: "loopback".to_string(),
    }];

    if let Ok(addrs) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in &addrs {
            if !is_private_lan_ip(ip) {
                continue;
            }
            if ip.is_loopback() {
                continue;
            }
            let addr_str = ip.to_string();
            if candidates.iter().any(|c| c.address == addr_str) {
                continue;
            }
            candidates.push(BindCandidate {
                label: format!("{} ({})", name, addr_str),
                address: addr_str,
                scope: "lan".to_string(),
            });
        }
    }

    Ok(candidates)
}

#[tauri::command]
pub fn start_stratum_server(
    payout_address: String,
    bind_address: Option<String>,
    port: Option<u16>,
) -> Result<StratumStatus, String> {
    let bind = default_bind_address(bind_address);
    let port = default_port(port);
    let addr = format!("{}:{}", bind, port);

    if !is_valid_bind_address(&bind) {
        return Err(
            "Only private LAN or loopback addresses are allowed. Wildcard and public bindings are not supported."
                .to_string(),
        );
    }

    if port < 1024 {
        return Err("Ports below 1024 are not allowed".to_string());
    }

    if !is_routable_address(&payout_address) {
        return Err("Invalid payout address".to_string());
    }

    {
        match probe_node_ready_for_stratum(Duration::from_secs(30)) {
            Ok(()) => {
                if let Ok(mut state) = global_state().lock() {
                    state.node_rpc_ok = true;
                    state.last_error = None;
                }
            }
            Err(err) => {
                if let Ok(mut state) = global_state().lock() {
                    state.node_rpc_ok = false;
                    state.last_error = Some(err.clone());
                }
                return Err(format!("Cannot start solo mining: {err}"));
            }
        }
    }

    {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        match state.state {
            ServerState::Running | ServerState::Starting => {
                return Ok(build_status(&*state));
            }
            _ => {}
        }
        state.state = ServerState::Starting;
        state.bind_address = bind.clone();
        state.port = port;
        state.payout_address = payout_address.clone();
        state.last_error = None;
        state.started_at = Some(current_timestamp());
    }

    let rt = Runtime::new().map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    let listener = rt
        .block_on(async {
            TcpListener::bind(&addr)
                .await
                .map_err(|e| format!("Failed to bind {}: {}", addr, e))
        })
        .map_err(|e| {
            if let Ok(mut state) = global_state().lock() {
                state.state = ServerState::Stopped;
                state.last_error = Some(e.clone());
            }
            e
        })?;

    {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        state.state = ServerState::Running;
    }

    log::info!("Stratum server bound to {}", addr);

    rt.spawn(async move {
        if let Err(e) = server::run_accept_loop(listener).await {
            if let Ok(mut state) = global_state().lock() {
                state.last_error = Some(format!("Server error: {}", e));
                if state.state != ServerState::Stopped {
                    state.state = ServerState::Error;
                }
            }
        }
    });

    {
        let mut rt_lock = get_runtime_lock()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        *rt_lock = Some(rt);
    }

    let state = global_state()
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(build_status(&*state))
}

#[tauri::command]
pub fn stop_stratum_server() -> Result<StratumStatus, String> {
    {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        match state.state {
            ServerState::Stopped | ServerState::Stopping => {
                state.state = ServerState::Stopped;
                return Ok(build_status(&*state));
            }
            _ => {}
        }
        state.state = ServerState::Stopping;
        state.stop_signal = true;
    }

    {
        let mut rt_lock = get_runtime_lock()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        if let Some(rt) = rt_lock.take() {
            drop(rt);
        }
    }

    {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        reset_state_for_stop(&mut *state);
        Ok(build_status(&*state))
    }
}

#[tauri::command]
pub fn get_stratum_status() -> Result<StratumStatus, String> {
    let state = global_state()
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    Ok(build_status(&*state))
}

#[tauri::command]
pub fn validate_stratum_address(address: String) -> Result<bool, String> {
    Ok(is_routable_address(&address))
}

#[tauri::command]
pub fn reset_stratum_stats() -> Result<StratumStatus, String> {
    let mut state = global_state()
        .lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    reset_state_stats(&mut *state);
    Ok(build_status(&*state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    fn local_state() -> state::SharedState {
        state::SharedState {
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
        }
    }

    #[test]
    fn default_bind_uses_localhost() {
        assert_eq!(default_bind_address(None), "127.0.0.1");
        assert_eq!(default_bind_address(Some("".to_string())), "127.0.0.1");
    }

    #[test]
    fn default_bind_respects_custom() {
        assert_eq!(
            default_bind_address(Some("127.0.0.1".to_string())),
            "127.0.0.1"
        );
    }

    #[test]
    fn default_port_uses_3333() {
        assert_eq!(default_port(None), 3333);
        assert_eq!(default_port(Some(0)), 3333);
    }

    #[test]
    fn default_port_respects_custom() {
        assert_eq!(default_port(Some(4444)), 4444);
    }

    #[test]
    fn is_valid_bind_allows_loopback() {
        assert!(is_valid_bind_address("127.0.0.1"));
    }

    #[test]
    fn is_valid_bind_allows_private() {
        assert!(is_valid_bind_address("192.168.1.100"));
        assert!(is_valid_bind_address("10.0.0.1"));
        assert!(is_valid_bind_address("172.16.0.1"));
        assert!(is_valid_bind_address("169.254.1.1"));
    }

    #[test]
    fn is_valid_bind_rejects_wildcard() {
        assert!(!is_valid_bind_address("0.0.0.0"));
        assert!(!is_valid_bind_address("::"));
        assert!(!is_valid_bind_address("::0"));
    }

    #[test]
    fn is_valid_bind_rejects_public() {
        assert!(!is_valid_bind_address("8.8.8.8"));
        assert!(!is_valid_bind_address("1.2.3.4"));
    }

    #[test]
    fn is_valid_bind_rejects_empty() {
        assert!(!is_valid_bind_address(""));
    }

    #[test]
    fn is_private_lan_ip_loopback() {
        assert!(is_private_lan_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_lan_ip(&"::1".parse().unwrap()));
    }

    #[test]
    fn is_private_lan_ip_rfc1918() {
        assert!(is_private_lan_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_lan_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_lan_ip(&"172.31.255.255".parse().unwrap()));
        assert!(is_private_lan_ip(&"192.168.0.1".parse().unwrap()));
    }

    #[test]
    fn is_private_lan_ip_link_local() {
        assert!(is_private_lan_ip(&"169.254.0.1".parse().unwrap()));
    }

    #[test]
    fn is_private_lan_ip_rejects_public() {
        assert!(!is_private_lan_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_lan_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_private_lan_ip(&"2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn bind_candidates_includes_localhost() {
        let candidates = get_stratum_bind_candidates().unwrap();
        assert!(candidates
            .iter()
            .any(|c| c.address == "127.0.0.1" && c.scope == "loopback"));
    }

    #[test]
    fn start_rejects_wildcard_bind() {
        let result = start_stratum_server(
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
            Some("0.0.0.0".to_string()),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn start_rejects_public_bind() {
        let result = start_stratum_server(
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
            Some("8.8.8.8".to_string()),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn start_rejects_low_port() {
        let result = start_stratum_server(
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
            None,
            Some(80),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("1024"));
    }

    #[test]
    fn start_rejects_bad_address() {
        let result = start_stratum_server("badaddress".to_string(), None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("payout address"));
    }

    #[test]
    fn validate_stratum_address_returns_true_for_valid() {
        let result = validate_stratum_address("RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string());
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn validate_stratum_address_returns_false_for_invalid() {
        let result = validate_stratum_address("badaddress".to_string());
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn reset_stratum_stats_clears_counters() {
        {
            let mut state = global_state().lock().unwrap();
            state.accepted_shares = 42;
            state.rejected_shares = 7;
            state.blocks_found = 3;
            state.block_candidates = 5;
            state.last_submit_result = Some("accepted".to_string());
            state.last_submitted_block_height = Some(1000);
            state.share_events.push_back(state::ShareEvent {
                timestamp: current_timestamp(),
                accepted: true,
                difficulty: 1.0,
            });
        }
        let status = reset_stratum_stats().unwrap();
        assert_eq!(status.accepted_shares, 0);
        assert_eq!(status.rejected_shares, 0);
        assert_eq!(status.blocks_found, 0);
        assert_eq!(status.block_candidates, 0);
        assert_eq!(status.last_submit_result, None);
        assert_eq!(status.last_submitted_block_height, None);
        assert_eq!(status.shares_per_minute, 0.0);
        assert_eq!(status.estimated_hashrate_hs, 0.0);
    }

    #[test]
    fn share_events_produce_shares_per_minute() {
        {
            let mut state = global_state().lock().unwrap();
            state.share_events.clear();
            let now = current_timestamp();
            for _ in 0..30 {
                state.share_events.push_back(state::ShareEvent {
                    timestamp: now,
                    accepted: true,
                    difficulty: 1.0,
                });
            }
        }
        let status = get_stratum_status().unwrap();
        assert!(status.shares_per_minute > 0.0);
    }

    #[test]
    fn share_events_produce_estimated_hashrate() {
        {
            let mut state = global_state().lock().unwrap();
            state.share_events.clear();
            let now = current_timestamp();
            for _ in 0..10 {
                state.share_events.push_back(state::ShareEvent {
                    timestamp: now,
                    accepted: true,
                    difficulty: 1.0,
                });
            }
        }
        let status = get_stratum_status().unwrap();
        assert!(status.estimated_hashrate_hs > 0.0);
    }

    #[test]
    fn no_events_yields_zero_rates() {
        {
            let mut state = global_state().lock().unwrap();
            state.share_events.clear();
        }
        let status = get_stratum_status().unwrap();
        assert_eq!(status.shares_per_minute, 0.0);
        assert_eq!(status.estimated_hashrate_hs, 0.0);
    }

    #[test]
    fn submission_history_is_bounded_to_100() {
        let mut state = local_state();
        for _ in 0..200 {
            state::insert_submission_pending(
                &mut state,
                1000,
                "deadbeef",
                "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM",
                "test.worker1",
            );
        }
        let len = state.submission_history.len();
        assert!(len <= 100, "expected <= 100, got {}", len);
        assert!(len > 50, "expected > 50, got {}", len);
        let status = build_status(&state);
        assert_eq!(status.submission_history.len(), 50);
    }

    #[test]
    fn reset_clears_submission_history() {
        let mut state = local_state();
        state.submission_history.push_back(state::SubmissionRecord {
            id: 1,
            timestamp: current_timestamp(),
            height: 1000,
            digest: "deadbeef".to_string(),
            payout_address: "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
            worker_id: "test.worker1".to_string(),
            status: "accepted".to_string(),
            result: None,
            error: None,
        });
        reset_state_stats(&mut state);
        let status = build_status(&state);
        assert_eq!(status.submission_history.len(), 0);
        assert_eq!(state.submission_history.len(), 0);
    }

    #[test]
    fn submission_update_status_accepted() {
        let mut state = local_state();
        let sid = state::insert_submission_pending(
            &mut state,
            1000,
            "deadbeef",
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM",
            "test.worker1",
        );
        assert_eq!(state.submission_history.len(), 1);
        assert_eq!(state.submission_history[0].status, "pending");
        state::update_submission_result(&mut state, sid, "accepted", None, None);
        assert_eq!(state.submission_history[0].status, "accepted");
        assert!(state.submission_history[0].result.is_none());
        assert!(state.submission_history[0].error.is_none());
    }

    #[test]
    fn submission_update_status_rejected() {
        let mut state = local_state();
        let sid = state::insert_submission_pending(
            &mut state,
            1000,
            "deadbeef",
            "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM",
            "test.worker1",
        );
        state::update_submission_result(&mut state, sid, "rejected", Some("high-hash"), None);
        assert_eq!(state.submission_history[0].status, "rejected");
        assert_eq!(
            state.submission_history[0].result.as_deref(),
            Some("high-hash")
        );
    }

    #[test]
    fn worker_status_includes_difficulty_and_hashrate() {
        let now = current_timestamp();
        let mut state = local_state();
        state.workers.insert(
            "session:1".to_string(),
            state::Worker {
                wallet: "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
                worker_name: "test".to_string(),
                mode: "solo".to_string(),
                connected_at: now,
                last_seen: now,
                accepted_shares: 0,
                rejected_shares: 0,
                extra_nonce_1: [0, 0, 0, 1],
                difficulty: 8.0,
                share_history: VecDeque::from(vec![(now - 59, 8.0), (now - 30, 8.0)]),
            },
        );
        let status = build_status(&state);
        assert_eq!(status.workers.len(), 1);
        let w = &status.workers[0];
        assert_eq!(w.difficulty, 8.0);
        assert!(w.estimated_hashrate_hs > 0.0);
    }

    #[test]
    fn worker_status_no_panic_empty_history() {
        let now = current_timestamp();
        let mut state = local_state();
        state.workers.insert(
            "session:1".to_string(),
            state::Worker {
                wallet: "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
                worker_name: "test".to_string(),
                mode: "solo".to_string(),
                connected_at: now,
                last_seen: now,
                accepted_shares: 0,
                rejected_shares: 0,
                extra_nonce_1: [0, 0, 0, 1],
                difficulty: 0.5,
                share_history: VecDeque::new(),
            },
        );
        let status = build_status(&state);
        assert_eq!(status.workers.len(), 1);
        let w = &status.workers[0];
        assert_eq!(w.difficulty, 0.5);
        assert_eq!(w.estimated_hashrate_hs, 0.0);
    }

    #[test]
    fn submission_history_exposed_most_recent_first() {
        let mut state = local_state();
        for i in 0..10 {
            state.submission_history.push_back(state::SubmissionRecord {
                id: i as u64,
                timestamp: current_timestamp(),
                height: 1000 + (i as u32),
                digest: format!("digest{}", i),
                payout_address: "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
                worker_id: format!("worker{}", i),
                status: if i % 3 == 0 {
                    "accepted".to_string()
                } else {
                    "pending".to_string()
                },
                result: None,
                error: None,
            });
        }
        let status = build_status(&state);
        assert_eq!(status.submission_history.len(), 10);
        assert_eq!(status.submission_history[0].id, 9);
        assert_eq!(status.submission_history[9].id, 0);
    }

    #[test]
    fn submission_counts_derived_from_history() {
        let mut state = local_state();
        for i in 0..10 {
            let status = match i % 5 {
                0 => "accepted",
                1 => "stale_orphan",
                2 => "inconclusive",
                3 => "rejected",
                _ => "rpc_error",
            };
            state.submission_history.push_back(state::SubmissionRecord {
                id: i as u64,
                timestamp: current_timestamp(),
                height: 1000 + (i as u32),
                digest: format!("digest{}", i),
                payout_address: "RXvTfjhH4FqPsjpVJJQBBG4ZW3DiLzHkgM".to_string(),
                worker_id: format!("worker{}", i),
                status: status.to_string(),
                result: None,
                error: None,
            });
        }
        let status = build_status(&state);
        assert_eq!(status.accepted_submissions, 2);
        assert_eq!(status.stale_orphan_submissions, 2);
        assert_eq!(status.inconclusive_submissions, 2);
    }

    #[test]
    fn submission_counts_zero_on_empty_history() {
        let state = local_state();
        let status = build_status(&state);
        assert_eq!(status.accepted_submissions, 0);
        assert_eq!(status.stale_orphan_submissions, 0);
        assert_eq!(status.inconclusive_submissions, 0);
    }
}
