import { writable } from 'svelte/store';

export const STRATUM_POLL_INTERVAL = 5000;

export const stratumStatus = writable({
    state: "STOPPED",
    bind_address: "127.0.0.1",
    port: 3333,
    payout_address: "",
    worker_count: 0,
    accepted_shares: 0,
    rejected_shares: 0,
    blocks_found: 0,
    last_error: null,
    started_at: null,
    workers: [],
    current_height: null,
    current_bits: null,
    current_target: null,
    template_age_secs: null,
    last_job_id: null,
    block_candidates: 0,
    template_error: null,
    last_block_candidate_height: null,
    last_block_candidate_digest: null,
    last_submit_result: null,
    last_submitted_block_height: null,
    last_submitted_block_digest: null,
    last_submitted_block_at: null,
    shares_per_minute: 0,
    estimated_hashrate_hs: 0,
    submission_history: [],
    accepted_submissions: 0,
    stale_orphan_submissions: 0,
    inconclusive_submissions: 0,
});

export function formatHashrate(hs) {
    if (hs == null || hs <= 0) return '--';
    if (hs >= 1e12) return `${(hs / 1e12).toFixed(2)} TH/s`;
    if (hs >= 1e9) return `${(hs / 1e9).toFixed(2)} GH/s`;
    if (hs >= 1e6) return `${(hs / 1e6).toFixed(2)} MH/s`;
    if (hs >= 1e3) return `${(hs / 1e3).toFixed(2)} KH/s`;
    return `${hs.toFixed(0)} H/s`;
}
