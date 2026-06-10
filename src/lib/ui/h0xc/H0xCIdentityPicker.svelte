<script>
    import { createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";
    import { deriveRootNameFn } from "../../stores/h0xc.js";

    /** @type {string[]} */
    export let identities = [];
    export let loading = false;
    export let error = "";

    const dispatch = createEventDispatcher();

    /** @param {string} identity */
    function select(identity) {
        dispatch("select", { identity });
    }

    /** @param {string} id */
    function rootLabel(id) {
        if (!id) return "";
        return deriveRootNameFn(id).toUpperCase();
    }

    function cancel() {
        dispatch("cancel");
    }

    function newIdentity() {
        dispatch("create");
    }

    function browseAsGuest() {
        dispatch("guest");
    }
</script>

<div class="h0xc-identity-picker" transition:fade={{ duration: 120 }}>
    <div class="pick-header">
        <span class="pick-title">SELECT H0XC IDENTITY</span>
        <button class="pick-close" on:click={cancel}>&times;</button>
    </div>
    <div class="pick-body">
        {#if loading}
            <div class="pick-status">
                <span class="inline-spinner"></span>
                Loading identities...
            </div>
        {:else if error}
            <div class="pick-status error">{error}</div>
        {:else if identities.length === 0}
            <div class="pick-empty">
                <div class="empty-icon">◈</div>
                <div class="empty-title">No H0XC Identities</div>
                <div class="empty-desc">
                    You don't own any sub-assets ending in .H0XC. Create one from a root asset you own to participate in community chat.
                </div>
                <code class="step-command">issue YOURROOT/H0XC 1</code>
                <div class="empty-warning">
                    <span class="warn-icon">⚠</span>
                    <span class="warn-text">
                        This chat is <strong>fully public</strong>. Messages are on-chain <strong>forever</strong> and <strong>not encrypted</strong>.
                        For E2E encryption visit <a class="warn-link" href="https://hemp0x.social" target="_blank" rel="noopener">hemp0x.social</a>.
                    </span>
                </div>
                <div class="pick-actions">
                    <button class="pick-create-btn" on:click={newIdentity}>
                        Create .H0XC Sub-Asset
                    </button>
                    <button class="pick-guest-btn" on:click={browseAsGuest}>
                        Browse as Guest
                    </button>
                </div>
            </div>
        {:else}
            <div class="pick-list-label">Select an identity to join the chat:</div>
            <div class="pick-list">
                {#each identities as id}
                    <button class="pick-item" on:click={() => select(id)}>
                        <span class="pick-icon">◆</span>
                        <span class="pick-name">{id}</span>
                        <span class="pick-root">[{rootLabel(id)}]</span>
                    </button>
                {/each}
            </div>
            <div class="pick-divider"></div>
            <div class="pick-alt-actions">
                <button class="pick-guest-btn" on:click={browseAsGuest}>
                    Browse as Guest
                </button>
                <button class="pick-create-btn small" on:click={newIdentity}>
                    + Create Identity
                </button>
            </div>
            <div class="pick-note">Each identity is a sub-asset you own under a root asset. Creating one is an on-chain transaction.</div>
        {/if}
    </div>
</div>

<style>
    .h0xc-identity-picker {
        display: flex;
        flex-direction: column;
        flex: 1;
    }
    .pick-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.7rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
    }
    .pick-title {
        font-size: 0.68rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.2px;
    }
    .pick-close {
        background: none;
        border: none;
        color: #666;
        font-size: 1.2rem;
        cursor: pointer;
        padding: 0.15rem;
        line-height: 1;
    }
    .pick-close:hover { color: #fff; }
    .pick-body {
        padding: 0.8rem;
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        flex: 1;
    }
    .pick-status {
        font-size: 0.62rem;
        color: #888;
        text-align: center;
        padding: 1rem;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.4rem;
    }
    .inline-spinner {
        width: 12px;
        height: 12px;
        border: 2px solid rgba(0, 255, 65, 0.15);
        border-top-color: var(--color-primary);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
        display: inline-block;
        flex-shrink: 0;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    .pick-status.error {
        color: #ff5555;
    }
    .pick-empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.6rem;
        padding: 1.5rem 0;
        text-align: center;
    }
    .empty-icon {
        font-size: 2rem;
        color: var(--color-primary);
        opacity: 0.4;
    }
    .empty-title {
        font-size: 0.78rem;
        font-weight: 700;
        color: #ccc;
        letter-spacing: 0.5px;
    }
    .empty-desc {
        font-size: 0.65rem;
        color: #777;
        max-width: 280px;
        line-height: 1.55;
    }
    .step-command {
        display: inline-block;
        padding: 0.25rem 0.5rem;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 4px;
        font-family: var(--font-mono);
        font-size: 0.58rem;
        color: #aaa;
        letter-spacing: 0.3px;
    }
    .empty-warning {
        display: flex;
        align-items: flex-start;
        gap: 0.4rem;
        background: rgba(255, 170, 0, 0.04);
        border: 1px solid rgba(255, 170, 0, 0.12);
        border-radius: 6px;
        padding: 0.5rem 0.6rem;
        max-width: 280px;
        text-align: left;
    }
    .warn-icon {
        font-size: 0.7rem;
        color: #ffaa00;
        flex-shrink: 0;
        margin-top: 0.05rem;
    }
    .warn-text {
        font-size: 0.58rem;
        color: #999;
        line-height: 1.45;
    }
    .warn-text strong {
        color: #ccc;
    }
    .warn-link {
        color: var(--color-primary);
        text-decoration: underline;
        transition: color 0.15s;
    }
    .warn-link:hover {
        color: #fff;
    }
    .pick-list-label {
        font-size: 0.55rem;
        color: #666;
        letter-spacing: 0.5px;
        margin-bottom: 0.2rem;
    }
    .pick-list {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
        max-height: 240px;
        overflow-y: auto;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.25) transparent;
    }
    .pick-item {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        padding: 0.55rem 0.7rem;
        color: #ccc;
        font-size: 0.65rem;
        cursor: pointer;
        transition: all 0.15s;
        text-align: left;
        width: 100%;
    }
    .pick-item:hover {
        border-color: rgba(0, 255, 65, 0.3);
        background: rgba(0, 255, 65, 0.05);
    }
    .pick-icon {
        color: var(--color-primary);
        font-size: 0.7rem;
    }
    .pick-name {
        flex: 1;
        font-family: var(--font-mono);
    }
    .pick-root {
        font-weight: 700;
        color: var(--color-primary);
        font-size: 0.6rem;
        letter-spacing: 0.5px;
    }
    .pick-note {
        font-size: 0.5rem;
        color: #555;
        text-align: center;
        margin-top: 0.3rem;
    }
    .pick-actions {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        width: 100%;
        max-width: 280px;
    }
    .pick-create-btn {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        border-radius: 6px;
        padding: 0.5rem 1rem;
        color: var(--color-primary);
        font-size: 0.62rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        width: 100%;
    }
    .pick-create-btn:hover {
        background: var(--color-primary);
        color: #000;
    }
    .pick-create-btn.small {
        padding: 0.35rem 0.6rem;
        font-size: 0.55rem;
    }
    .pick-guest-btn {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 6px;
        padding: 0.5rem 1rem;
        color: #aaa;
        font-size: 0.62rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        width: 100%;
    }
    .pick-guest-btn:hover {
        border-color: rgba(255, 255, 255, 0.25);
        color: #fff;
    }
    .pick-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.3rem 0;
    }
    .pick-alt-actions {
        display: flex;
        gap: 0.4rem;
        justify-content: center;
        flex-wrap: wrap;
    }
    .pick-alt-actions .pick-guest-btn,
    .pick-alt-actions .pick-create-btn {
        width: auto;
        flex: 1;
        min-width: 120px;
    }
</style>
