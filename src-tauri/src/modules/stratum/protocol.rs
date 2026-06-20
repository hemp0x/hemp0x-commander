use crate::modules::rpc::call_rpc;
use crate::modules::stratum::address::is_routable_address;
use crate::modules::stratum::job::{
    build_block_hex, build_coinbase_tx, build_header_hash, build_merkle_root,
    extract_template_tx_hashes, JobCache, JobData,
};
use crate::modules::stratum::share::{compare_target_256, normalize_mix_hash, normalize_nonce_64};
use crate::modules::stratum::state::{
    current_timestamp, global_state, insert_submission_pending, next_extra_nonce,
    record_share_event, request_template_refresh, update_submission_result, Worker,
};
use crate::modules::stratum::vardiff::{
    compute_vardiff_adjustment, difficulty_to_target, VARDIFF_MIN_DIFF, VARDIFF_RETARGET_TIME_SECS,
};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

pub struct ConnectionState {
    pub session_id: u64,
    pub extra_nonce_1: [u8; 4],
    pub authorized_worker_id: Option<String>,
    pub wallet: Option<String>,
    pub payout_address: Option<String>,
    pub job_cache: JobCache,
    pub last_generation: u64,
    pub difficulty: f64,
    pub share_history: std::collections::VecDeque<(u64, f64)>,
    pub last_vardiff_adjust: u64,
    pub seen_shares: std::collections::HashSet<String>,
}

impl ConnectionState {
    pub fn new() -> Self {
        let en1 = {
            let mut state = global_state().lock().unwrap();
            next_extra_nonce(&mut *state)
        };
        let now = current_timestamp();
        Self {
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::SeqCst),
            extra_nonce_1: en1,
            authorized_worker_id: None,
            wallet: None,
            payout_address: None,
            job_cache: JobCache::new(),
            last_generation: 0,
            difficulty: VARDIFF_MIN_DIFF,
            share_history: std::collections::VecDeque::new(),
            last_vardiff_adjust: now,
            seen_shares: std::collections::HashSet::new(),
        }
    }

    pub fn session_key(&self) -> String {
        format!("session:{}", self.session_id)
    }
}

pub fn validate_mode_solo(password: &str) -> Result<(), String> {
    let p = password.trim().to_lowercase();
    if p.is_empty() || p == "s" || p == "solo" {
        return Ok(());
    }
    let tokens: Vec<&str> = p
        .split(|c: char| c.is_whitespace() || ";:,|".contains(c))
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.contains(&"s") || tokens.contains(&"solo") {
        return Ok(());
    }
    Err("Commander only supports local solo mining".to_string())
}

pub async fn send_json(writer: &mut OwnedWriteHalf, value: &Value) -> Result<(), String> {
    let mut line = serde_json::to_vec(value).map_err(|e| format!("Serialize error: {}", e))?;
    line.push(b'\n');
    writer
        .write_all(&line)
        .await
        .map_err(|e| format!("Write error: {}", e))
}

pub async fn handle_message(
    writer: &mut OwnedWriteHalf,
    msg: &Value,
    conn: &mut ConnectionState,
) -> Result<(), String> {
    let method = msg["method"].as_str().unwrap_or("");
    let id = msg.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "mining.subscribe" => handle_subscribe(writer, &id, conn).await,
        "mining.authorize" => handle_authorize(writer, &id, &msg["params"], conn).await,
        "mining.configure" => {
            send_json(
                writer,
                &serde_json::json!({ "id": id, "result": true, "error": null }),
            )
            .await
        }
        "mining.extranonce.subscribe" => {
            send_json(
                writer,
                &serde_json::json!({ "id": id, "result": true, "error": null }),
            )
            .await
        }
        "mining.submit" => handle_submit(writer, &id, &msg["params"], conn).await,
        _ => {
            send_json(
                writer,
                &serde_json::json!({
                    "id": id,
                    "result": null,
                    "error": [20, format!("Unknown method: {}", method), null]
                }),
            )
            .await
        }
    }
}

async fn handle_subscribe(
    writer: &mut OwnedWriteHalf,
    id: &Value,
    conn: &ConnectionState,
) -> Result<(), String> {
    let en1_hex = hex_str(&conn.extra_nonce_1);

    send_json(
        writer,
        &serde_json::json!({
            "id": id,
            "result": [[["mining.notify", "session"]], en1_hex, 4],
            "error": null
        }),
    )
    .await?;

    send_json(
        writer,
        &serde_json::json!({
            "method": "mining.set_difficulty",
            "params": [conn.difficulty]
        }),
    )
    .await
}

