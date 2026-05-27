use crate::modules::rpc::call_rpc;
use crate::modules::stratum::job::{bits_to_target, extract_template_tx_hashes, get_seed_hash};
use crate::modules::stratum::protocol::{
    build_and_send_job, handle_message, send_json, ConnectionState,
};
use crate::modules::stratum::state::{
    broadcast_template_wake, current_timestamp, global_state, register_template_refresh_tx,
    register_wake_tx,
};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

const TEMPLATE_POLL_INTERVAL_SECS: u64 = 1;

pub async fn run_accept_loop(listener: TcpListener) -> Result<(), String> {
    log::info!("Stratum server accept loop started");

    let mut poll_interval = interval(Duration::from_secs(TEMPLATE_POLL_INTERVAL_SECS));
    let mut template_refresh_rx = {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let (tx, rx) = mpsc::unbounded_channel();
        register_template_refresh_tx(&mut *state, tx);
        rx
    };

    loop {
        {
            let state = global_state()
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            if state.stop_signal {
                break;
            }
        }

        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        log::info!("Stratum connection from {}", peer_addr);
                        tokio::spawn(handle_connection(stream));
                    }
                    Err(e) => {
                        {
                            let state = global_state().lock().map_err(|e| format!("Lock error: {}", e))?;
                            if state.stop_signal {
                                break;
                            }
                        }
                        log::error!("Accept error: {}", e);
                    }
                }
            }
            _ = poll_interval.tick() => {
                let stop = {
                    let state = global_state().lock().map_err(|e| format!("Lock error: {}", e))?;
                    state.stop_signal
                };
                if stop {
                    break;
                }
                if let Err(e) = update_template().await {
                    log::warn!("Template poll error: {}", e);
                }
            }
            Some(()) = template_refresh_rx.recv() => {
                let stop = {
                    let state = global_state().lock().map_err(|e| format!("Lock error: {}", e))?;
                    state.stop_signal
                };
                if stop {
                    break;
                }
                log::info!("Template refresh triggered by submission result");
                if let Err(e) = update_template().await {
                    log::warn!("Template refresh error: {}", e);
                }
            }
        }
    }

    log::info!("Stratum server accept loop exiting");
    Ok(())
}

