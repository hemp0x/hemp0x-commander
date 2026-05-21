<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let nodeOnline = false;

    // Props
    export let parentName = "";

    // Bindable fields
    export let tag = "";
    export let ipfs = "";

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
                <span class="form-title">MINT UNIQUE / NFT</span>
            </div>
            <div class="form-grid compact-grid">
                <div class="form-group wide">
                    <span class="label-text">PARENT ASSET</span>
                    <div class="static-value">{parentName}</div>
                </div>

                <div class="form-group narrow">
                    <span class="label-text">COST</span>
                    <div class="static-value">0.01 HEMP</div>
                </div>

                <div class="form-group narrow">
                    <label for="nft-tag">UNIQUE TAG</label>
                    <input
                        id="nft-tag"
                        type="text"
                        class="glass-input mono"
                        placeholder="tag_name"
                        bind:value={tag}
                    />
                </div>

                <div class="form-group wide">
                    <label for="nft-ipfs">IPFS HASH (Optional)</label>
                    <input
                        id="nft-ipfs"
                        type="text"
                        class="glass-input mono"
                        placeholder="Qm..."
                        bind:value={ipfs}
                    />
                </div>

                <!-- NFT Footer: Button Right aligned -->
                <div
                    class="form-group full-width action-row"
                    style="justify-content: flex-end;"
                >
                    <button
                        class="neon-btn"
                        on:click={create}
                        disabled={!nodeOnline || !tag.trim()}
                    >
                        <span class="btn-glow"></span>
                        MINT NFT
                    </button>
                </div>
            </div>
        </div>
    </div>
{/if}

<style>
    /* Local Style Copy */
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
        width: 100%;
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
    .label-text,
    label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1.5px;
        text-transform: uppercase;
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