async fn handle_authorize(
    writer: &mut OwnedWriteHalf,
    id: &Value,
    params: &Value,
    conn: &mut ConnectionState,
) -> Result<(), String> {
    let worker_id_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .unwrap_or("anonymous")
        .to_string();

    let password = params.get(1).and_then(|v| v.as_str()).unwrap_or("s");

    let wallet = worker_id_str.split('.').next().unwrap_or("").to_string();
    let worker_name = if worker_id_str.contains('.') {
        worker_id_str.splitn(2, '.').nth(1).unwrap_or("default")
    } else {
        "default"
    }
    .to_string();

    if !is_routable_address(&wallet) {
        send_json(
            writer,
            &serde_json::json!({
                "id": id,
                "result": false,
                "error": [20, "Invalid wallet address", null]
            }),
        )
        .await?;
        return Ok(());
    }

    if let Err(e) = validate_mode_solo(password) {
        send_json(
            writer,
            &serde_json::json!({
                "id": id,
                "result": false,
                "error": [20, e, null]
            }),
        )
        .await?;
        return Ok(());
    }

    conn.authorized_worker_id = Some(worker_id_str.clone());
    conn.wallet = Some(wallet.clone());
    conn.payout_address = Some(wallet.clone());

    let now = current_timestamp();
    let session_key = conn.session_key();

    {
        let mut state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        state.workers.insert(
            session_key,
            Worker {
                wallet: wallet.clone(),
                worker_name: worker_name.clone(),
                mode: "solo".to_string(),
                connected_at: now,
                last_seen: now,
                accepted_shares: 0,
                rejected_shares: 0,
                extra_nonce_1: conn.extra_nonce_1,
                difficulty: conn.difficulty,
                share_history: std::collections::VecDeque::new(),
            },
        );
    }

    send_json(
        writer,
        &serde_json::json!({ "id": id, "result": true, "error": null }),
    )
    .await
}

pub fn build_and_send_job(conn: &mut ConnectionState) -> Result<Option<serde_json::Value>, String> {
    let (template, job_id, seed_hash, gen, clean) = {
        let state = global_state()
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let tpl = match &state.current_template {
            Some(t) => t.clone(),
            None => return Ok(None),
        };
        (
            tpl,
            state.current_job_id,
            state.current_seed_hash.clone().unwrap_or_default(),
            state.template_generation,
            state.template_clean,
        )
    };

    if gen == conn.last_generation && !conn.job_cache.jobs.is_empty() {
        return Ok(None);
    }

    if clean {
        conn.job_cache.clear();
        conn.seen_shares.clear();
    }
    conn.last_generation = gen;

    let payout_address = match conn.payout_address.clone() {
        Some(addr) => addr,
        None => return Err("Missing payout address for Stratum job".to_string()),
    };

    let height = template.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let version = template
        .get("version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let prev_hash = template
        .get("previousblockhash")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let time = template
        .get("curtime")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let bits = template
        .get("bits")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let network_target = template
        .get("target")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::modules::stratum::job::bits_to_target(&bits));

    let extra_nonce_2 = rand_bytes_4();

    let coinbase = build_coinbase_tx(
        height,
        &payout_address,
        &conn.extra_nonce_1,
        &extra_nonce_2,
        &template,
    )?;

    let tx_hashes = extract_template_tx_hashes(&template);
    let merkle_root = build_merkle_root(&coinbase.txid, &tx_hashes);

    let header_hash = build_header_hash(version, &prev_hash, &merkle_root, time, &bits, height);

    let share_target = difficulty_to_target(conn.difficulty)?;

    let job = JobData {
        job_id,
        header_hash: header_hash.clone(),
        merkle_root,
        coinbase_tx: coinbase.tx,
        time,
        height,
        bits: bits.clone(),
        share_target: share_target.clone(),
        network_target: network_target.clone(),
        payout_address: payout_address.clone(),
        template_version: version,
        template_prev_hash: prev_hash,
        template_transactions: crate::modules::stratum::job::extract_template_transactions(
            &template,
        ),
        template_coinbase_value: template
            .get("coinbasevalue")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        clean,
    };

    conn.job_cache.insert(job);

    let notify_json = serde_json::json!({
        "method": "mining.notify",
        "params": [
            job_id.to_string(),
            header_hash,
            seed_hash,
            share_target,
            clean,
            height,
            bits,
        ]
    });

    Ok(Some(notify_json))
}

fn rand_bytes_4() -> [u8; 4] {
    rand::random::<u32>().to_be_bytes()
}

