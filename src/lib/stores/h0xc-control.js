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

export function isHcControlFrame(hex) {
    if (!hex || hex.length < 4) return false;
    return hex.slice(0, 4).toLowerCase() === HC_MAGIC_HEX;
}

export async function buildDeleteCommandHex(txid) {
    return await core.invoke("h0xc_control_encode_delete", { txid });
}

export async function interpretControlMessages(messages) {
    const derivedHidden = new Set();

    for (const msg of messages) {
        const hex = msg.message;
        if (!hex || !isHcControlFrame(hex)) continue;

        let parsed = null;
        try {
            const result = await core.invoke("h0xc_control_decode", { hex });
            if (result.is_control && result.command === "delete" && result.txid_prefix && result.txid_suffix) {
                parsed = { prefix: result.txid_prefix, suffix: result.txid_suffix };
            }
        } catch {
            continue;
        }

        if (!parsed) continue;

        const cmdChannel = canonicalMessageChannel(msg);
        if (!cmdChannel) continue;

        const { prefix, suffix } = parsed;
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
    }

    return derivedHidden;
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
