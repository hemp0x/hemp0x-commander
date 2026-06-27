// Shared message-index helpers for H0XC chat and System > Repair.
//
// Both UI locations use the same Core rescanmessages path and the same
// getmessaginginfo-driven status model so logic is not duplicated.

import { core } from "@tauri-apps/api";
import { systemHubSection } from "./systemHub.js";

/**
 * Fetch the current message-index state from Core via get_messaginginfo.
 * Returns an object with:
 *  - mi: the message_index block (null when absent/unsupported or RPCs down)
 *  - messagingAvailable: true when messaging RPCs responded and are active
 *    (distinguishes "messaging works / no message_index" from "RPCs down")
 * Returns null if the RPC call itself fails (Core unreachable/wrong daemon).
 * @returns {Promise<{ mi: { enabled: boolean, mode: string, synced: boolean, synced_height: number, best_height: number, needs_rescan: boolean, rescan_in_progress: boolean, rescan_start_height: number, rescan_stop_height: number, rescan_current_height: number, rescan_scanned_blocks: number, rescan_messages_found: number, rescan_messages_added: number, rescan_last_error: string|null, pruned: boolean, pruned_limitation: string|null } | null, messagingAvailable: boolean } | null>}
 */
export async function fetchMessageIndexState() {
    let info;
    try {
        info = await core.invoke("get_messaging_info");
    } catch {
        return null;
    }
    if (!info) return null;
    const messagingAvailable = !!(info.enabled || info.messaging_active);
    if (!messagingAvailable) {
        return { mi: null, messagingAvailable: false };
    }
    return { mi: info.message_index || null, messagingAvailable: true };
}

/**
 * Derive a compact, user-facing status label from message-index state.
 * Returns one of: unavailable, unsupported, disabled, needs-recovery,
 * catching-up, recovering, synced, pruned.
 * @param {any} mi  message_index object from get_messaginginfo (or null)
 * @param {boolean} messagingUnavailable  true when Core messaging RPCs are down
 * @param {boolean} [unsupported]  true when messaging works but message_index field is absent
 * @returns {"unavailable"|"unsupported"|"disabled"|"needs-recovery"|"catching-up"|"recovering"|"synced"|"pruned"}
 */
export function messageIndexStatusLabel(mi, messagingUnavailable, unsupported) {
    if (messagingUnavailable && !mi) return "unavailable";
    if (unsupported && !mi) return "unsupported";
    if (!mi) return "unavailable";
    if (mi.pruned) return "pruned";
    if (!mi.enabled) return "disabled";
    if (mi.rescan_in_progress) return "recovering";
    if (isNearTipMessageIndexLag(mi)) return "catching-up";
    if (mi.needs_rescan) return "needs-recovery";
    if (mi.synced) return "synced";
    return "needs-recovery";
}

export function isNearTipMessageIndexLag(mi) {
    if (!mi || !mi.enabled || mi.synced) return false;
    const synced = Number(mi.synced_height || 0);
    const best = Number(mi.best_height || 0);
    if (synced <= 0 || best <= synced) return false;
    return best - synced <= 100;
}

/**
 * Human-readable label + hint + action for a message-index status.
 * Action is one of: "", "enable", "recover".
 * No em dashes.
 * @param {any} mi  message_index object (or null)
 * @param {boolean} messagingUnavailable  true when Core messaging RPCs are down
 * @param {boolean} [unsupported]  true when messaging works but message_index field is absent
 * @returns {{ label: string, hint: string, action: ""|"enable"|"recover" }}
 */
export function messageIndexStatusText(mi, messagingUnavailable, unsupported) {
    const label = messageIndexStatusLabel(mi, messagingUnavailable, unsupported);
    switch (label) {
        case "unsupported":
            return {
                label: "MESSAGE INDEX UNSUPPORTED",
                hint: "This Core build does not expose the message_index block. Update Core to a version that supports getmessaginginfo.message_index for full H0XC history.",
                action: "enable",
            };
        case "disabled":
            return {
                label: "MESSAGE INDEX OFF",
                hint: "Full message indexing is off. H0XC chat can only see subscribed-channel messages. Enable H0XC Message Index in System Config for full community chat.",
                action: "enable",
            };
        case "needs-recovery": {
            const hint = mi && mi.synced_height > 0
                ? `Indexed to height ${mi.synced_height} of ${mi.best_height || "?"}. Recover H0XC history to backfill.`
                : "Message index enabled but history needs recovery. Run Recover H0XC History once.";
            return { label: "HISTORY NEEDS RECOVERY", hint, action: "recover" };
        }
        case "recovering": {
            const cur = mi?.rescan_current_height ?? 0;
            const stop = mi?.rescan_stop_height ?? 0;
            const found = mi?.rescan_messages_found ?? 0;
            const err = mi?.rescan_last_error ?? "";
            const progress = stop > 0 ? ` ${cur}/${stop}` : "";
            const hint = err
                ? `Backfilling message index${progress}. Last error: ${err}`
                : `Backfilling message index from blocks${progress}.${found ? ` ${found} messages found.` : ""} Commander stays responsive.`;
            return { label: `RECOVERING${progress}`, hint, action: "" };
        }
        case "catching-up": {
            const synced = Number(mi?.synced_height || 0);
            const best = Number(mi?.best_height || 0);
            const gap = best > synced ? best - synced : 0;
            return {
                label: "MESSAGE INDEX CATCHING UP",
                hint: gap > 0
                    ? `Message index is ${gap} block${gap === 1 ? "" : "s"} behind the chain tip. Refresh Messages will catch up the missing window.`
                    : "Message index is catching up. Refresh Messages will check again.",
                action: "",
            };
        }
        case "synced":
            return {
                label: "MESSAGE INDEX SYNCED",
                hint: "Full message index is up to date. H0XC chat sees public messages without manual subscriptions.",
                action: "",
            };
        case "pruned":
            return {
                label: "PRUNED NODE",
                hint: mi?.pruned_limitation || "Historical message recovery is limited on pruned nodes because older blocks may not be available locally.",
                action: "",
            };
        case "unavailable":
        default:
            return {
                label: "MESSAGE RPC UNAVAILABLE",
                hint: "Core message RPCs are unavailable. The daemon may be wrong or still warming up. Start a messaging-capable Hemp0x Core build, then refresh.",
                action: "enable",
            };
    }
}

/**
 * Start a message-history recovery (rescanmessages). Does not block the UI:
 * the backend runs the long Core call on a blocking task. Returns a result
 * object. If a rescan is already in progress, the caller should not start
 * another; this helper does not check that itself (the UI owns the guard so
 * it can show existing progress instead).
 *
 * @param {{ startHeight?: number|null, stopHeight?: number|null, channel?: string|null }} [options]
 * @returns {Promise<{ success: boolean, raw: string, error: string }>}
 */
export async function startMessageRescan(options = {}) {
    return await core.invoke("rescan_messages", {
        startHeight: options.startHeight ?? null,
        stopHeight: options.stopHeight ?? null,
        channel: options.channel ?? null,
    });
}

/**
 * Open Tools > System > Config via the existing event navigation pattern.
 * Safe to call from anywhere (modal, chat, repair page).
 */
export function openSystemConfig() {
    try {
        systemHubSection.set("config");
    } catch {
        // store unavailable in some contexts; the event still routes the view.
    }
    window.dispatchEvent(
        new CustomEvent("commander-open-tools-system", { detail: { section: "config" } }),
    );
}