async fn handle_submit(
    writer: &mut OwnedWriteHalf,
    id: &Value,
    params: &Value,
    conn: &mut ConnectionState,
) -> Result<(), String> {
    if conn.authorized_worker_id.is_none() {
        send_json(
            writer,
            &serde_json::json!({
                "id": id,
                "result": false,
                "error": [21, "Unauthorized worker", null]
            }),
        )
        .await?;
        return Ok(());
    }

    let params_arr = params.as_array().ok_or("Invalid params")?;
    if params_arr.len() < 4 {
        reject_submit(writer, id, "Invalid share", conn).await?;
        return Ok(());
    }

    let _worker_name = params_arr[0].as_str().unwrap_or("");
    let job_id_str = params_arr[1].as_str().unwrap_or("");
    let nonce_raw = params_arr[2].as_str().unwrap_or("");
    let mut mix_hash_raw = params_arr[3].as_str().unwrap_or("");

    let submitted_job_ref = conn.job_cache.get(job_id_str);

    if let Some(job) = submitted_job_ref.cloned() {
        if params_arr.len() >= 5 {
            let p3 =
                crate::modules::stratum::share::normalize_hex(params_arr[3].as_str().unwrap_or(""));
            let p4 =
                crate::modules::stratum::share::normalize_hex(params_arr[4].as_str().unwrap_or(""));
            if p3 == job.header_hash {
                mix_hash_raw = params_arr[4].as_str().unwrap_or("");
            } else if p4 == job.header_hash {
                mix_hash_raw = params_arr[3].as_str().unwrap_or("");
            }
        }
    }

    let submitted_job = match conn.job_cache.get(job_id_str).cloned() {
        Some(j) => j,
        None => {
            send_json(
                writer,
                &serde_json::json!({
                    "id": id,
                    "result": false,
                    "error": [21, "Stale share", null]
                }),
            )
            .await?;
            increment_worker_rejected_by_key(&conn.session_key(), conn.difficulty);
            return Ok(());
        }
    };

    let nonce_hex = match normalize_nonce_64(nonce_raw) {
        Ok(n) => n,
        Err(e) => {
            reject_submit_with_msg(writer, id, &e, conn).await?;
            return Ok(());
        }
    };

    let mix_hash = match normalize_mix_hash(mix_hash_raw) {
        Some(mh) => mh,
        None => {
            reject_submit_with_msg(writer, id, "Invalid mixhash", conn).await?;
            return Ok(());
        }
    };

    let share_key = make_share_key(job_id_str, &nonce_hex, &mix_hash);
    if !conn.seen_shares.insert(share_key) {
        log::debug!(
            "Duplicate share rejected: job_id={}, nonce={}, mix_hash={}",
            job_id_str,
            nonce_hex,
            mix_hash
        );
        increment_worker_rejected_by_key(&conn.session_key(), conn.difficulty);
        send_json(
            writer,
            &serde_json::json!({
                "id": id,
                "result": false,
                "error": [22, "Duplicate share", null]
            }),
        )
        .await?;
        return Ok(());
    }

    let rpc_nonce = format!("0x{}", nonce_hex);
    let header_hash = submitted_job.header_hash.clone();
    let height = submitted_job.height;
    let share_target = submitted_job.share_target.clone();
    let network_target = submitted_job.network_target.clone();
    let mix_hash_for_rpc = mix_hash.clone();

    let kawpow_result = tokio::task::spawn_blocking(move || {
        call_rpc(
            "getkawpowhash",
            &[
                serde_json::json!(header_hash),
                serde_json::json!(mix_hash_for_rpc),
                serde_json::json!(rpc_nonce),
                serde_json::json!(height),
                serde_json::json!(share_target),
            ],
        )
    })
    .await
    .map_err(|e| format!("spawn_blocking join error: {}", e))?;

    let sk = conn.session_key();
    let diff = conn.difficulty;

    match kawpow_result {
        Ok(result) => {
            let (is_valid, meets_target) = parse_kawpow_validity(&result);
            if !is_valid || !meets_target {
                if result.get("meets_target").is_none() {
                    log::warn!(
                        "getkawpowhash response missing meets_target field — rejecting share \
                         (possible Core build mismatch)"
                    );
                }
                increment_worker_rejected_by_key(&sk, diff);
                send_json(
                    writer,
                    &serde_json::json!({
                        "id": id,
                        "result": false,
                        "error": [20, "Low difficulty share", null]
                    }),
                )
                .await?;
                return Ok(());
            }

            increment_worker_accepted_by_key(&sk, diff);

            record_share_history(&sk, diff);
            record_connection_share_history(conn, diff);
            adjust_difficulty_for_connection(conn, writer).await?;

            let digest = result["digest"].as_str().unwrap_or("");
            let is_block_candidate =
                !digest.is_empty() && compare_target_256(digest, &network_target);

            if is_block_candidate {
                let job_for_block = submitted_job.clone();
                let nonce_for_block = nonce_hex.clone();
                let mix_for_block = mix_hash.clone();
                let digest_for_block = digest.to_string();
                let payout_for_record = submitted_job.payout_address.clone();
                let worker_for_record = conn
                    .authorized_worker_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());

                let submission_id = {
                    let mut state = global_state()
                        .lock()
                        .map_err(|e| format!("Lock error: {}", e))?;
                    state.block_candidates = state.block_candidates.saturating_add(1);
                    log::info!(
                        "Block candidate detected: height={}, digest={}, network_target={}",
                        submitted_job.height,
                        digest,
                        network_target,
                    );
                    insert_submission_pending(
                        &mut state,
                        submitted_job.height,
                        &digest_for_block,
                        &payout_for_record,
                        &worker_for_record,
                    )
                };

                send_json(
                    writer,
                    &serde_json::json!({ "id": id, "result": true, "error": null }),
                )
                .await?;

                tokio::spawn(async move {
                    submit_block_candidate(
                        submission_id,
                        job_for_block,
                        nonce_for_block,
                        mix_for_block,
                        digest_for_block,
                    )
                    .await;
                });
            } else {
                send_json(
                    writer,
                    &serde_json::json!({ "id": id, "result": true, "error": null }),
                )
                .await?;
            }

            Ok(())
        }
        Err(e) => {
            conn.seen_shares
                .remove(&make_share_key(job_id_str, &nonce_hex, &mix_hash));
            let node_unavailable = is_rpc_unavailable_error(&e)
                || global_state()
                    .lock()
                    .map(|state| !state.node_rpc_ok)
                    .unwrap_or(false);
            if !node_unavailable {
                increment_worker_rejected_by_key(&sk, diff);
            }
            log::warn!("getkawpowhash RPC error: {}", e);
            send_json(
                writer,
                &serde_json::json!({
                    "id": id,
                    "result": false,
                    "error": [
                        20,
                        if node_unavailable { "Node unavailable" } else { "Share validation error" },
                        null
                    ]
                }),
            )
            .await
        }
    }
}

