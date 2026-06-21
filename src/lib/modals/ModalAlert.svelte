<script>
    import { fade, scale } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import "../../components.css";

    export let isOpen = false;
    export let title = "Alert";
    export let message = "";
    export let type = "info"; // info, warning, error, success

    const dispatch = createEventDispatcher();

    function close() {
        dispatch("close");
    }
</script>

{#if isOpen}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal-frame"
            role="alertdialog"
            aria-modal="true"
            tabindex="-1"
            on:click|stopPropagation
            on:keydown|stopPropagation
            transition:scale={{ duration: 150, start: 0.95 }}
        >
            <div class="modal-header">
                <h3>
                    {#if type === "warning"}⚠️{:else if type === "error"}❌{:else}ℹ️{/if}
                    {title}
                </h3>
            </div>
            <div class="modal-body">
                <p>{message}</p>
            </div>
            <div class="modal-actions">
                <button class="cyber-btn" on:click={close}>OK</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.8);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 999999; /* Highest priority */
        backdrop-filter: blur(2px);
    }
    .modal-frame {
        width: min(350px, 90vw);
        max-width: 90vw;
        max-height: min(40rem, calc(100dvh - 2rem));
        display: flex;
        flex-direction: column;
    }
    .modal-header {
        padding: 1rem;
        background: rgba(255, 255, 255, 0.05);
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        font-size: 1.1rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .modal-body {
        padding: 1.25rem;
        text-align: center;
        color: #ddd;
        font-size: 0.9rem;
        line-height: 1.5;
        overflow-y: auto;
        max-height: min(36rem, calc(100dvh - 10rem));
    }
    .modal-actions {
        padding: 0.85rem 1rem;
        display: flex;
        justify-content: center;
    }

    @media (max-width: 520px) {
        .modal-header h3 {
            font-size: 1rem;
        }
        .modal-body {
            padding: 1rem;
            font-size: 0.85rem;
        }
    }
</style>
