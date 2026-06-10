import { core } from "@tauri-apps/api";

const HC_MAGIC_HEX = "4843";

function normalizeChannel(channel) {
    if (!channel) return "";
    return channel.replace(/!$/, "").trim().toUpperCase();
}

export function canonicalMessageChannel(msg) {
    if (msg.authority_asset) return normalizeChannel(msg.authority_asset);
    if (msg.channel) return normalizeChannel(msg.channel);
    if (msg.asset_name) return normalizeChannel(msg.asset_name);
    return "";
}

function matchTxid(txid, prefix, suffix) {
    if (!txid || txid.length < prefix.length + suffix.length) return false;
    const lower = txid.toLowerCase();
    return lower.startsWith(prefix) && lower.endsWith(suffix);
}

export function controlMessageKey(msg) {
    return [
        canonicalMessageChannel(msg),
        msg.txid || "",
        msg.time || "",
        msg.block_height || "",
        msg.message || "",
    ].join("|");
}

export function isHcControlFrame(hex) {
    if (!hex || hex.length < 4) return false;
    return hex.slice(0, 4).toLowerCase() === HC_MAGIC_HEX;
}

export async function buildDeleteCommandHex(txid) {
    return await core.invoke("h0xc_control_encode_delete", { txid });
}

export async function buildLeaveCommandHex() {
    return await core.invoke("h0xc_control_encode_leave");
}

export async function buildStatusCommandHex(status, expiryMode, expiryValue) {
    return await core.invoke("h0xc_control_encode_status", {
        status,
        expiryMode,
        expiryValue,
    });
}

export async function buildReportMessageCommandHex(txid, reason, severity, durationDays) {
    return await core.invoke("h0xc_control_encode_report_message", {
        txid,
        reason,
        severity,
        durationDays,
    });
}

export async function buildReportChannelCommandHex(channel, reason, severity, durationDays) {
    return await core.invoke("h0xc_control_encode_report_channel", {
        channel,
        reason,
        severity,
        durationDays,
    });
}

// --- Constants ---

export const STATUS_LABELS = {
    0: "Available",
    1: "Away",
    2: "Do Not Disturb",
    3: "Hidden",
    4: "Clear",
};

export const STATUS_ICONS = {
    0: "●",
    1: "◐",
    2: "⊘",
    3: "◌",
    4: "✕",
};

export const REASON_LABELS = {
    1: "Spam",
    2: "Abuse",
    3: "Scam",
    4: "Off-topic",
    5: "Other",
};

export const SEVERITY_LABELS = {
    1: "Low",
    2: "Medium",
    3: "High",
};

// --- Status helpers ---

/**
 * Check if a decoded status command is expired.
 * @param {{status_expiry_mode?: number, status_expiry_value?: number}} parsed
 * @returns {boolean}
 */
export function isStatusExpired(parsed) {
    if (!parsed || parsed.status_expiry_mode === undefined) return false;
    const mode = parsed.status_expiry_mode;
    if (mode === 3) return false; // until changed
    if (mode === 0) {
        // default 24 hours from when the status command was broadcast
        return false; // cannot check without command timestamp; treat as not expired
    }
    if (mode === 1) {
        // duration hours - cannot check without command timestamp; treat as not expired
        return false;
    }
    if (mode === 2) {
        // absolute UTC timestamp
        const now = Math.floor(Date.now() / 1000);
        return parsed.status_expiry_value < now;
    }
    return false;
}

/**
 * Compute the effective expiry timestamp for a status command.
 * @param {{status_expiry_mode?: number, status_expiry_value?: number, time?: string|number}} parsed
 * @param {string|number} commandTime - the message timestamp (ms or seconds)
 * @returns {number} expiry as seconds since epoch, or Infinity if until-changed/no-expiry
 */
export function statusExpiryTimestamp(parsed, commandTime) {
    if (!parsed || parsed.status_expiry_mode === undefined) return Infinity;
    const mode = parsed.status_expiry_mode;
    if (mode === 3) return Infinity;
    if (mode === 2) return parsed.status_expiry_value;
    const cmdTsSec = typeof commandTime === "number"
        ? (commandTime > 1e12 ? Math.floor(commandTime / 1000) : commandTime)
        : Math.floor(Date.now() / 1000);
    if (mode === 0) return cmdTsSec + 86400; // default 24h
    if (mode === 1) return cmdTsSec + parsed.status_expiry_value * 3600;
    return Infinity;
}