async fn submit_block_candidate(
    submission_id: u64,
    submitted_job: JobData,
    nonce_hex: String,
    mix_hash: String,
    digest: String,
) {
    match build_block_hex(&submitted_job, &nonce_hex, &mix_hash) {
        Ok(block_hex) => {
            let now = crate::modules::stratum::state::current_timestamp();
            let submit_result = tokio::task::spawn_blocking(move || {
                call_rpc("submitblock", &[serde_json::json!(block_hex)])
            })
            .await;

            match submit_result {
                Ok(Ok(resp)) => {
                    let is_null = resp.is_null();
                    {
                        let mut state = global_state().lock().unwrap();
                        state.last_submit_result = Some(if is_null {
                            "accepted".to_string()
                        } else {
                            resp.as_str().unwrap_or("unknown").to_string()
                        });
                        state.last_block_candidate_height = Some(submitted_job.height);
                        state.last_block_candidate_digest = Some(digest.clone());
                        state.last_submitted_block_height = Some(submitted_job.height);
                        state.last_submitted_block_digest = Some(digest.clone());
                        state.last_submitted_block_at = Some(now);
                        if is_null {
                            state.blocks_found = state.blocks_found.saturating_add(1);
                            log::info!(
                                "Block submitted successfully: height={}, digest={}",
                                submitted_job.height,
                                digest,
                            );
                            update_submission_result(
                                &mut state,
                                submission_id,
                                "accepted",
                                None,
                                None,
                            );
                            request_template_refresh(&mut *state);
                        } else {
                            let (status, result_str) = classify_submitblock_response(&resp);
                            let result_value = result_str.as_deref();
                            let needs_reconciliation = status == "pending";
                            let log_fn = if needs_reconciliation {
                                log::Level::Info
                            } else {
                                log::Level::Warn
                            };
                            log::log!(
                                log_fn,
                                "Block submission result: height={}, digest={}, status={}, result={:?}",
                                submitted_job.height,
                                digest,
                                status,
                                resp,
                            );
                            update_submission_result(
                                &mut state,
                                submission_id,
                                status,
                                result_value,
                                None,
                            );
                        }
                    }

                    let (status, _) = classify_submitblock_response(&resp);
                    if status == "pending" {
                        let height = submitted_job.height;
                        tokio::spawn(async move {
                            reconcile_submission(submission_id, height, digest).await;
                        });
                    }
                }
                Ok(Err(e)) => {
                    let now = crate::modules::stratum::state::current_timestamp();
                    let mut state = global_state().lock().unwrap();
                    state.last_submit_result = Some(format!("RPC error: {}", e));
                    state.last_block_candidate_height = Some(submitted_job.height);
                    state.last_block_candidate_digest = Some(digest.clone());
                    state.last_submitted_block_height = Some(submitted_job.height);
                    state.last_submitted_block_digest = Some(digest.clone());
                    state.last_submitted_block_at = Some(now);
                    update_submission_result(
                        &mut state,
                        submission_id,
                        "rpc_error",
                        None,
                        Some(&format!("RPC error: {}", e)),
                    );
                    log::error!(
                        "Block submit RPC error: height={}, digest={}, error={}",
                        submitted_job.height,
                        digest,
                        e,
                    );
                }
                Err(join_err) => {
                    let mut state = global_state().lock().unwrap();
                    update_submission_result(
                        &mut state,
                        submission_id,
                        "submit_error",
                        None,
                        Some(&format!("spawn error: {}", join_err)),
                    );
                    log::error!(
                        "Block submit spawn error: height={}, digest={}, error={}",
                        submitted_job.height,
                        digest,
                        join_err,
                    );
                }
            }
        }
        Err(e) => {
            let mut state = global_state().lock().unwrap();
            update_submission_result(&mut state, submission_id, "assembly_error", None, Some(&e));
            log::error!(
                "Block hex assembly error: height={}, error={}",
                submitted_job.height,
                e,
            );
        }
    }
}

