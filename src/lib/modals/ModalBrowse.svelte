<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let inline = false;

    function close() {
        dispatch("close");
    }
</script>

{#snippet panelContent()}
    <div class="modal-header">
        <h3>Browse Network Assets</h3>
        <button class="close-btn" on:click={close}>&times;</button>
    </div>

    <div class="modal-body">
        <div class="panel-body">
            <div class="empty-state">
                <div class="empty-icon">🔍</div>
                <div class="empty-text">Network browser coming soon...</div>
            </div>
        </div>
    </div>
{/snippet}

{#if isOpen}
    {#if inline}
        <div class="browse-panel" in:fade={{ duration: 150 }}>
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
        align-items: flex-start;
        justify-content: center;
        padding-top: 0.5rem;
        padding-bottom: 1.5rem;
        z-index: 200000;
        backdrop-filter: blur(5px);
    }
    .modal {
        width: 560px;
        max-width: 92vw;
        max-height: calc(100vh - 2rem);
        border: 1px solid rgba(0, 255, 65, 0.2);
        box-shadow: 0 0 30px rgba(0, 0, 0, 0.8);
        border-radius: 8px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        background: rgba(10, 15, 12, 0.98);
    }
    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 1rem 0.65rem;
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        flex-shrink: 0;
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
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

    .empty-state {
        text-align: center;
        padding: 4rem 2rem;
        color: #444;
        flex: 1;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        gap: 1rem;
    }
    .empty-icon {
        font-size: 2.5rem;
        opacity: 0.5;
    }
    .empty-text {
        font-size: 0.85rem;
        color: #555;
        letter-spacing: 1px;
    }

    /* Inline panel mode */
    .browse-panel {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
</style>