// --- Channel fingerprint (mirrors Rust FNV-1a) ---

/**
 * Deterministic 16-byte fingerprint from a canonical channel name.
 * Matches the Rust channel_fingerprint() function.
 * @param {string} channel
 * @returns {string} hex string of 16 bytes
 */
export function channelFingerprintHex(channel) {
    const canonical = (channel || "").replace(/!$/, "").trim().toUpperCase();
    let hash = 0xcbf29ce484222325n;
    const prime = 0x100000001b3n;
    const mask = 0xffffffffffffffffn;
    for (let i = 0; i < canonical.length; i++) {
        hash ^= BigInt(canonical.charCodeAt(i) & 0xff);
        hash = (hash * prime) & mask;
    }

    const h2 = hash ^ 0x5bd1e995f5a0c2b3n;
    const result = new Uint8Array(16);
    for (let i = 0; i < 8; i++) {
        result[i] = Number((hash >> BigInt(i * 8)) & 0xffn);
        result[i + 8] = Number((h2 >> BigInt(i * 8)) & 0xffn);
    }

    return Array.from(result).map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Check if a 16-byte hex fingerprint matches a channel name.
 * @param {string} fingerprintHex
 * @param {string} channel
 * @returns {boolean}
 */
export function fingerprintMatchesChannel(fingerprintHex, channel) {
    return fingerprintHex === channelFingerprintHex(channel);
}

// --- Report helpers ---

/**
 * Check if a report effect has expired.
 * @param {number} durationDays
 * @param {number} reportTimeSec - when the report was broadcast (seconds)
 * @returns {boolean}
 */
export function isReportExpired(durationDays, reportTimeSec) {
    if (durationDays === 0) {
        // default 30 days
        durationDays = 30;
    }
    const now = Math.floor(Date.now() / 1000);
    const expiry = reportTimeSec + durationDays * 86400;
    return now > expiry;
}

/**
 * Aggregate community reports for a target.
 * @param {Array<{target: string, channel: string, reason: number, severity: number, durationDays: number, timeSec: number}>} allReports
 * @param {Set<string>} blockedChannels - channels to ignore reports from
 * @param {number} windowDays - aggregation window
 * @returns {Map<string, {count: number, channels: Set<string>, maxSeverity: number, maxDuration: number, latestTime: number}>}
 */
export function aggregateReports(allReports, blockedChannels, windowDays) {
    const windowSec = windowDays * 86400;
    const now = Math.floor(Date.now() / 1000);
    const cutoff = now - windowSec;

    /** @type {Map<string, {count: number, channels: Set<string>, maxSeverity: number, maxDuration: number, latestTime: number}>} */
    const byTarget = new Map();

    for (const r of allReports) {
        if (r.timeSec < cutoff) continue;
        if (blockedChannels.has(r.channel)) continue;

        const key = `${r.targetType}:${r.target}`;
        let entry = byTarget.get(key);
        if (!entry) {
            entry = { count: 0, channels: new Set(), maxSeverity: 0, maxDuration: 0, latestTime: 0 };
            byTarget.set(key, entry);
        }
        if (entry.channels.has(r.channel)) continue;
        entry.channels.add(r.channel);
        entry.count = entry.channels.size;
        if (r.severity > entry.maxSeverity) entry.maxSeverity = r.severity;
        if (r.durationDays > entry.maxDuration) entry.maxDuration = r.durationDays;
        if (r.timeSec > entry.latestTime) entry.latestTime = r.timeSec;
    }

    return byTarget;
}

/**
 * Check if a target should be auto-hidden based on community reports.
 * @param {{count: number, channels: Set<string>}} aggregation
 * @param {number} minReports
 * @param {number} minRatio
 * @param {number} totalParticipants - non-blocked recent participants
 * @returns {boolean}
 */
export function shouldAutoHide(aggregation, minReports, minRatio, totalParticipants) {
    if (aggregation.channels.size < minReports) return false;
    if (totalParticipants <= 0) return false;
    const ratio = aggregation.channels.size / totalParticipants;
    return ratio >= minRatio;
}

// --- Interpret control messages ---

/**
 * Interpret HC control messages from a loaded message set.
 * Returns hidden txids (delete commands), left channels (leave commands),
 * status map, and report list.
 * @param {Array<{message: string, txid?: string, asset_name?: string, channel?: string, authority_asset?: string, time?: string|number}>} messages
 * @returns {{
 *   hiddenTxids: Set<string>,
 *   leftChannels: Set<string>,
 *   leaveMessageKeys: Set<string>,
 *   statusByChannel: Map<string, {value: number, expiryMode: number, expiryValue: number, expiryTs: number, commandTime: number, commandKey: string}>,
 *   reportCommands: Array<{target: string, targetType: number, reason: number, severity: number, durationDays: number, channel: string, timeSec: number, commandKey: string}>
 * }}
 */
export async function interpretControlMessages(messages) {
    const derivedHidden = new Set();
    const leftChannels = new Set();
    const leaveMessageKeys = new Set();
    /** @type {Map<string, {value: number, expiryMode: number, expiryValue: number, expiryTs: number, commandTime: number, commandKey: string}>} */
    const statusByChannel = new Map();
    /** @type {Array<{target: string, targetType: number, reason: number, severity: number, durationDays: number, channel: string, timeSec: number, commandKey: string}>} */
    const reportCommands = [];

    for (const msg of messages) {
        const hex = msg.message;
        if (!hex || !isHcControlFrame(hex)) continue;

        let parsed = null;
        try {
            parsed = await core.invoke("h0xc_control_decode", { hex });
        } catch {
            continue;
        }

        if (!parsed || !parsed.is_control) continue;
        if (Array.isArray(parsed.warnings) && parsed.warnings.length > 0) continue;

        const cmdChannel = canonicalMessageChannel(msg);
        if (!cmdChannel) continue;

        const msgTimeSec = parseTimeSec(msg.time);

        if (parsed.command === "delete" && parsed.txid_prefix && parsed.txid_suffix) {
            const { txid_prefix: prefix, txid_suffix: suffix } = parsed;
            const matches = messages.filter((m) => {
                if (!m.txid) return false;
                if (!matchTxid(m.txid, prefix, suffix)) return false;
                const targetChannel = canonicalMessageChannel(m);
                return targetChannel && targetChannel === cmdChannel;
            });

            if (matches.length === 1) {
                const targetTxid = matches[0].txid;
                if (targetTxid) {
                    derivedHidden.add(targetTxid);
                }
            }
        } else if (parsed.command === "leave") {
            leftChannels.add(cmdChannel);
            leaveMessageKeys.add(controlMessageKey(msg));
        } else if (parsed.command === "status" && parsed.status_value !== undefined) {
            const existing = statusByChannel.get(cmdChannel);
            const expiryTs = statusExpiryTimestamp(parsed, msg.time);
            const nowSec = Math.floor(Date.now() / 1000);
            const cmdKey = controlMessageKey(msg);
            if (!existing || msgTimeSec > existing.commandTime) {
                if (parsed.status_value === 4 || expiryTs > nowSec) {
                    statusByChannel.set(cmdChannel, {
                        value: parsed.status_value,
                        expiryMode: parsed.status_expiry_mode,
                        expiryValue: parsed.status_expiry_value,
                        expiryTs,
                        commandTime: msgTimeSec,
                        commandKey: cmdKey,
                    });
                } else {
                    statusByChannel.delete(cmdChannel);
                }
            }
        } else if (parsed.command === "report" && parsed.report_target_type !== undefined) {
            let target = "";
            if (parsed.report_target_type === 1 && parsed.txid_prefix && parsed.txid_suffix) {
                // Message report - find matching message
                const matches = messages.filter((m) => {
                    if (!m.txid) return false;
                    if (!matchTxid(m.txid, parsed.txid_prefix, parsed.txid_suffix)) return false;
                    const targetChannel = canonicalMessageChannel(m);
                    return targetChannel === cmdChannel;
                });
                if (matches.length === 1 && matches[0].txid) {
                    target = matches[0].txid;
                }
            } else if (parsed.report_target_type === 2 && parsed.txid_prefix && parsed.txid_suffix) {
                // Channel report - the prefix+suffix is the 16-byte fingerprint
                target = parsed.txid_prefix + parsed.txid_suffix;
            }
            if (target) {
                reportCommands.push({
                    target,
                    targetType: parsed.report_target_type,
                    reason: parsed.report_reason,
                    severity: parsed.report_severity,
                    durationDays: parsed.report_duration_days || 30,
                    channel: cmdChannel,
                    timeSec: msgTimeSec,
                    commandKey: controlMessageKey(msg),
                });
            }
        }
    }

    return {
        hiddenTxids: derivedHidden,
        leftChannels,
        leaveMessageKeys,
        statusByChannel,
        reportCommands,
    };
}

/**
 * Parse a message timestamp to seconds since epoch.
 * @param {string|number|undefined|null} raw
 * @returns {number}
 */
function parseTimeSec(raw) {
    if (raw == null || raw === "") return Math.floor(Date.now() / 1000);
    if (typeof raw === "number" || (typeof raw === "string" && /^\d{10,}$/.test(raw.trim()))) {
        const n = typeof raw === "number" ? raw : parseInt(raw, 10);
        if (Number.isNaN(n)) return Math.floor(Date.now() / 1000);
        if (n > 1e12) return Math.floor(n / 1000);
        if (n > 1e9) return n;
        return Math.floor(Date.now() / 1000);
    }
    if (typeof raw === "string") {
        const coreMatch = raw.match(/^(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})$/);
        if (coreMatch) {
            const [, y, mo, d, h, mi, s] = coreMatch;
            const ms = Date.UTC(+y, +mo - 1, +d, +h, +mi, +s);
            return Number.isNaN(ms) ? Math.floor(Date.now() / 1000) : Math.floor(ms / 1000);
        }
        const d = new Date(raw);
        if (!isNaN(d.getTime())) return Math.floor(d.getTime() / 1000);
    }
    return Math.floor(Date.now() / 1000);
}