async fn reconcile_submission(submission_id: u64, height: u32, digest: String) {
    tokio::time::sleep(std::time::Duration::from_secs(20)).await;

    let snapshot = {
        let state = global_state().lock().unwrap();
        crate::modules::stratum::state::get_submission_snapshot(&state, submission_id)
    };

    let snapshot = match snapshot {
        Some(s) => s,
        None => {
            log::debug!(
                "Reconciliation skipped for submission {}: evicted from history",
                submission_id
            );
            return;
        }
    };

    if snapshot.status != "pending" {
        return;
    }

    let best_height = {
        let info = tokio::task::spawn_blocking(|| call_rpc("getblockchaininfo", &[])).await;
        match info {
            Ok(Ok(resp)) => resp["blocks"].as_u64().unwrap_or(0) as u32,
            Ok(Err(e)) => {
                log_reconciliation_rpc_error(submission_id, "getblockchaininfo", &e);
                return;
            }
            Err(e) => {
                log_reconciliation_spawn_error(submission_id, &e);
                return;
            }
        }
    };

    if best_height < height {
        let note = format!(
            "Best height {} below submitted height {} after reconciliation",
            best_height, height
        );
        update_submission_status(submission_id, "inconclusive", Some(&note));
        log::info!(
            "Submission {} at height {} marked inconclusive: best height is {}",
            submission_id,
            height,
            best_height,
        );
        return;
    }

    let chain_hash = {
        let resp = tokio::task::spawn_blocking(move || {
            call_rpc("getblockhash", &[serde_json::json!(height)])
        })
        .await;
        match resp {
            Ok(Ok(val)) => val.as_str().unwrap_or("").to_string(),
            Ok(Err(e)) => {
                log_reconciliation_rpc_error(submission_id, "getblockhash", &e);
                return;
            }
            Err(e) => {
                log_reconciliation_spawn_error(submission_id, &e);
                return;
            }
        }
    };

    let (resolved_status, resolved_note) =
        resolve_chain_comparison(&digest, &chain_hash, best_height, height);

    if resolved_status == "accepted" {
        {
            let mut state = global_state().lock().unwrap();
            state.blocks_found = state.blocks_found.saturating_add(1);
            update_submission_result(&mut state, submission_id, "accepted", None, None);
            request_template_refresh(&mut *state);
        }
        log::info!(
            "Reconciliation confirmed block {} at height {} via active-chain hash match",
            digest,
            height,
        );
    } else if resolved_status == "stale_orphan" {
        let note = resolved_note
            .unwrap_or_else(|| format!("Unable to reconcile submitted block at height {}", height));
        update_submission_status(submission_id, resolved_status, Some(&note));
        log::warn!(
            "Submission {} at height {} marked stale/orphan. {}",
            submission_id,
            height,
            note,
        );
        {
            let mut state = global_state().lock().unwrap();
            request_template_refresh(&mut *state);
        }
    } else {
        update_submission_status(submission_id, resolved_status, resolved_note.as_deref());
        log::warn!(
            "Submission {} at height {} marked {}. {:?}",
            submission_id,
            height,
            resolved_status,
            resolved_note,
        );
    }
}

