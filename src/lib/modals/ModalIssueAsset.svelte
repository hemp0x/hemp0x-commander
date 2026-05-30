<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import IpfsHashField from "../ui/IpfsHashField.svelte";
    import HelpHitbox from "../ui/HelpHitbox.svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let nodeOnline = false;

    // Bindable fields
    export let name = "";
    export let qty = "1";
    export let units = 0;
    export let ipfs = "";
    export let reissuable = true;

    function close() {
        dispatch("close");
    }

    function create() {
        dispatch("create");
    }
</script>

{#if isOpen}
    <div
        class="modal-overlay"
        transition:fade={{ duration: 150 }}
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        role="button"
        tabindex="0"
    >
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
            class="form-panel glass-modal"
            on:click|stopPropagation
            transition:fly={{ y: 20 }}
        >
            <div class="form-header compact">
                <button class="back-btn" on:click={close} title="Close">
                    ← BACK
                </button>
                <span class="form-title">CREATE ROOT ASSET</span>
            </div>

            <div class="form-grid compact-grid">
                <div class="form-group wide">
                    <label for="root-name">ASSET NAME</label>
                    <input
                        id="root-name"
                        type="text"
                        class="glass-input"
                        placeholder="MY_TOKEN"
                        bind:value={name}
                    />
                </div>

                <div class="form-group narrow">
                    <span class="label-text">COST</span>
                    <div class="static-value">0.25 HEMP</div>
                </div>

                <div class="form-group wide">
                    <label for="root-qty">QUANTITY</label>
                    <input
                        id="root-qty"
                        type="number"
                        class="glass-input mono"
                        placeholder="1"
                        bind:value={qty}
                    />
                </div>

                <div class="form-group narrow">
                    <label for="root-units">DECIMALS</label>
                    <input
                        id="root-units"
                        type="number"
                        class="glass-input mono"
                        min="0"
                        max="8"
                        bind:value={units}
                    />
                </div>

                <div class="form-group full-width">
                    <div class="field-label-row">
                        <label for="root-ipfs">IPFS HASH (Optional)</label>
                        <HelpHitbox title="Root Asset Metadata">
                            <p>Root assets are top-level asset names on the chain.</p>
                            <p>Ownership of a root asset controls future sub-assets and metadata/reissue behavior.</p>
                            <p>Metadata should be a published CID or hash reference. Create and publish the package in Content Library first, then select it here.</p>
                        </HelpHitbox>
                    </div>
                    <IpfsHashField id="root-ipfs" bind:value={ipfs} />
                </div>

                <!-- Footer Row: Checkbox Left, Button Right -->
                <div class="form-group full-width action-row">
                    <label class="checkbox-wrap">
                        <input type="checkbox" bind:checked={reissuable} />
                        <span class="checkbox-visual"></span>
                        <span class="checkbox-text">REISSUABLE</span>
                    </label>

                    <button
                        class="neon-btn"
                        on:click={create}
                        disabled={!nodeOnline || !name}
                    >
                        <span class="btn-glow"></span>
                        CREATE ASSET
                    </button>
                </div>
            </div>
        </div>
    </div>
{/if}

<style>
    /* ═══════════════ LOCAL MODAL STYLES ═══════════════ */
    .glass-modal {
        background: rgba(10, 15, 12, 0.95);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        box-shadow:
            0 0 80px rgba(0, 0, 0, 0.8),
            0 0 40px rgba(0, 255, 65, 0.1);
        overflow: hidden;
    }
    .form-panel {
        max-width: 700px;
        margin: 0 auto;
        padding: 2rem;
        width: 100%; /* Fix width */
    }
    .form-header.compact {
        padding: 0.8rem 1.5rem;
    }
    .form-header {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        margin-bottom: 0.75rem;
    }
    .back-btn {
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
        padding: 0.4rem 0.8rem;
        font-size: 0.7rem;
        font-weight: 500;
        letter-spacing: 1px;
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .back-btn:hover {
        color: var(--color-primary);
        border-color: var(--color-primary);
        background: rgba(0, 255, 65, 0.05);
    }
    .form-title {
        color: var(--color-primary);
        font-size: 0.85rem;
        font-weight: 600;
        letter-spacing: 1.5px;
    }
    .form-grid.compact-grid {
        gap: 0.8rem 1.2rem;
        padding: 0 1.5rem 1.5rem;
    }
    .form-grid {
        display: grid;
        grid-template-columns: repeat(12, 1fr);
        gap: 1rem 1.5rem;
    }
    .form-group {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
    }
    .form-group.full-width {
        grid-column: span 12;
    }
    .form-group.wide {
        grid-column: span 8;
    }
    .form-group.narrow {
        grid-column: span 4;
    }
    label,
    .label-text {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1.5px;
        text-transform: uppercase;
    }
    .field-label-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .static-value {
        padding: 0.7rem 1rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid transparent;
        border-radius: 8px;
        color: #888;
        font-size: 0.85rem;
        font-family: var(--font-mono);
    }
    .glass-input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.7rem 1rem;
        font-size: 0.85rem;
        border-radius: 8px;
        outline: none;
        width: 100%;
        transition: all 0.2s;
        backdrop-filter: blur(5px);
    }
    .glass-input:focus {
        border-color: var(--color-primary);
        box-shadow:
            0 0 20px rgba(0, 255, 65, 0.15),
            inset 0 0 20px rgba(0, 255, 65, 0.03);
    }
    .action-row {
        display: flex !important;
        flex-direction: row !important;
        align-items: center;
        justify-content: space-between;
        margin-top: 0.5rem;
        gap: 1rem;
    }
    .checkbox-wrap {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        cursor: pointer;
        padding: 0.5rem 0;
    }
    .checkbox-wrap input {
        display: none;
    }
    .checkbox-visual {
        width: 16px;
        height: 16px;
        border: 2px solid #444;
        border-radius: 4px;
        transition: all 0.15s;
        position: relative;
    }
    .checkbox-wrap input:checked + .checkbox-visual {
        background: var(--color-primary);
        border-color: var(--color-primary);
        box-shadow: 0 0 10px var(--color-primary);
    }
    .checkbox-wrap input:checked + .checkbox-visual::after {
        content: "✓";
        position: absolute;
        top: -1px;
        left: 2px;
        font-size: 11px;
        color: #000;
        font-weight: bold;
    }
    .checkbox-text {
        font-size: 0.65rem;
        color: #888;
        letter-spacing: 1px;
    }
    .checkbox-wrap input:checked ~ .checkbox-text {
        color: #fff;
    }

    /* Buttons */
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
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        overflow: hidden;
    }
    .neon-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow:
            0 0 20px var(--color-primary),
            0 0 30px rgba(0, 255, 65, 0.3);
        transform: translateY(-1px);
    }
    .neon-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .btn-glow {
        position: absolute;
        inset: -50%;
        background: conic-gradient(
            transparent,
            transparent,
            transparent,
            rgba(0, 255, 65, 0.2)
        );
        animation: spin 4s linear infinite;
        opacity: 0;
    }
    .neon-btn:hover .btn-glow {
        opacity: 1;
    }
    @keyframes spin {
        100% {
            transform: rotate(360deg);
        }
    }
</style>
