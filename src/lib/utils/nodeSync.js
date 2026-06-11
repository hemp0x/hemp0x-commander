import { core } from "@tauri-apps/api";

/**
 * Verify the local node is caught up enough to broadcast user-facing messages.
 * This mirrors Commander's dashboard sync rules so message sends do not proceed
 * while Core is still catching up or serving stale chain state.
 */
export async function ensureNodeSyncedForBroadcast() {
    const result = await core.invoke("rpc_get_blockchain_info");
    if (result?.success === false) {
        throw new Error(result.error || "Unable to verify node sync status.");
    }

    const data = result?.data ?? result;
    const blocks = Number(data?.blocks ?? 0);
    const headers = Number(data?.headers ?? 0);
    const progress = Number(data?.verificationprogress ?? 0);
    const initialBlockDownload = Boolean(data?.initialblockdownload);
    const medianTime = Number(data?.mediantime ?? 0);
    const staleTip = medianTime > 0 && (Date.now() / 1000 - medianTime) >= 5400;

    const synced = headers > 0
        && blocks >= headers
        && progress >= 0.999
        && !initialBlockDownload
        && !staleTip;

    if (!synced) {
        const heightText = headers > 0 ? `${blocks}/${headers}` : `${blocks}`;
        throw new Error(`Node is not fully synced yet (${heightText} blocks). Wait for Core Next to finish syncing before sending messages.`);
    }
}