export function isMessageHidden(msg, hiddenTxids) {
    if (!msg.txid) return false;
    return hiddenTxids.has(msg.txid);
}

export function isControlCommandMessage(msg) {
    const hex = msg.message;
    if (!hex) return false;
    return isHcControlFrame(hex);
}

export function isDecodedLeaveCommandMessage(msg, leaveMessageKeys) {
    return leaveMessageKeys.has(controlMessageKey(msg));
}

// --- Moderation list persistence helpers ---

const LOCAL_HIDDEN_MESSAGES_KEY = "h0xc_localHiddenMessages";
const LOCAL_HIDDEN_CHANNELS_KEY = "h0xc_localHiddenChannels";
const COMMUNITY_HIDDEN_KEY = "h0xc_communityHidden";
const OVERRIDE_HIDDEN_KEY = "h0xc_overrideHidden";
const REPORT_LOG_KEY = "h0xc_reportLog";

function loadJson(key, fallback) {
    try {
        const raw = localStorage.getItem(key);
        if (raw) return JSON.parse(raw);
    } catch { /* corrupt */ }
    return fallback;
}

function saveJson(key, value) {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* quota */ }
}

/** @returns {Array<{txid: string, timeSec: number, reason?: number}>} */
export function getLocalHiddenMessages() {
    return loadJson(LOCAL_HIDDEN_MESSAGES_KEY, []);
}

