<script>
    import { createEventDispatcher } from "svelte";
    import { cleanWalletPin } from "../walletPinUnlock.js";

    const dispatch = createEventDispatcher();

    export let show = false;
    export let password = "";
    export let pin = "";
    // "passphrase" | "pin". Defaults to "passphrase" so callers that do not
    // opt into PIN support are unaffected.
    export let mode = "passphrase";
    export let unlocking = false;
    export let error = "";
    export let title = "UNLOCK WALLET";
    export let body = "Your wallet is locked.";
    export let confirmLabel = "UNLOCK";
    // When true in PIN mode, the PIN layer requires the full wallet passphrase
    // (rotation / wallet changed / lockout / identity unavailable). The modal
    // hides the PIN entry and shows the reason + a direct path to passphrase.
    export let pinRequiresPassphrase = false;
    export let pinRequiresPassphraseReason = "";
    export let lockoutRemainingSecs = 0;
    // Action affordances (only rendered when the corresponding flag is true).
    export let showUsePassphrase = true;
    export let showUsePin = true;
    export let showForgotPin = true;
    export let pinConfigured = false;

    function switchToPassphrase() {
        pin = "";
        error = "";
        dispatch("usepassphrase");
    }

    function switchToPin() {
        error = "";
        dispatch("usepin");
    }

    function forgotPin() {
        if (unlocking) return;
        pin = "";
        error = "";
        dispatch("forgotpin");
    }
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
                <span class="wallet-unlock-icon">{mode === "pin" && !pinRequiresPassphrase ? "PIN" : "KEY"}</span>
                <h2 id="wallet-unlock-title">{title}</h2>
            </div>
            <div class="wallet-unlock-body">
                <p>{body}</p>

                {#if mode === "pin" && !pinRequiresPassphrase}
                    <input
                        class="wallet-unlock-input wallet-unlock-input-pin"
                        type="password"
                        inputmode="numeric"
                        pattern="[0-9]*"
                        autocomplete="off"
                        maxlength="6"
                        bind:value={pin}
                        placeholder="6-digit PIN"
                        on:input={(e) => (pin = cleanWalletPin(e.currentTarget.value))}
                        on:keydown={(e) => e.key === "Enter" && !unlocking && pin.length === 6 && dispatch("confirm")}
                        disabled={unlocking}
                    />
                {:else}
                    <input
                        class="wallet-unlock-input"
                        type="password"
                        bind:value={password}
                        placeholder="Wallet passphrase"
                        autocomplete="current-password"
                        on:keydown={(e) => e.key === "Enter" && !unlocking && password.trim() && dispatch("confirm")}
                        disabled={unlocking}
                    />
                {/if}

                {#if mode === "pin" && pinRequiresPassphrase && pinRequiresPassphraseReason}
                    <div class="wallet-unlock-info">{pinRequiresPassphraseReason}</div>
                {/if}

                {#if lockoutRemainingSecs > 0 && mode === "pin"}
                    <div class="wallet-unlock-info">
                        PIN unlock temporarily locked. Try again in {Math.ceil(lockoutRemainingSecs)}s, or use your wallet passphrase.
                    </div>
                {/if}

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
                    disabled={unlocking || (mode === "pin" && !pinRequiresPassphrase ? pin.length !== 6 : !password.trim())}
                >{unlocking ? "UNLOCKING..." : confirmLabel}</button>
            </div>

            {#if mode === "pin"}
                <div class="wallet-unlock-actions">
                    {#if showUsePassphrase}
                        <button type="button" class="wallet-unlock-link" on:click={switchToPassphrase} disabled={unlocking}>
                            Use wallet passphrase
                        </button>
                    {/if}
                    {#if showForgotPin}
                        <button type="button" class="wallet-unlock-link wallet-unlock-link-danger" on:click={forgotPin} disabled={unlocking}>
                            Forgot PIN
                        </button>
                    {/if}
                </div>
            {:else if pinConfigured && showUsePin}
                <div class="wallet-unlock-actions">
                    <button type="button" class="wallet-unlock-link" on:click={switchToPin} disabled={unlocking}>
                        Use device PIN instead
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<style>
    .wallet-unlock-overlay {
        position: fixed;
        inset: 0;
        z-index: 100000;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.78);
        backdrop-filter: blur(6px);
    }
    .wallet-unlock-box {
        width: min(31rem, 92vw);
        overflow: hidden;
        border: 1.5px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        background:
            radial-gradient(circle at top left, rgba(0, 255, 65, 0.08), transparent 34%),
            linear-gradient(180deg, #050706 0%, #010201 100%);
        box-shadow:
            0 24px 60px rgba(0, 0, 0, 0.84),
            0 0 28px rgba(0, 255, 65, 0.08);
    }
    .wallet-unlock-header {
        display: flex;
        align-items: center;
        gap: 0.65rem;
        padding: 0.9rem 1rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.18);
        background: rgba(0, 255, 65, 0.055);
    }
    .wallet-unlock-icon {
        display: inline-grid;
        place-items: center;
        width: 2rem;
        height: 2rem;
        border: 1px solid rgba(0, 255, 65, 0.28);
        border-radius: 7px;
        background: rgba(0, 255, 65, 0.08);
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.62rem;
        font-weight: 800;
        letter-spacing: 0.8px;
        box-shadow: inset 0 0 14px rgba(0, 255, 65, 0.05);
    }
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
        padding: 1rem 1rem 0.95rem;
    }
    .wallet-unlock-body p {
        margin: 0;
        color: #aeb7b2;
        font-size: 0.7rem;
        line-height: 1.45;
    }
    .wallet-unlock-input {
        width: 100%;
        min-height: 2.35rem;
        border: 1.5px solid rgba(0, 255, 65, 0.24);
        border-radius: 6px;
        background: rgba(0, 0, 0, 0.7);
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.72rem;
        padding: 0.5rem 0.65rem;
    }
    .wallet-unlock-input-pin {
        text-align: center;
        letter-spacing: 0.45rem;
        font-size: 1rem;
        color: var(--color-primary);
    }
    .wallet-unlock-input:focus {
        border-color: var(--color-primary);
        outline: none;
        box-shadow: 0 0 0 1px rgba(0, 255, 65, 0.08), 0 0 14px rgba(0, 255, 65, 0.16);
    }
    .wallet-unlock-error {
        border: 1px solid rgba(255, 85, 85, 0.3);
        border-radius: 6px;
        background: rgba(255, 85, 85, 0.09);
        color: #ff7777;
        font-size: 0.64rem;
        padding: 0.5rem 0.6rem;
    }
    .wallet-unlock-info {
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 6px;
        background: rgba(0, 255, 65, 0.06);
        color: #9fd8af;
        font-size: 0.64rem;
        line-height: 1.45;
        padding: 0.5rem 0.6rem;
    }
    .wallet-unlock-footer {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.7rem;
        padding: 0.85rem 1rem 0.95rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.28);
    }
    .wallet-unlock-cancel,
    .wallet-unlock-confirm {
        min-height: 2.35rem;
        border-radius: 6px;
        font-family: var(--font-mono);
        font-size: 0.68rem;
        font-weight: 700;
        letter-spacing: 0.8px;
        cursor: pointer;
    }
    .wallet-unlock-cancel {
        border: 1.5px solid rgba(255, 255, 255, 0.16);
        background: transparent;
        color: #9da7a1;
    }
    .wallet-unlock-cancel:hover:not(:disabled) {
        border-color: #ff5555;
        color: #ff5555;
    }
    .wallet-unlock-confirm {
        border: 1.5px solid var(--color-primary);
        background: linear-gradient(180deg, rgba(0, 255, 65, 0.95), rgba(0, 210, 54, 0.92));
        color: #001805;
        box-shadow: 0 0 16px rgba(0, 255, 65, 0.16);
    }
    .wallet-unlock-confirm:hover:not(:disabled) {
        background: var(--color-primary);
        color: #001805;
        box-shadow: 0 0 22px rgba(0, 255, 65, 0.25);
    }
    .wallet-unlock-cancel:disabled,
    .wallet-unlock-confirm:disabled,
    .wallet-unlock-input:disabled { cursor: not-allowed; opacity: 0.48; }
    .wallet-unlock-actions {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.35rem;
        padding: 0.6rem 1rem 0.85rem;
    }
    .wallet-unlock-link {
        background: none;
        border: none;
        color: #7fb69b;
        font-family: var(--font-mono);
        font-size: 0.62rem;
        letter-spacing: 0.4px;
        cursor: pointer;
        padding: 0.15rem 0.25rem;
        text-decoration: underline;
        text-underline-offset: 2px;
    }
    .wallet-unlock-link:hover:not(:disabled) { color: var(--color-primary); }
    .wallet-unlock-link-danger { color: #9a8a8a; }
    .wallet-unlock-link-danger:hover:not(:disabled) { color: #ff7777; }
    .wallet-unlock-link:disabled { cursor: not-allowed; opacity: 0.48; text-decoration: none; }
</style>