fn update_submission_status(submission_id: u64, status: &str, error: Option<&str>) {
    let mut state = global_state().lock().unwrap();
    update_submission_result(&mut state, submission_id, status, None, error);
}

fn log_reconciliation_rpc_error(submission_id: u64, method: &str, error: &str) {
    let mut state = global_state().lock().unwrap();
    update_submission_result(
        &mut state,
        submission_id,
        "rpc_error",
        None,
        Some(&format!("Reconciliation {} RPC error: {}", method, error)),
    );
    log::error!(
        "Reconciliation {} RPC error for submission {}: {}",
        method,
        submission_id,
        error,
    );
}

fn log_reconciliation_spawn_error(submission_id: u64, error: &dyn std::fmt::Display) {
    let mut state = global_state().lock().unwrap();
    update_submission_result(
        &mut state,
        submission_id,
        "rpc_error",
        None,
        Some(&format!("Reconciliation spawn error: {}", error)),
    );
    log::error!(
        "Reconciliation spawn error for submission {}: {}",
        submission_id,
        error,
    );
}

fn classify_submitblock_response(resp: &serde_json::Value) -> (&'static str, Option<String>) {
    if resp.is_null() {
        return ("accepted", None);
    }

    let result = resp.as_str().unwrap_or("unknown").to_string();
    if result.eq_ignore_ascii_case("inconclusive") {
        ("pending", Some(result))
    } else {
        ("rejected", Some(result))
    }
}

pub fn resolve_chain_comparison(
    digest: &str,
    chain_hash: &str,
    best_height: u32,
    submitted_height: u32,
) -> (&'static str, Option<String>) {
    if best_height < submitted_height {
        return (
            "inconclusive",
            Some(format!(
                "Best height {} below submitted height {}",
                best_height, submitted_height
            )),
        );
    }
    if chain_hash.eq_ignore_ascii_case(digest) {
        ("accepted", None)
    } else {
        (
            "stale_orphan",
            Some(format!(
                "Active chain has {} at height {}",
                chain_hash, submitted_height
            )),
        )
    }
}

async fn reject_submit(
    writer: &mut OwnedWriteHalf,
    id: &Value,
    _reason: &str,
    conn: &mut ConnectionState,
) -> Result<(), String> {
    increment_worker_rejected_by_key(&conn.session_key(), conn.difficulty);
    send_json(
        writer,
        &serde_json::json!({ "id": id, "result": false, "error": [20, "Invalid share", null] }),
    )
    .await
}

async fn reject_submit_with_msg(
    writer: &mut OwnedWriteHalf,
    id: &Value,
    msg: &str,
    conn: &mut ConnectionState,
) -> Result<(), String> {
    increment_worker_rejected_by_key(&conn.session_key(), conn.difficulty);
    send_json(
        writer,
        &serde_json::json!({ "id": id, "result": false, "error": [20, msg, null] }),
    )
    .await
}

fn increment_worker_accepted_by_key(session_key: &str, difficulty: f64) {
    if let Ok(mut state) = global_state().lock() {
        if let Some(w) = state.workers.get_mut(session_key) {
            w.accepted_shares = w.accepted_shares.saturating_add(1);
        }
        state.accepted_shares = state.accepted_shares.saturating_add(1);
        record_share_event(&mut *state, true, difficulty);
    }
}

fn increment_worker_rejected_by_key(session_key: &str, difficulty: f64) {
    if let Ok(mut state) = global_state().lock() {
        if let Some(w) = state.workers.get_mut(session_key) {
            w.rejected_shares = w.rejected_shares.saturating_add(1);
        }
        state.rejected_shares = state.rejected_shares.saturating_add(1);
        record_share_event(&mut *state, false, difficulty);
    }
}

fn record_share_history(session_key: &str, difficulty: f64) {
    let now = current_timestamp();
    if let Ok(mut state) = global_state().lock() {
        if let Some(w) = state.workers.get_mut(session_key) {
            w.share_history.push_back((now, difficulty));
            while w.share_history.len() > 2000 {
                w.share_history.pop_front();
            }
        }
    }
}