export function hideMessageLocally(txid, reason) {
    const list = getLocalHiddenMessages();
    if (!list.find((e) => e.txid === txid)) {
        list.push({ txid, timeSec: Math.floor(Date.now() / 1000), reason });
        saveJson(LOCAL_HIDDEN_MESSAGES_KEY, list);
    }
}

export function unhideMessageLocally(txid) {
    const list = getLocalHiddenMessages().filter((e) => e.txid !== txid);
    saveJson(LOCAL_HIDDEN_MESSAGES_KEY, list);
}

/** @returns {Array<{channel: string, timeSec: number, reason?: number, rootName?: string}>} */
export function getLocalHiddenChannels() {
    return loadJson(LOCAL_HIDDEN_CHANNELS_KEY, []);
}

export function hideChannelLocally(channel, reason, rootName) {
    const list = getLocalHiddenChannels();
    const norm = normalizeChannel(channel);
    if (!list.find((e) => normalizeChannel(e.channel) === norm)) {
        list.push({
            channel: norm,
            timeSec: Math.floor(Date.now() / 1000),
            reason,
            rootName: rootName || "",
        });
        saveJson(LOCAL_HIDDEN_CHANNELS_KEY, list);
    }
}

export function unhideChannelLocally(channel) {
    const norm = normalizeChannel(channel);
    const list = getLocalHiddenChannels().filter((e) => normalizeChannel(e.channel) !== norm);
    saveJson(LOCAL_HIDDEN_CHANNELS_KEY, list);
}

