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
                <h3 class:is-warning={type === "warning"} class:is-error={type === "error"}>
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
        padding: 0.85rem 1rem;
        background: rgba(0, 255, 65, 0.04);
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        font-size: 0.85rem;
        letter-spacing: 1px;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .modal-header h3.is-warning {
        color: #ffcc66;
    }
    .modal-header h3.is-error {
        color: #ff8888;
    }
    .modal-body {
        padding: 1.25rem;
        text-align: center;
        color: #ccc;
        font-size: 0.85rem;
        line-height: 1.5;
        overflow-y: auto;
        max-height: min(36rem, calc(100dvh - 10rem));
    }
    .modal-actions {
        padding: 0.85rem 1rem;
        display: flex;
        justify-content: center;
        background: rgba(0, 0, 0, 0.35);
        border-top: 1px solid rgba(0, 255, 65, 0.08);
    }

    @media (max-width: 520px) {
        .modal-header h3 {
            font-size: 0.8rem;
        }
        .modal-body {
            padding: 1rem;
            font-size: 0.8rem;
        }
    }
</style>
