<script>
    import { fly, fade } from "svelte/transition";

    export let isOpen = false;
    export let type = "";
    export let payload = {};
    export let previewData = null;
    export let isBroadcasting = false;

    import { createEventDispatcher } from "svelte";
    const dispatch = createEventDispatcher();

    function close() {
        dispatch("close");
    }

    function confirm() {
        dispatch("confirm");
    }

    $: isPreview = previewData && previewData.validated !== undefined;
</script>

{#if isOpen}
    <div class="modal-overlay" transition:fade={{ duration: 100 }}>
        <div class="confirm-modal glass-modal" transition:fly={{ y: 15 }}>
            <div class="confirm-header">
                {isPreview ? "REVIEW " + type : "CONFIRM " + type}
            </div>

            <div class="confirm-body">
                {#if isPreview}
                    <div class="confirm-row">
                        <span class="row-key">OPERATION</span>
                        <span class="row-val">{previewData.operation_type || type}</span>
                    </div>
                    <div class="confirm-row">
                        <span class="row-key">ASSET</span>
                        <span class="row-val">{previewData.asset_name}</span>
                    </div>
                    {#if previewData.qty}
                        <div class="confirm-row">
                            <span class="row-key">QUANTITY</span>
                            <span class="row-val">{previewData.qty}</span>
                        </div>
                    {/if}
                    {#if previewData.units != null && previewData.units !== undefined}
                        <div class="confirm-row">
                            <span class="row-key">DECIMALS</span>
                            <span class="row-val">{previewData.units}</span>
                        </div>
                    {/if}
                    {#if previewData.reissuable != null && previewData.reissuable !== undefined}
                        <div class="confirm-row">
                            <span class="row-key">REISSUABLE</span>
                            <span class="row-val yes">{previewData.reissuable ? "YES" : "NO"}</span>
                        </div>
                    {/if}
                    {#if previewData.ipfs_hash}
                        <div class="confirm-row">
                            <span class="row-key">IPFS</span>
                            <span class="row-val mono" style="font-size:0.7rem;">{previewData.ipfs_hash}</span>
                        </div>
                    {/if}
                    {#if previewData.parent_asset}
                        <div class="confirm-row">
                            <span class="row-key">PARENT</span>
                            <span class="row-val">{previewData.parent_asset}</span>
                        </div>
                    {/if}
                    {#if previewData.tags}
                        <div class="confirm-row">
                            <span class="row-key">TAGS</span>
                            <span class="row-val" style="font-size:0.7rem;">{previewData.tags.join(", ")}</span>
                        </div>
                    {/if}

                    {#if previewData.is_irreversible}
                        <div class="warning-box irreversible">
                            <span class="warning-icon">&#9888;</span>
                            <span>This operation has IRREVERSIBLE effects that cannot be undone.</span>
                        </div>
                    {/if}

                    {#each previewData.warnings as warn}
                        <div class="warning-box">
                            <span class="warning-icon">&#9432;</span>
                            <span>{warn}</span>
                        </div>
                    {/each}

                    <div class="summary-box">
                        <span class="summary-text">{previewData.summary}</span>
                    </div>
                {:else}
                    {#each Object.entries(payload) as [k, v]}
                        <div class="confirm-row">
                            <span class="row-key">{k}</span>
                            <span class="row-val">{v}</span>
                        </div>
                    {/each}
                {/if}
            </div>

            <div class="confirm-footer">
                <button class="ghost-btn" on:click={close} disabled={isBroadcasting}>
                    CANCEL
                </button>
                <button
                    class="neon-btn sm"
                    class:danger={isPreview && previewData?.is_irreversible}
                    on:click={confirm}
                    disabled={isBroadcasting}
                >
                    {isBroadcasting
                        ? "BROADCASTING..."
                        : isPreview
                          ? "BROADCAST"
                          : "CONFIRM"}
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .confirm-modal {
        width: 100%;
        max-width: 420px;
    }
    .glass-modal {
        background: rgba(10, 15, 12, 0.95);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 16px;
        box-shadow:
            0 0 80px rgba(0, 0, 0, 0.8),
            0 0 40px rgba(0, 255, 65, 0.1);
        overflow: hidden;
    }
    .confirm-header {
        padding: 1rem 1.5rem;
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
        border-bottom: 1px solid rgba(0, 255, 65, 0.2);
        background: rgba(0, 255, 65, 0.05);
    }
    .confirm-body {
        padding: 1.5rem;
        max-height: 60vh;
        overflow-y: auto;
    }
    .confirm-row {
        display: flex;
        justify-content: space-between;
        padding: 0.6rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        font-size: 0.8rem;
    }
    .confirm-row:last-child {
        border-bottom: none;
    }
    .row-key {
        color: #666;
        font-family: var(--font-mono);
    }
    .row-val {
        color: #fff;
        font-weight: 600;
    }
    .row-val.yes {
        color: var(--color-primary);
    }
    .row-val.mono {
        font-family: var(--font-mono);
    }
    .warning-box {
        display: flex;
        align-items: flex-start;
        gap: 0.5rem;
        background: rgba(255, 200, 0, 0.1);
        border: 1px solid rgba(255, 200, 0, 0.2);
        border-radius: 6px;
        padding: 0.6rem 0.8rem;
        margin-top: 0.5rem;
        color: #ffcc00;
        font-size: 0.75rem;
    }
    .warning-box.irreversible {
        background: rgba(255, 68, 68, 0.1);
        border-color: rgba(255, 68, 68, 0.3);
        color: #ff6666;
    }
    .warning-icon {
        flex-shrink: 0;
        font-size: 0.9rem;
    }
    .summary-box {
        background: rgba(0, 255, 65, 0.05);
        border-left: 3px solid var(--color-primary);
        border-radius: 0 4px 4px 0;
        padding: 0.8rem;
        margin-top: 0.8rem;
    }
    .summary-text {
        color: #ccc;
        font-size: 0.8rem;
        line-height: 1.4;
    }
    .confirm-footer {
        display: flex;
        gap: 1rem;
        padding: 1.5rem;
        background: rgba(0, 0, 0, 0.3);
    }
    .confirm-footer button {
        flex: 1;
    }
    .neon-btn {
        position: relative;
        background: linear-gradient(
            180deg,
            rgba(0, 255, 65, 0.15) 0%,
            rgba(0, 255, 65, 0.05) 100%
        );
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.8rem 2rem;
        font-size: 0.75rem;
        font-weight: 700;
        letter-spacing: 2px;
        border-radius: 10px;
        cursor: pointer;
        transition: all 0.2s;
        overflow: hidden;
    }
    .neon-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow:
            0 0 30px var(--color-primary),
            0 0 60px rgba(0, 255, 65, 0.3);
        transform: translateY(-1px);
    }
    .neon-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .neon-btn.sm {
        padding: 0.6rem 1.5rem;
    }
    .neon-btn.danger {
        border-color: #ff6666;
        color: #ff6666;
        background: linear-gradient(
            180deg,
            rgba(255, 68, 68, 0.15) 0%,
            rgba(255, 68, 68, 0.05) 100%
        );
    }
    .ghost-btn {
        background: transparent;
        border: 1px solid #444;
        color: #888;
        padding: 0.6rem 1.5rem;
        font-size: 0.7rem;
        letter-spacing: 1px;
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .ghost-btn:hover:not(:disabled) {
        border-color: #888;
        color: #fff;
    }
    .ghost-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
</style>
