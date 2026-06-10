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

/**
 * Interpret HC control messages from a loaded message set.
 * Returns hidden txids (delete commands) and left channels (leave commands).
 * Both are derived from the currently loaded messages only.
 * @param {Array<{message: string, txid?: string, asset_name?: string, channel?: string, authority_asset?: string}>} messages
 * @returns {{hiddenTxids: Set<string>, leftChannels: Set<string>, leaveMessageKeys: Set<string>}}
 */
export async function interpretControlMessages(messages) {
    const derivedHidden = new Set();
    const leftChannels = new Set();
    const leaveMessageKeys = new Set();

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
        }
    }

    return { hiddenTxids: derivedHidden, leftChannels, leaveMessageKeys };
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
