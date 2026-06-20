import { writable } from 'svelte/store';

/**
 * System Status
 * Tracks the availability of the Tauri backend and core environment.
 */
export const systemStatus = writable({
    tauriReady: false,
    os: "unknown"
});

/**
 * Node Status
 * Tracks the connectivity and general health of the local Hemp0x node.
 */
/** @type {import('svelte/store').Writable<{ online: boolean, version: string, connections: number, headers: number, blocks: number, verificationProgress: number, error: string | null }>} */
export const nodeStatus = writable({
    online: false,
    version: "--",
    connections: 0,
    headers: 0,
    blocks: 0,
    verificationProgress: 0,
    error: null
});

/**
 * Network Info
 * Tracks blockchain specifics like chain type and difficulty.
 */
export const networkInfo = writable({
    chain: "mainnet",
    difficulty: 0,
    networkHashps: 0,
    testnet: false
});

/**
 * Wallet Info
 * Tracks financial data for the user.
 */
export const walletInfo = writable({
    balance: "--",
    unconfirmed: 0.0,
    immature: 0.0,
    transactions: [],
    newTxCount: 0,
    status: "--"
});

/**
 * Daemon Runtime
 * Tracks daemon lifecycle: ownership, runtime status, launch decisions.
 */
/**
 * Vault Status
 * Tracks the unified vault existence and lock state.
 */
export const vaultStatus = writable({
    exists: false,
    unlocked: false,
});

export const coreBusyUntil = writable(0);

export const daemonRuntime = writable({
    commanderOwns: false,
    bundledCoreNextReady: false,
    probe: {
        rpc_port_open: false,
        p2p_port_open: false,
        default_rpc_port: 42068,
        default_p2p_port: 42069,
    },
    daemon: {
        path: "",
        exists: false,
        raw: "",
        base_version: null,
        commit_hash: null,
        exact_core_next_match: false,
    },
    runningIdentity: {
        rpc_authenticated: false,
        base_version: null,
        subversion: null,
        build: null,
        build_commit: null,
        protocol_version: null,
        numeric_version: null,
        is_required_core_next: false,
        commit_match: false,
        commit_available: false,
        status: "",
        capabilities: {
            help_probe_success: false,
            wallet_migration: false,
            messaging: false,
            restricted_assets: false,
            qualifiers: false,
            rewards: false,
            snapshots: false,
            has_view_channel_messages: false,
            has_message_txid_lookup: false,
            detected_rpc_names: [],
        },
    },
    readiness: {
        ready: false,
        progress: "",
        elapsed_ms: 0,
        retries: 0,
        rpc_error: "",
    },
    processIdentity: {
        available: false,
        pid: null,
        exe_path: null,
        matches_bundled_path: false,
        exe_sha256: null,
        bundled_sha256: null,
        sha256_match: false,
        version_raw: null,
        version_commit_match: false,
        confidence: "none",
    },
    settings: {
        auto_start_daemon_on_launch: false,
        keep_daemon_running_on_close: false,
        allow_non_bundled_core_next: false,
    },
    conflictResolved: false,
});
