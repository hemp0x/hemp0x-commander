<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    const dispatch = createEventDispatcher();

    export let isOpen = false;

    function close() {
        dispatch("close");
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
            style="max-width: 800px; height: 80vh; display: flex; flex-direction: column;"
            on:click|stopPropagation
            transition:fly={{ y: 20 }}
        >
            <div class="form-header">
                <span class="form-title">BROWSE NETWORK ASSETS</span>
                <button class="modal-close" on:click={close}> × </button>
            </div>
            <div class="empty-state">
                <div class="empty-icon">🔍</div>
                <div class="empty-text">Network browser coming soon...</div>
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
    .form-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 1rem;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        margin-bottom: 0.75rem;
    }
    .form-title {
        color: var(--color-primary);
        font-size: 0.85rem;
        font-weight: 600;
        letter-spacing: 1.5px;
    }
    .modal-close {
        background: transparent;
        border: none;
        color: #555;
        font-size: 1.5rem;
        cursor: pointer;
        transition: all 0.15s;
        width: 32px;
        height: 32px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 8px;
    }
    .modal-close:hover {
        color: #fff;
        background: rgba(255, 255, 255, 0.1);
    }
    .empty-state {
        grid-column: 1 / -1;
        text-align: center;
        padding: 4rem 2rem;
        color: #444;
        flex: 1;
        display: flex;
        flex-direction: column;
        justify-content: center;
    }
    .empty-icon {
        font-size: 3rem;
        margin-bottom: 1rem;
        opacity: 0.3;
    }
    .empty-text {
        font-size: 0.85rem;
        letter-spacing: 1px;
    }
</style>
