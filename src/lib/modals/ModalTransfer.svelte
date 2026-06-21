<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import AddressBookPicker from "../ui/AddressBookPicker.svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let nodeOnline = false;
    export let inline = false;
    export let assets = [];
    export let showBack = false;

    // Bindable fields
    export let selectedAsset = "";
    export let toAddress = "";
    export let amount = "";

    function close() {
        dispatch("close");
    }

    function goBack() {
        dispatch("back");
    }

    function transfer() {
        dispatch("transfer");
    }
</script>

{#snippet panelContent()}
    <div class="modal-header">
        {#if showBack}
            <button class="back-btn" on:click={goBack} title="Back to Asset">←</button>
        {/if}
        <h3>Transfer {selectedAsset}</h3>
        <button class="close-btn" on:click={close}>&times;</button>
    </div>

    <div class="modal-body">
        <div class="panel-body">
            <div class="panel-title-row">
                <h4>Send Asset</h4>
            </div>

            <div class="field-group">
                <label for="tx-asset">Asset</label>
                <select id="tx-asset" bind:value={selectedAsset} class="cyber-input">
                    {#each assets as item}
                        <option value={item.name}>{item.name} • {item.balance}</option>
                    {/each}
                </select>
            </div>

            <AddressBookPicker
                id="tx-to"
                label="Recipient Address"
                bind:value={toAddress}
            />

            <div class="field-group narrow-inline">
                <label for="tx-amt">Amount</label>
                <input id="tx-amt" type="number" class="cyber-input mono" placeholder="0" bind:value={amount} />
            </div>

            <div class="panel-actions">
                <button class="cyber-btn" on:click={transfer} disabled={!nodeOnline}>
                    SEND TRANSFER
                </button>
            </div>
        </div>
    </div>
{/snippet}

{#if isOpen}
    {#if inline}
        <div class="transfer-panel" in:fade={{ duration: 150 }}>
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

    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .field-group.narrow-inline {
        max-width: 180px;
    }

    label {
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
    select.cyber-input {
        cursor: pointer;
        appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='%2300ff41'%3E%3Cpath d='M7 10l5 5 5-5z'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 8px center;
        background-size: 12px;
        padding-right: 28px;
    }
    select.cyber-input option {
        background: #0a0a0a;
        color: #ccc;
    }

    .panel-actions {
        display: flex;
        justify-content: flex-end;
        margin-top: 0.25rem;
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

    .transfer-panel {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
</style>
