<script>
    import { createEventDispatcher } from "svelte";

    const dispatch = createEventDispatcher();

    export let show = false;
    export let password = "";
    export let unlocking = false;
    export let error = "";
    export let title = "UNLOCK VAULT";
    export let body = "Your vault is locked.";
    export let confirmLabel = "UNLOCK";
</script>

{#if show}
    <div
        class="vault-unlock-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="vault-unlock-title"
        tabindex="0"
        on:click={() => !unlocking && dispatch("cancel")}
        on:keydown={(e) => e.key === "Escape" && !unlocking && dispatch("cancel")}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="vault-unlock-box" role="document" on:click|stopPropagation>
            <div class="vault-unlock-header">
                <h2 id="vault-unlock-title">{title}</h2>
            </div>
            <div class="vault-unlock-divider"></div>
            <div class="vault-unlock-body">
                <p>{body}</p>
                <input
                    class="vault-unlock-input"
                    type="password"
                    bind:value={password}
                    placeholder="Vault passphrase"
                    autocomplete="off"
                    on:keydown={(e) => e.key === "Enter" && !unlocking && dispatch("confirm")}
                    disabled={unlocking}
                />
                {#if error}
                    <div class="vault-unlock-error">{error}</div>
                {/if}
            </div>
            <div class="vault-unlock-footer">
                <button
                    type="button"
                    class="vault-unlock-cancel"
                    on:click={() => dispatch("cancel")}
                    disabled={unlocking}
                >CANCEL</button>
                <button
                    type="button"
                    class="vault-unlock-confirm"
                    on:click={() => dispatch("confirm")}
                    disabled={unlocking || !password.trim()}
                >{unlocking ? "UNLOCKING..." : confirmLabel}</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .vault-unlock-overlay {
        position: fixed;
        inset: 0;
        z-index: 601;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.82);
        backdrop-filter: blur(3px);
    }
    .vault-unlock-box {
        width: min(25rem, 92vw);
        overflow: hidden;
        border: 1px solid rgba(0, 255, 65, 0.32);
        border-radius: 8px;
        background: rgba(5, 10, 7, 0.96);
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.82), 0 0 36px rgba(0, 255, 65, 0.14);
        padding: 1.5rem;
    }
    .vault-unlock-header {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0;
    }
    .vault-unlock-header h2 {
        margin: 0;
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.9rem;
        letter-spacing: 1px;
    }
    .vault-unlock-divider {
        height: 1px;
        margin: 0.75rem 0;
        background: linear-gradient(90deg, transparent, rgba(0, 255, 65, 0.32), transparent);
    }
    .vault-unlock-body {
        display: grid;
        gap: 0.75rem;
        padding: 0;
    }
    .vault-unlock-body p {
        margin: 0;
        color: #aaa;
        font-size: 0.7rem;
        line-height: 1.45;
    }
    .vault-unlock-input {
        width: 100%;
        min-height: 2.35rem;
        border: 1px solid rgba(0, 255, 65, 0.24);
        border-radius: 6px;
        background: rgba(0, 0, 0, 0.62);
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.72rem;
        padding: 0.5rem 0.65rem;
    }
    .vault-unlock-input:focus {
        border-color: var(--color-primary);
        outline: none;
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.16);
    }
    .vault-unlock-error {
        border: 1px solid rgba(255, 85, 85, 0.28);
        border-radius: 5px;
        background: rgba(255, 85, 85, 0.09);
        color: #ff7777;
        font-size: 0.64rem;
        padding: 0.5rem 0.6rem;
    }
    .vault-unlock-footer {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.5rem;
        padding: 0.85rem 0 0;
        background: transparent;
    }
    .vault-unlock-cancel,
    .vault-unlock-confirm {
        min-height: 2.35rem;
        border-radius: 5px;
        font-family: var(--font-mono);
        font-size: 0.68rem;
        font-weight: 700;
        letter-spacing: 0.8px;
        cursor: pointer;
    }
    .vault-unlock-cancel {
        border: 1px solid rgba(255,255,255,0.18);
        background: transparent;
        color: #999;
    }
    .vault-unlock-cancel:hover:not(:disabled) {
        border-color: #ff5555;
        color: #ff5555;
    }
    .vault-unlock-confirm {
        border: 1px solid var(--color-primary);
        background: rgba(0, 255, 65, 0.12);
        color: var(--color-primary);
    }
    .vault-unlock-confirm:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 16px rgba(0, 255, 65, 0.38);
    }
    .vault-unlock-cancel:disabled,
    .vault-unlock-confirm:disabled,
    .vault-unlock-input:disabled { cursor: not-allowed; opacity: 0.48; }
</style>