/** @returns {Array<{target: string, targetType: number, count: number, channels: string[], maxSeverity: number, expiryTs: number}>} */
export function getCommunityHidden() {
    return loadJson(COMMUNITY_HIDDEN_KEY, []);
}

export function setCommunityHidden(list) {
    saveJson(COMMUNITY_HIDDEN_KEY, list);
}

/** @returns {Set<string>} targets the user has manually un-hidden from community results */
export function getOverrideHidden() {
    return new Set(loadJson(OVERRIDE_HIDDEN_KEY, []));
}

export function addOverrideHidden(target) {
    const list = loadJson(OVERRIDE_HIDDEN_KEY, []);
    if (!list.includes(target)) {
        list.push(target);
        saveJson(OVERRIDE_HIDDEN_KEY, list);
    }
}

export function removeOverrideHidden(target) {
    const list = loadJson(OVERRIDE_HIDDEN_KEY, []).filter((t) => t !== target);
    saveJson(OVERRIDE_HIDDEN_KEY, list);
}

/** @returns {Array<{target: string, targetType: number, reason: number, severity: number, channel: string, timeSec: number}>} */
export function getReportLog() {
    return loadJson(REPORT_LOG_KEY, []);
}

export function addReportLog(entry) {
    const list = getReportLog();
    list.push({ ...entry, timeSec: Math.floor(Date.now() / 1000) });
    saveJson(REPORT_LOG_KEY, list);
}

/**
 * Check if a message is hidden by local or community moderation.
 * @param {{txid?: string}} msg
 * @param {Set<string>} hiddenTxids - from delete commands
 * @returns {{hidden: boolean, reason?: string, details?: string}}
 */
export function isMessageModerationHidden(msg, hiddenTxids) {
    if (!msg.txid) return { hidden: false };

    // Delete-command hidden
    if (hiddenTxids.has(msg.txid)) return { hidden: true, reason: "Delete command" };

    // Local hidden messages
    const localMsgs = getLocalHiddenMessages();
    if (localMsgs.find((e) => e.txid === msg.txid)) {
        return { hidden: true, reason: "Locally hidden" };
    }

    // Community hidden messages
    const overrides = getOverrideHidden();
    if (overrides.has(msg.txid)) return { hidden: false };

    const community = getCommunityHidden();
    const entry = community.find((e) => e.target === msg.txid);
    if (entry && entry.expiryTs > Math.floor(Date.now() / 1000)) {
        return {
            hidden: true,
            reason: "Community report",
            details: `${entry.count} reports, severity ${entry.maxSeverity}`,
        };
    }

    return { hidden: false };
}

/**
 * Check if a channel/user is hidden by local or community moderation.
 * @param {string} channel
 * @param {Set<string>} leftChannels
 * @param {Set<string>} blockedUsers
 * @param {Set<string>} tagBlockedChannels
 * @returns {{hidden: boolean, reason?: string, details?: string}}
 */
export function isChannelModerationHidden(channel, leftChannels, blockedUsers, tagBlockedChannels) {
    const norm = normalizeChannel(channel);
    if (!norm) return { hidden: false };

    if (leftChannels.has(norm)) return { hidden: true, reason: "Left chat" };
    if (blockedUsers.has(norm)) return { hidden: true, reason: "Blocked" };
    if (tagBlockedChannels.has(norm)) return { hidden: true, reason: "Tag blocked" };

    // Local hidden channels
    const localChs = getLocalHiddenChannels();
    if (localChs.find((e) => normalizeChannel(e.channel) === norm)) {
        return { hidden: true, reason: "Locally hidden" };
    }

    // Community hidden channels - check by fingerprint
    const overrides = getOverrideHidden();
    const fp = channelFingerprintHex(channel);
    if (overrides.has(fp)) return { hidden: false };

    const community = getCommunityHidden();
    const entry = community.find((e) => e.target === fp);
    if (entry && entry.expiryTs > Math.floor(Date.now() / 1000)) {
        return {
            hidden: true,
            reason: "Community report",
            details: `${entry.count} reports, severity ${entry.maxSeverity}`,
        };
    }

    return { hidden: false };
}
