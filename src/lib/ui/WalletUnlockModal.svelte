<script>
    import { createEventDispatcher } from "svelte";

    const dispatch = createEventDispatcher();

    export let show = false;
    export let password = "";
    export let unlocking = false;
    export let error = "";
    export let title = "UNLOCK WALLET";
    export let body = "Your wallet is locked.";
    export let confirmLabel = "UNLOCK";
</script>

{#if show}
    <div
        class="wallet-unlock-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="wallet-unlock-title"
        tabindex="0"
        on:click={() => !unlocking && dispatch("cancel")}
        on:keydown={(e) => e.key === "Escape" && !unlocking && dispatch("cancel")}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="wallet-unlock-box" role="document" on:click|stopPropagation>
            <div class="wallet-unlock-header">
                <span class="wallet-unlock-icon">🔒</span>
                <h2 id="wallet-unlock-title">{title}</h2>
            </div>
            <div class="wallet-unlock-body">
                <p>{body}</p>
                <input
                    class="wallet-unlock-input"
                    type="password"
                    bind:value={password}
                    placeholder="Wallet passphrase"
                    autocomplete="current-password"
                    on:keydown={(e) => e.key === "Enter" && !unlocking && dispatch("confirm")}
                    disabled={unlocking}
                />
                {#if error}
                    <div class="wallet-unlock-error">{error}</div>
                {/if}
            </div>
            <div class="wallet-unlock-footer">
                <button
                    type="button"
                    class="wallet-unlock-cancel"
                    on:click={() => dispatch("cancel")}
                    disabled={unlocking}
                >CANCEL</button>
                <button
                    type="button"
                    class="wallet-unlock-confirm"
                    on:click={() => dispatch("confirm")}
                    disabled={unlocking || !password.trim()}
                >{unlocking ? "UNLOCKING..." : confirmLabel}</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .wallet-unlock-overlay {
        position: fixed;
        inset: 0;
        z-index: 600;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.82);
        backdrop-filter: blur(3px);
    }
    .wallet-unlock-box {
        width: min(30rem, 92vw);
        overflow: hidden;
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 8px;
        background: linear-gradient(180deg, #020403 0%, #070b09 100%);
        box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
    }
    .wallet-unlock-header {
        display: flex;
        align-items: center;
        gap: 0.65rem;
        padding: 0.85rem 1rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.18);
        background: rgba(0, 255, 65, 0.07);
    }
    .wallet-unlock-icon { color: var(--color-primary); font-size: 1rem; }
    .wallet-unlock-header h2 {
        margin: 0;
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.82rem;
        letter-spacing: 1.8px;
    }
    .wallet-unlock-body {
        display: grid;
        gap: 0.75rem;
        padding: 1rem;
    }
    .wallet-unlock-body p {
        margin: 0;
        color: #ffd36a;
        font-size: 0.7rem;
        line-height: 1.45;
    }
    .wallet-unlock-input {
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
    .wallet-unlock-input:focus {
        border-color: var(--color-primary);
        outline: none;
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.16);
    }
    .wallet-unlock-error {
        border: 1px solid rgba(255, 85, 85, 0.28);
        border-radius: 5px;
        background: rgba(255, 85, 85, 0.09);
        color: #ff7777;
        font-size: 0.64rem;
        padding: 0.5rem 0.6rem;
    }
    .wallet-unlock-footer {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.7rem;
        padding: 0.85rem 1rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.28);
    }
    .wallet-unlock-cancel,
    .wallet-unlock-confirm {
        min-height: 2.35rem;
        border-radius: 5px;
        font-family: var(--font-mono);
        font-size: 0.68rem;
        font-weight: 700;
        letter-spacing: 0.8px;
        cursor: pointer;
    }
    .wallet-unlock-cancel {
        border: 1px solid #666;
        background: transparent;
        color: #999;
    }
    .wallet-unlock-cancel:hover:not(:disabled) {
        border-color: #ff5555;
        color: #ff5555;
    }
    .wallet-unlock-confirm {
        border: 1px solid var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }
    .wallet-unlock-confirm:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.25);
    }
    .wallet-unlock-cancel:disabled,
    .wallet-unlock-confirm:disabled,
    .wallet-unlock-input:disabled { cursor: not-allowed; opacity: 0.48; }
</style>