fn record_connection_share_history(conn: &mut ConnectionState, difficulty: f64) {
    let now = current_timestamp();
    conn.share_history.push_back((now, difficulty));
    while conn.share_history.len() > 2000 {
        conn.share_history.pop_front();
    }
}

async fn adjust_difficulty_for_connection(
    conn: &mut ConnectionState,
    writer: &mut OwnedWriteHalf,
) -> Result<(), String> {
    let deadline = conn
        .last_vardiff_adjust
        .saturating_add(VARDIFF_RETARGET_TIME_SECS);
    let network_diff = global_state()
        .lock()
        .ok()
        .and_then(|s| s.current_bits.clone())
        .and_then(|bits| crate::modules::stratum::job::bits_to_difficulty(&bits).ok());
    let adj =
        compute_vardiff_adjustment(conn.difficulty, &conn.share_history, deadline, network_diff);

    if adj.changed {
        conn.difficulty = adj.new_diff;
        conn.last_vardiff_adjust = current_timestamp();

        let sk = conn.session_key();
        if let Ok(mut state) = global_state().lock() {
            if let Some(w) = state.workers.get_mut(&sk) {
                w.difficulty = adj.new_diff;
            }
        }

        send_json(
            writer,
            &serde_json::json!({
                "method": "mining.set_difficulty",
                "params": [adj.new_diff]
            }),
        )
        .await?;

        log::info!(
            "VARDIFF adjust session {}: {:.2}",
            conn.session_id,
            adj.new_diff
        );
    }

    Ok(())
}

fn hex_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn parse_kawpow_validity(result: &Value) -> (bool, bool) {
    let is_valid =
        result["result"].as_str() == Some("true") || result["result"].as_bool() == Some(true);
    let meets_target = result["meets_target"].as_str() == Some("true")
        || result["meets_target"].as_bool() == Some(true);
    (is_valid, meets_target)
}

pub fn make_share_key(job_id_str: &str, nonce_hex: &str, mix_hash: &str) -> String {
    format!("{}:{}:{}", job_id_str, nonce_hex, mix_hash)
}

