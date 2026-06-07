import { writable } from 'svelte/store';

export const STRATUM_POLL_INTERVAL = 5000;

/**
 * @typedef {{
 *   id?: number,
 *   timestamp?: number,
 *   height?: number,
 *   digest?: string,
 *   payout_address?: string,
 *   worker_id?: string,
 *   status?: string,
 *   result?: string | null,
 *   error?: string | null
 * }} StratumSubmission
 *
 * @typedef {{
 *   id?: string,
 *   worker_name: string,
 *   mode?: string,
 *   connected_at?: number,
 *   hashrate_hs?: number,
 *   estimated_hashrate_hs?: number,
 *   accepted_shares?: number,
 *   rejected_shares?: number,
 *   last_share_at?: number | null,
 *   last_seen?: number,
 *   connected?: boolean,
 *   difficulty?: number,
 *   wallet?: string
 * }} StratumWorker
 */

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
    /** @type {StratumWorker[]} */
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
    /** @type {string | null} */
    last_submit_result: null,
    last_submitted_block_height: null,
    last_submitted_block_digest: null,
    last_submitted_block_at: null,
    shares_per_minute: 0,
    estimated_hashrate_hs: 0,
    /** @type {StratumSubmission[]} */
    submission_history: [],
    accepted_submissions: 0,
    stale_orphan_submissions: 0,
    inconclusive_submissions: 0,
});

/** @param {number | null | undefined} hs */
export function formatHashrate(hs) {
    if (hs == null || hs <= 0) return '--';
    if (hs >= 1e12) return `${(hs / 1e12).toFixed(2)} TH/s`;
    if (hs >= 1e9) return `${(hs / 1e9).toFixed(2)} GH/s`;
    if (hs >= 1e6) return `${(hs / 1e6).toFixed(2)} MH/s`;
    if (hs >= 1e3) return `${(hs / 1e3).toFixed(2)} KH/s`;
    return `${hs.toFixed(0)} H/s`;
}