async fn update_template() -> Result<(), String> {
    let result = call_rpc("getblocktemplate", &[serde_json::json!({})]);

    match result {
        Ok(template) => {
            let prev_hash = template["previousblockhash"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let height = template["height"].as_u64().map(|v| v as u32);
            let _curtime = template["curtime"].as_u64().map(|v| v as u32).unwrap_or(0);
            let bits = template["bits"].as_str().unwrap_or("").to_string();
            let target = template["target"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| bits_to_target(&bits));
            let tx_hashes = extract_template_tx_hashes(&template);

            let mut state = global_state()
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;

            let previous_hash = state
                .current_template
                .as_ref()
                .and_then(|t| t["previousblockhash"].as_str())
                .map(|s| s.to_string());

            let changed = should_broadcast_template_change(
                state.current_template.as_ref(),
                &prev_hash,
                template["bits"].as_str(),
                template["transactions"].as_array().map(|a| a.len()),
            );

            if changed {
                let new_prev =
                    previous_hash.is_some() && previous_hash.as_deref() != Some(&prev_hash);
                state.current_template = Some(template.clone());
                state.current_height = height;
                state.current_bits = Some(bits.clone());
                state.current_target = Some(target.clone());
                state.last_template_update = Some(current_timestamp());
                state.template_error = None;
                state.current_job_id = (state.current_job_id + 1) % 10000;
                state.template_generation = state.template_generation.wrapping_add(1);
                state.template_clean = new_prev || state.current_seed_hash.is_none();

                let sh = get_seed_hash(height.unwrap_or(0));
                state.current_seed_hash = Some(sh.clone());

                broadcast_template_wake(&mut *state);

                log::info!(
                    "Template changed: gen={} height={:?} job_id={}",
                    state.template_generation,
                    height,
                    state.current_job_id,
                );
            } else {
                state.last_template_update = Some(current_timestamp());
                state.template_error = None;

                if let Ok(_current) = serde_json::to_string(&template) {
                    let _ = tx_hashes;
                }
            }

            Ok(())
        }
        Err(e) => {
            let mut state = global_state()
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            state.template_error = Some(format!("Template RPC error: {}", e));
            state.last_error = Some(format!("Template RPC error: {}", e));
            log::warn!("Block template update failed: {}", e);
            Err(e)
        }
    }
}

fn should_broadcast_template_change(
    current_template: Option<&serde_json::Value>,
    next_prev_hash: &str,
    next_bits: Option<&str>,
    next_tx_count: Option<usize>,
) -> bool {
    let Some(current) = current_template else {
        return true;
    };

    current.get("previousblockhash").and_then(|v| v.as_str()) != Some(next_prev_hash)
        || current.get("bits").and_then(|v| v.as_str()) != next_bits
        || current
            .get("transactions")
            .and_then(|v| v.as_array().map(|a| a.len()))
            != next_tx_count
}

async fn handle_connection(stream: tokio::net::TcpStream) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let mut conn = ConnectionState::new();

    let (wake_tx, mut wake_rx) = mpsc::unbounded_channel();

    {
        if let Ok(mut state) = global_state().lock() {
            register_wake_tx(&mut *state, wake_tx);
        }
    }

    let result: Result<(), String> = async {
        loop {
            tokio::select! {
                read_result = reader.read_line(&mut line) => {
                    match read_result {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() || !trimmed.starts_with('{') {
                                line.clear();
                                continue;
                            }
                            match serde_json::from_str::<Value>(trimmed) {
                                Ok(msg) => {
                                    let was_authorized = conn.authorized_worker_id.is_some();

                                    if let Err(e) = handle_message(&mut write_half, &msg, &mut conn).await {
                                        log::warn!("Handler error for session {}: {}", conn.session_id, e);
                                    }

                                    if conn.authorized_worker_id.is_some() {
                                        let sk = conn.session_key();
                                        if let Ok(mut state) = global_state().lock() {
                                            if let Some(w) = state.workers.get_mut(&sk) {
                                                w.last_seen = current_timestamp();
                                            }
                                        }
                                        if !was_authorized {
                                            match build_and_send_job(&mut conn) {
                                                Ok(Some(job_json)) => {
                                                    let _ = send_json(&mut write_half, &job_json).await;
                                                }
                                                Ok(None) => {}
                                                Err(e) => {
                                                    log::warn!("Initial job build error for session {}: {}", conn.session_id, e);
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Invalid JSON from session {}: {}", conn.session_id, e);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Read error from session {}: {}", conn.session_id, e);
                            break;
                        }
                    }
                    line.clear();
                }
                Some(()) = wake_rx.recv() => {
                    if conn.authorized_worker_id.is_some() {
                        match build_and_send_job(&mut conn) {
                            Ok(Some(job_json)) => {
                                let _ = send_json(&mut write_half, &job_json).await;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                log::warn!("Job build error for session {}: {}", conn.session_id, e);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    if let Err(e) = result {
        log::warn!("Connection error: {}", e);
    }

    let sk = conn.session_key();
    if let Ok(mut state) = global_state().lock() {
        state.workers.remove(&sk);
    }
    log::info!("Session {} disconnected", conn.session_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(prev_hash: &str, curtime: u64, bits: &str, tx_count: usize) -> serde_json::Value {
        let txs: Vec<serde_json::Value> = (0..tx_count)
            .map(|i| serde_json::json!({ "hash": format!("tx{}", i) }))
            .collect();
        serde_json::json!({
            "previousblockhash": prev_hash,
            "curtime": curtime,
            "bits": bits,
            "transactions": txs,
        })
    }

    #[test]
    fn template_change_broadcasts_first_template() {
        assert!(should_broadcast_template_change(
            None,
            "prev1",
            Some("1d00ffff"),
            Some(0),
        ));
    }

    #[test]
    fn template_change_ignores_curtime_only() {
        let current = template("prev1", 100, "1d00ffff", 0);
        assert!(!should_broadcast_template_change(
            Some(&current),
            "prev1",
            Some("1d00ffff"),
            Some(0),
        ));
    }

    #[test]
    fn template_change_broadcasts_previous_hash_change() {
        let current = template("prev1", 100, "1d00ffff", 0);
        assert!(should_broadcast_template_change(
            Some(&current),
            "prev2",
            Some("1d00ffff"),
            Some(0),
        ));
    }

    #[test]
    fn template_change_broadcasts_bits_change() {
        let current = template("prev1", 100, "1d00ffff", 0);
        assert!(should_broadcast_template_change(
            Some(&current),
            "prev1",
            Some("1d00fffe"),
            Some(0),
        ));
    }

    #[test]
    fn template_change_broadcasts_transaction_count_change() {
        let current = template("prev1", 100, "1d00ffff", 0);
        assert!(should_broadcast_template_change(
            Some(&current),
            "prev1",
            Some("1d00ffff"),
            Some(1),
        ));
    }
}
