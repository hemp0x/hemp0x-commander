<script>
    import { fly, fade } from "svelte/transition";
    import "../../components.css";

    export let isOpen = false;
    export let type = "";
    /** @type {Record<string, any> | null} */
    export let payload = {};
    /** @type {Record<string, any> | null} */
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
    $: previewDetails = previewData || {};
    $: payloadEntries = Object.entries(payload || {});
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
                        <span class="row-val">{previewDetails.operation_type || type}</span>
                    </div>
                    <div class="confirm-row">
                        <span class="row-key">ASSET</span>
                        <span class="row-val">{previewDetails.asset_name}</span>
                    </div>
                    {#if previewDetails.qty}
                        <div class="confirm-row">
                            <span class="row-key">QUANTITY</span>
                            <span class="row-val">{previewDetails.qty}</span>
                        </div>
                    {/if}
                    {#if previewDetails.units != null && previewDetails.units !== undefined}
                        <div class="confirm-row">
                            <span class="row-key">DECIMALS</span>
                            <span class="row-val">{previewDetails.units}</span>
                        </div>
                    {/if}
                    {#if previewDetails.reissuable != null && previewDetails.reissuable !== undefined}
                        <div class="confirm-row">
                            <span class="row-key">REISSUABLE</span>
                            <span class="row-val yes">{previewDetails.reissuable ? "YES" : "NO"}</span>
                        </div>
                    {/if}
                    {#if previewDetails.ipfs_hash}
                        <div class="confirm-row">
                            <span class="row-key">IPFS</span>
                            <span class="row-val mono" style="font-size:0.7rem;">{previewDetails.ipfs_hash}</span>
                        </div>
                    {/if}
                    {#if previewDetails.parent_asset}
                        <div class="confirm-row">
                            <span class="row-key">PARENT</span>
                            <span class="row-val">{previewDetails.parent_asset}</span>
                        </div>
                    {/if}
                    {#if previewDetails.tags}
                        <div class="confirm-row">
                            <span class="row-key">TAGS</span>
                            <span class="row-val" style="font-size:0.7rem;">{previewDetails.tags.join(", ")}</span>
                        </div>
                    {/if}
                    {#if previewDetails.operation_type === "distribute_reward"}
                        <div class="confirm-row">
                            <span class="row-key">SNAPSHOT HEIGHT</span>
                            <span class="row-val">{previewDetails.snapshot_height}</span>
                        </div>
                        <div class="confirm-row">
                            <span class="row-key">DISTRIBUTION ASSET</span>
                            <span class="row-val">{previewDetails.distribution_asset}</span>
                        </div>
                        <div class="confirm-row">
                            <span class="row-key">GROSS AMOUNT</span>
                            <span class="row-val">{previewDetails.gross_amount}</span>
                        </div>
                        <div class="confirm-row">
                            <span class="row-key">RECIPIENTS</span>
                            <span class="row-val">{previewDetails.estimated_recipient_count ?? "unknown"}</span>
                        </div>
                        {#if previewDetails.exception_addresses}
                            <div class="confirm-row">
                                <span class="row-key">EXCLUSIONS</span>
                                <span class="row-val mono" style="font-size:0.65rem;">{previewDetails.exception_addresses}</span>
                            </div>
                        {/if}
                    {/if}

                    {#if previewDetails.is_irreversible}
                        <div class="warning-box irreversible">
                            <span class="warning-icon">&#9888;</span>
                            <span>This operation has IRREVERSIBLE effects that cannot be undone.</span>
                        </div>
                    {/if}

                    {#each previewDetails.warnings || [] as warn}
                        <div class="warning-box">
                            <span class="warning-icon">&#9432;</span>
                            <span>{warn}</span>
                        </div>
                    {/each}

                    <div class="summary-box">
                        <span class="summary-text">{previewDetails.summary}</span>
                    </div>
                {:else}
                    {#each payloadEntries as [k, v]}
                        <div class="confirm-row">
                            <span class="row-key">{k}</span>
                            <span class="row-val">{v}</span>
                        </div>
                    {/each}
                {/if}
            </div>

            <div class="confirm-footer">
                <button class="cyber-btn ghost" on:click={close} disabled={isBroadcasting}>
                    CANCEL
                </button>
                <button
                    class="cyber-btn"
                    class:btn-danger={isPreview && previewDetails.is_irreversible}
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
        max-width: min(420px, 92vw);
        max-height: min(44rem, calc(100dvh - 2rem));
        display: flex;
        flex-direction: column;
    }
    .glass-modal {
        background: rgba(10, 15, 12, 0.95);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
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
        padding: 1rem 1.25rem;
        max-height: min(44rem, calc(100dvh - 10rem));
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
        gap: 0.75rem;
        padding: 1rem 1.25rem;
        background: rgba(0, 0, 0, 0.3);
    }
    .confirm-footer button {
        flex: 1;
    }
    @media (max-width: 520px) {
        .confirm-header {
            padding: 0.8rem 1rem;
            font-size: 0.8rem;
        }
        .confirm-body {
            padding: 0.8rem 1rem;
        }
        .confirm-footer {
            padding: 0.8rem 1rem;
            flex-direction: column;
            gap: 0.5rem;
        }
        .confirm-row {
            flex-direction: column;
            align-items: flex-start;
            gap: 0.2rem;
            padding: 0.5rem 0;
        }
    }
</style>
