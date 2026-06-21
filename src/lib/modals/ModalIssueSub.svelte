<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import IpfsHashField from "../ui/IpfsHashField.svelte";
    import HelpHitbox from "../ui/HelpHitbox.svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let nodeOnline = false;
    export let inline = false;
    export let showBack = false;

    // Props
    export let parentName = "";

    // Bindable fields
    export let name = "";
    export let qty = "1";
    export let units = 0;
    export let ipfs = "";
    export let reissuable = true;

    function close() {
        dispatch("close");
    }

    function goBack() {
        dispatch("back");
    }

    function create() {
        dispatch("create");
    }
</script>

{#snippet panelContent()}
    <div class="modal-header">
        {#if showBack}
            <button class="back-btn" on:click={goBack} title="Back to Asset">←</button>
        {/if}
        <h3>Create Sub-Asset</h3>
        <button class="close-btn" on:click={close}>&times;</button>
    </div>

    <div class="modal-body">
        <div class="panel-body">
            <div class="panel-title-row">
                <h4>Issue Sub-Asset under {parentName}</h4>
                <HelpHitbox title="Sub-Assets">
                    <p>Sub-assets are child tokens under a root asset, separated by a forward slash.</p>
                    <p>Example: <code>ROOT/CHILD</code></p>
                    <p>You must own the parent asset to create sub-assets under it.</p>
                </HelpHitbox>
            </div>

            <div class="field-row">
                <div class="field-group flex-grow">
                    <span class="field-label">Parent Asset</span>
                    <div class="read-only-field">{parentName}</div>
                </div>
                <div class="field-group narrow-inline">
                    <span class="field-label">Cost</span>
                    <div class="read-only-field">0.05 HEMP</div>
                </div>
            </div>

            <div class="field-group">
                <label for="sub-name">Sub-Asset Name</label>
                <input id="sub-name" type="text" class="cyber-input mono" placeholder="CHILD_NAME" bind:value={name} />
                <div class="input-hint">Will create: {parentName}/{name || "..."}</div>
            </div>

            <div class="field-row">
                <div class="field-group flex-grow">
                    <label for="sub-qty">Quantity</label>
                    <input id="sub-qty" type="number" class="cyber-input mono" placeholder="1" bind:value={qty} />
                </div>
                <div class="field-group narrow-inline">
                    <label for="sub-units">Decimals</label>
                    <input id="sub-units" type="number" class="cyber-input mono" min="0" max="8" bind:value={units} />
                </div>
            </div>

            <div class="field-group">
                <label for="sub-ipfs">Metadata CID / Hash (Optional)</label>
                <IpfsHashField id="sub-ipfs" bind:value={ipfs} />
            </div>

            <div class="panel-actions align-center">
                <label class="confirm-check">
                    <input type="checkbox" bind:checked={reissuable} />
                    <span class="checkbox-visual"></span>
                    <span class="check-label">Reissuable</span>
                </label>
                <button class="cyber-btn" on:click={create} disabled={!nodeOnline || !name.trim()}>
                    CREATE SUB-ASSET
                </button>
            </div>
        </div>
    </div>
{/snippet}

{#if isOpen}
    {#if inline}
        <div class="sub-panel" in:fade={{ duration: 150 }}>
            {@render panelContent()}
        </div>
    {:else}
        <div
            class="modal-backdrop"
            role="button"
            tabindex="0"
            on:click={close}
            on:keydown={(e) => e.key === "Escape" && close()}
            transition:fade={{ duration: 200 }}
        >
            <div
                class="modal glass-panel"
                role="dialog"
                aria-modal="true"
                tabindex="-1"
                on:click|stopPropagation
                on:keydown={() => {}}
                transition:fly={{ y: 20, duration: 200 }}
            >
                {@render panelContent()}
            </div>
        </div>
    {/if}
{/if}

<style>
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.85);
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0.75rem;
        z-index: 200000;
        backdrop-filter: blur(5px);
        box-sizing: border-box;
    }
    .modal {
        width: min(560px, 92vw);
        max-width: 92vw;
        max-height: min(44rem, calc(100dvh - 2rem));
        border: 1px solid rgba(0, 255, 65, 0.2);
        box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
        border-radius: 8px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        background: rgba(2, 4, 3, 0.98);
    }
    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 1rem 0.65rem;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        font-size: 0.9rem;
        letter-spacing: 1px;
    }
    .close-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
        margin: -0.2rem -0.4rem -0.35rem 0;
    }
    .close-btn:hover { color: #fff; }
    .back-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.2rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
        margin: -0.2rem 0 -0.35rem -0.4rem;
    }
    .back-btn:hover {
        color: var(--color-primary);
    }

    .modal-body {
        padding: 0.6rem 0.9rem;
        overflow-y: auto;
        overflow-x: hidden;
        flex: 1 1 0%;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .modal-body::-webkit-scrollbar {
        width: 8px;
    }
    .modal-body::-webkit-scrollbar-track {
        background: transparent;
    }
    .modal-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 4px;
    }
    .modal-body::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.55);
    }

    .panel-body {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        padding-bottom: 1.2rem;
    }
    .panel-title-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.5rem;
    }
    .panel-title-row h4 {
        margin: 0;
        color: var(--color-primary);
        font-size: 0.85rem;
        letter-spacing: 1px;
    }

    .field-row {
        display: flex;
        align-items: flex-end;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .field-group.narrow-inline {
        max-width: 140px;
    }
    .field-group.flex-grow {
        flex: 1;
        min-width: 0;
    }

    label,
    .field-label {
        color: #888;
        font-size: 0.65rem;
        letter-spacing: 0.5px;
        display: block;
        margin-bottom: 0.15rem;
    }

    .cyber-input {
        width: 100%;
        padding: 0.45rem 0.6rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        box-sizing: border-box;
        outline: none;
        transition: all 0.2s;
    }
    .cyber-input:focus {
        border-color: var(--color-primary);
    }

    .read-only-field {
        padding: 0.45rem 0.6rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        color: #888;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .input-hint {
        font-size: 0.6rem;
        color: #555;
        margin-top: 0.1rem;
    }

    .confirm-check {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: #ddd;
        font-size: 0.8rem;
        cursor: pointer;
    }
    .confirm-check input {
        display: none;
    }
    .checkbox-visual {
        width: 16px;
        height: 16px;
        border: 2px solid #444;
        border-radius: 4px;
        transition: all 0.15s;
        position: relative;
        flex-shrink: 0;
    }
    .confirm-check input:checked + .checkbox-visual {
        background: var(--color-primary);
        border-color: var(--color-primary);
    }
    .confirm-check input:checked + .checkbox-visual::after {
        content: "✓";
        position: absolute;
        top: -1px;
        left: 2px;
        font-size: 11px;
        color: #000;
        font-weight: bold;
    }
    .check-label {
        font-size: 0.65rem;
        color: #888;
        letter-spacing: 0.5px;
    }
    .confirm-check input:checked ~ .check-label {
        color: #fff;
    }

    .panel-actions {
        display: flex;
        justify-content: flex-end;
        margin-top: 0.25rem;
    }
    .panel-actions.align-center {
        align-items: center;
        justify-content: space-between;
        margin-top: 0.5rem;
    }
    .cyber-btn {
        padding: 0.5rem 1rem;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        font-size: 0.7rem;
        font-weight: 600;
        border-radius: 6px;
        cursor: pointer;
        letter-spacing: 1px;
        transition: all 0.2s;
    }
    .cyber-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.18);
    }
    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .sub-panel {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
</style>