fn is_rpc_unavailable_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connection aborted")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("transport")
        || lower.contains("rpc unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_mode_solo_accepts_valid() {
        assert!(validate_mode_solo("").is_ok());
        assert!(validate_mode_solo("s").is_ok());
        assert!(validate_mode_solo("solo").is_ok());
    }

    #[test]
    fn validate_mode_solo_rejects_pool() {
        assert!(validate_mode_solo("x").is_err());
        assert!(validate_mode_solo("pool").is_err());
    }

    #[test]
    fn validate_mode_solo_rejects_unknown() {
        assert!(validate_mode_solo("abc").is_err());
    }

    #[test]
    fn hex_str_output() {
        assert_eq!(hex_str(&[0xab, 0xcd, 0xef]), "abcdef");
    }

    #[test]
    fn connection_state_session_key_unique() {
        let c1 = ConnectionState::new();
        let c2 = ConnectionState::new();
        assert_ne!(c1.session_id, c2.session_id);
    }

    #[test]
    fn connection_state_extra_nonce_stable() {
        let c = ConnectionState::new();
        let en1 = c.extra_nonce_1;
        assert_eq!(c.extra_nonce_1, en1);
    }

    #[test]
    fn rand_bytes_4_roundtrip() {
        let b = rand_bytes_4();
        assert_eq!(b.len(), 4);
    }

    #[test]
    fn classify_submitblock_null_as_accepted() {
        let (status, result) = classify_submitblock_response(&serde_json::Value::Null);
        assert_eq!(status, "accepted");
        assert!(result.is_none());
    }

    #[test]
    fn classify_submitblock_inconclusive_as_pending() {
        let (status, result) =
            classify_submitblock_response(&serde_json::Value::String("inconclusive".to_string()));
        assert_eq!(status, "pending");
        assert_eq!(result.as_deref(), Some("inconclusive"));
    }

    #[test]
    fn classify_submitblock_string_as_rejected() {
        let (status, result) =
            classify_submitblock_response(&serde_json::Value::String("duplicate".to_string()));
        assert_eq!(status, "rejected");
        assert_eq!(result.as_deref(), Some("duplicate"));
    }

    #[test]
    fn resolve_chain_comparison_matching_hash_accepted() {
        let (status, note) = resolve_chain_comparison("deadbeef", "deadbeef", 100, 100);
        assert_eq!(status, "accepted");
        assert!(note.is_none());
    }

    #[test]
    fn resolve_chain_comparison_different_hash_stale_orphan() {
        let (status, note) = resolve_chain_comparison("abc", "def", 100, 100);
        assert_eq!(status, "stale_orphan");
        assert!(note.is_some());
        assert!(note.unwrap().contains("Active chain has def"));
    }

    #[test]
    fn resolve_chain_comparison_best_height_below_submitted_inconclusive() {
        let (status, note) = resolve_chain_comparison("abc", "abc", 99, 100);
        assert_eq!(status, "inconclusive");
        assert!(note.is_some());
        assert!(note.unwrap().contains("Best height 99 below"));
    }

    #[test]
    fn resolve_chain_comparison_case_insensitive_hash_comparison() {
        let (status, note) = resolve_chain_comparison("DeAdBeEf", "deadbeef", 100, 100);
        assert_eq!(status, "accepted");
        assert!(note.is_none());
    }

    #[test]
    fn parse_kawpow_validity_result_true_meets_target_true() {
        let r = serde_json::json!({"result": true, "meets_target": true});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(valid);
        assert!(meets);
    }

    #[test]
    fn parse_kawpow_validity_result_true_meets_target_false() {
        let r = serde_json::json!({"result": true, "meets_target": false});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(valid);
        assert!(!meets);
    }

    #[test]
    fn parse_kawpow_validity_result_false() {
        let r = serde_json::json!({"result": false, "meets_target": true});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(!valid);
        assert!(meets);
    }

    #[test]
    fn parse_kawpow_validity_missing_meets_target() {
        let r = serde_json::json!({"result": true});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(valid);
        assert!(!meets);
    }

    #[test]
    fn parse_kawpow_validity_string_true() {
        let r = serde_json::json!({"result": "true", "meets_target": "true"});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(valid);
        assert!(meets);
    }

    #[test]
    fn parse_kawpow_validity_string_false() {
        let r = serde_json::json!({"result": "true", "meets_target": "false"});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(valid);
        assert!(!meets);
    }

    #[test]
    fn parse_kawpow_validity_both_false() {
        let r = serde_json::json!({"result": false, "meets_target": false});
        let (valid, meets) = parse_kawpow_validity(&r);
        assert!(!valid);
        assert!(!meets);
    }

    #[test]
    fn make_share_key_deterministic() {
        let k1 = make_share_key("42", "0000000000000001", "abcdef");
        let k2 = make_share_key("42", "0000000000000001", "abcdef");
        assert_eq!(k1, k2);
    }

    #[test]
    fn make_share_key_different_for_different_nonce() {
        let k1 = make_share_key("42", "0000000000000001", "abcdef");
        let k2 = make_share_key("42", "0000000000000002", "abcdef");
        assert_ne!(k1, k2);
    }

    #[test]
    fn make_share_key_different_for_different_job() {
        let k1 = make_share_key("42", "0000000000000001", "abcdef");
        let k2 = make_share_key("43", "0000000000000001", "abcdef");
        assert_ne!(k1, k2);
    }

    #[test]
    fn make_share_key_different_for_different_mix_hash() {
        let k1 = make_share_key("42", "0000000000000001", "abcdef");
        let k2 = make_share_key("42", "0000000000000001", "123456");
        assert_ne!(k1, k2);
    }

    #[test]
    fn duplicate_share_tracking_basic() {
        let mut conn = ConnectionState::new();
        let key = make_share_key("1", "abcdef0123456789", "feedface");
        assert!(conn.seen_shares.insert(key.clone()));
        assert!(!conn.seen_shares.insert(key));
    }

    #[test]
    fn duplicate_share_tracking_different_jobs_independent() {
        let mut conn = ConnectionState::new();
        let k1 = make_share_key("1", "abcdef0123456789", "feedface");
        let k2 = make_share_key("2", "abcdef0123456789", "feedface");
        assert!(conn.seen_shares.insert(k1));
        assert!(conn.seen_shares.insert(k2));
    }

    #[test]
    fn duplicate_share_tracking_cleared_on_new_job() {
        let mut conn = ConnectionState::new();
        let key = make_share_key("1", "abcdef0123456789", "feedface");
        conn.seen_shares.insert(key.clone());
        assert!(!conn.seen_shares.is_empty());
        conn.seen_shares.clear();
        assert!(conn.seen_shares.is_empty());
    }

    #[test]
    fn rpc_unavailable_error_detects_transport_failures() {
        assert!(is_rpc_unavailable_error(
            "Connection refused (os error 111)"
        ));
        assert!(is_rpc_unavailable_error("request timed out"));
        assert!(!is_rpc_unavailable_error("invalid params"));
    }
}
