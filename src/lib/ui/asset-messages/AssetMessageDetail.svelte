<script>
    import { createEventDispatcher } from "svelte";
    import IpfsReference from "../IpfsReference.svelte";

    /**
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     *   txid?: string;
     *   channel?: string;
     *   authority_asset?: string;
     *   authority_address?: string;
     *   block_hash?: string;
     *   sender_address?: string;
     * }} AssetMessage
     * @typedef {{
     *   is_short_message?: boolean;
     *   text?: string;
     *   warnings?: string[];
     * }} ShortMessageDecodeResult
     */

    /** @type {AssetMessage} */
    export let message;
    /** @type {ShortMessageDecodeResult | undefined} */
    export let decoded = undefined;
    export let isPinned = false;
    export let status = "";

    const dispatch = createEventDispatcher();

    function close() {
        dispatch("close");
    }

    /** @param {string|number|null|undefined} isoLike */
    function formatExpireTime(isoLike) {
        if (!isoLike) return "";
        let d = new Date(isoLike);
        if (isNaN(d.getTime())) {
            const n = Number(isoLike);
            if (!Number.isNaN(n) && n > 1000000000) {
                d = new Date(n * 1000);
            }
        }
        if (isNaN(d.getTime())) return isoLike;
        const now = Date.now();
        const diff = d.getTime() - now;
        if (diff <= 0) return "Expired";
        const mins = Math.floor(diff / 60000);
        if (mins < 60) return `${mins}m`;
        const hrs = Math.floor(mins / 60);
        if (hrs < 24) return `${hrs}h ${mins % 60}m`;
        const days = Math.floor(hrs / 24);
        return `${days}d ${hrs % 24}h`;
    }

    function togglePin() {
        dispatch("togglePin");
    }

    function markUnread() {
        dispatch("markUnread");
    }
</script>

<div
    class="message-detail-overlay"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    on:click={close}
    on:keydown={(e) => e.key === 'Escape' && close()}
>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
    <div class="message-detail-panel" role="document" on:click|stopPropagation on:keydown|stopPropagation>
        <div class="message-detail-header">
            <div class="message-detail-title">MESSAGE DETAIL</div>
            <button class="detail-close-btn" on:click={close}>×</button>
        </div>
        <div class="message-detail-body">
            <div class="detail-row">
                <span class="detail-label">Channel</span>
                <span class="detail-value">{message.asset_name}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Time</span>
                <span class="detail-value">{message.time}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Block</span>
                <span class="detail-value">#{message.block_height}</span>
            </div>
            <div class="detail-row">
                <span class="detail-label">Status</span>
                <span class="detail-value" class:unread={status === 'UNREAD'}>{status}</span>
            </div>
            {#if message.expire_time}
                <div class="detail-row">
                    <span class="detail-label">Expires</span>
                    <span class="detail-value">{message.expire_time} ({formatExpireTime(message.expire_time)})</span>
                </div>
            {/if}

            <div class="detail-divider"></div>

            {#if decoded?.is_short_message}
                <div class="detail-row">
                    <span class="detail-label">Type</span>
                    <span class="detail-value short-badge">Short Message</span>
                </div>
                <div class="detail-row tall">
                    <span class="detail-label">Decoded</span>
                    <span class="detail-value decoded-text">{decoded.text}</span>
                </div>
                {#if decoded.warnings && decoded.warnings.length > 0}
                    <div class="detail-warnings">
                        {#each decoded.warnings as w}
                            <div class="detail-warning">⚠ {w}</div>
                        {/each}
                    </div>
                {/if}
                <div class="detail-row tall">
                    <span class="detail-label">Raw Hex</span>
                    <span class="detail-value mono hex-text">{message.message}</span>
                </div>
            {:else}
                <div class="detail-row">
                    <span class="detail-label">Type</span>
                    <span class="detail-value">CID / Hash Reference</span>
                </div>
                <div class="detail-row tall">
                    <span class="detail-label">Payload</span>
                    <IpfsReference hash={message.message} compact={false} />
                </div>
                {#if decoded && !decoded.is_short_message}
                    <div class="detail-note">
                        Not a recognized short-message frame. If this is a custom-table message, select the matching table pack in the inbox toolbar to decode it.
                    </div>
                {/if}
            {/if}

            <div class="detail-divider"></div>

            <div class="detail-actions-row">
                <button class="action-btn" on:click={togglePin}>
                    {isPinned ? "★ Unpin" : "☆ Pin"}
                </button>
                <button class="action-btn" on:click={markUnread}>
                    Mark Unread
                </button>
                <button class="action-btn" on:click={close}>
                    Close
                </button>
            </div>
        </div>
    </div>
</div>

<style>
    .message-detail-overlay {
        position: fixed;
        inset: 0;
        z-index: 500;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.75);
        backdrop-filter: blur(2px);
    }
    .message-detail-panel {
        width: min(32rem, 92vw);
        max-height: 85vh;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        background: linear-gradient(180deg, #080b09 0%, #0f1410 100%);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8), 0 0 36px rgba(0, 255, 65, 0.12);
    }
    .message-detail-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.6rem 0.8rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.05);
    }
    .message-detail-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.5px;
    }
    .detail-close-btn {
        background: transparent;
        border: none;
        color: #777;
        font-size: 1.4rem;
        cursor: pointer;
        line-height: 1;
        padding: 0.15rem 0.35rem;
        transition: color 0.15s;
    }
    .detail-close-btn:hover {
        color: #fff;
    }
    .message-detail-body {
        padding: 0.7rem 0.9rem;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .message-detail-body::-webkit-scrollbar {
        width: 6px;
    }
    .message-detail-body::-webkit-scrollbar-track {
        background: transparent;
    }
    .message-detail-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 3px;
    }
    .detail-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.5rem;
    }
    .detail-row.tall {
        align-items: flex-start;
        flex-direction: column;
        gap: 0.2rem;
    }
    .detail-label {
        font-size: 0.55rem;
        color: #777;
        letter-spacing: 0.5px;
        flex-shrink: 0;
    }
    .detail-value {
        font-size: 0.65rem;
        color: #ccc;
        text-align: right;
        word-break: break-word;
    }
    .detail-value.unread {
        color: var(--color-primary);
    }
    .detail-value.short-badge {
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        padding: 0.1rem 0.35rem;
        font-size: 0.55rem;
        font-weight: 600;
    }
    .detail-value.decoded-text {
        font-size: 0.75rem;
        color: #e0e0e0;
        line-height: 1.4;
        text-align: left;
    }
    .detail-value.hex-text {
        font-size: 0.6rem;
        color: #888;
        background: rgba(0, 0, 0, 0.3);
        padding: 0.35rem 0.45rem;
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.06);
        word-break: break-all;
        text-align: left;
        width: 100%;
    }
    .detail-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.25rem 0;
    }
    .detail-warnings {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .detail-warning {
        font-size: 0.6rem;
        color: #ffaa00;
    }
    .detail-note {
        font-size: 0.6rem;
        color: #888;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px;
        padding: 0.4rem 0.55rem;
        line-height: 1.4;
    }
    .detail-actions-row {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
        margin-top: 0.25rem;
    }
    .action-btn {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.35rem 0.65rem;
        color: #aaa;
        font-size: 0.65rem;
        font-weight: 600;
        letter-spacing: 1px;
        cursor: pointer;
        transition: all 0.15s;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.4rem;
        white-space: nowrap;
    }
    .action-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
</style>
