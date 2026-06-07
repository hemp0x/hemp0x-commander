<script>
    import { createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";

    export let show = false;
    /** @type {{ messageExpiryDefault: number, discoveryScanDepth: number, autoDiscovery: boolean, pollingIntervalSeconds: number, autoBlockTags: string[] }} */
    export let settings = {
        messageExpiryDefault: 0,
        discoveryScanDepth: 500,
        autoDiscovery: true,
        pollingIntervalSeconds: 30,
        autoBlockTags: ["#SPAM"],
    };

    const dispatch = createEventDispatcher();

    /** @type {typeof settings} */
    let draft;
    let draftTagsText = "";

    $: draft = { ...settings };
    $: draftTagsText = (draft.autoBlockTags || []).join(", ");

    function close() {
        dispatch("close");
    }

    function apply() {
        const tags = draftTagsText
            .split(/[,\n]+/)
            .map((/** @type {string} */ t) => t.trim())
            .filter((/** @type {string} */ t) => t.length > 0);
        draft.autoBlockTags = tags;
        dispatch("save", { settings: { ...draft, autoBlockTags: tags } });
    }

    function setExpiry(days) {
        draft.messageExpiryDefault = days;
        draft = draft;
    }

    function resetDefaults() {
        draft = {
            messageExpiryDefault: 0,
            discoveryScanDepth: 500,
            autoDiscovery: true,
            pollingIntervalSeconds: 30,
            autoBlockTags: ["#SPAM"],
        };
        draftTagsText = "#SPAM";
    }
</script>

{#if show}
    <div
        class="h0xc-settings-overlay"
        role="dialog"
        aria-modal="true"
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        tabindex="0"
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div class="h0xc-settings-panel" on:click|stopPropagation on:keydown|stopPropagation role="document">
            <div class="sett-header">
                <span class="sett-title">CHAT SETTINGS</span>
                <button class="sett-close" on:click={close}>&times;</button>
            </div>
            <div class="sett-body">
                <div class="sett-section">
                    <div class="sett-label">MESSAGE EXPIRY (DEFAULT)</div>
                    <div class="sett-expiry-row">
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 0} on:click={() => setExpiry(0)}>None</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 1} on:click={() => setExpiry(1)}>1 Day</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 7} on:click={() => setExpiry(7)}>7 Days</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 30} on:click={() => setExpiry(30)}>30 Days</button>
                    </div>
                    <div class="sett-hint">Applies to new messages. Expired messages remain on-chain but are hidden by wallets that respect expiry metadata.</div>
                </div>

                <div class="sett-section">
                    <div class="sett-label">RECENT BLOCK SCAN DEPTH</div>
                    <input
                        type="number"
                        class="sett-input input-glass"
                        bind:value={draft.discoveryScanDepth}
                        min="100"
                        max="10000"
                        step="100"
                    />
                    <div class="sett-hint">Number of recent blocks to scan during manual discovery. Higher = more thorough but slower.</div>
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.autoDiscovery} />
                        <span class="sett-toggle-label">AUTO-DISCOVERY</span>
                    </label>
                    <div class="sett-hint">Automatically scan for new .H0XC participants when the chat is open.</div>
                </div>

                <div class="sett-section">
                    <div class="sett-label">POLLING INTERVAL (SECONDS)</div>
                    <input
                        type="number"
                        class="sett-input input-glass"
                        bind:value={draft.pollingIntervalSeconds}
                        min="10"
                        max="300"
                        step="5"
                    />
                    <div class="sett-hint">How often to refresh messages when chat view is open.</div>
                </div>

                <div class="sett-section">
                    <div class="sett-label">AUTO-BLOCK TAGS</div>
                    <div class="sett-hint">Messages containing these tags will be hidden automatically. One tag per line or comma-separated. Default: #SPAM</div>
                    <textarea
                        class="sett-input tags-input"
                        bind:value={draftTagsText}
                        placeholder="#SPAM"
                        rows="3"
                    ></textarea>
                </div>
            </div>
            <div class="sett-footer">
                <button class="sett-btn reset" on:click={resetDefaults}>Reset Defaults</button>
                <button class="sett-btn cancel" on:click={close}>Cancel</button>
                <button class="sett-btn save" on:click={apply}>Apply</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .h0xc-settings-overlay {
        position: absolute;
        inset: 0;
        z-index: 100;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(0, 0, 0, 0.75);
        backdrop-filter: blur(2px);
    }
    .h0xc-settings-panel {
        width: min(26rem, 88vw);
        background: linear-gradient(180deg, #080b09, #0f1410);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        box-shadow: 0 16px 48px rgba(0, 0, 0, 0.8);
        overflow: hidden;
    }
    .sett-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.55rem 0.7rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.05);
    }
    .sett-title {
        font-size: 0.65rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.2px;
    }
    .sett-close {
        background: none;
        border: none;
        color: #666;
        font-size: 1.2rem;
        cursor: pointer;
        line-height: 1;
    }
    .sett-close:hover { color: #fff; }
    .sett-body {
        padding: 0.8rem;
        display: flex;
        flex-direction: column;
        gap: 0.7rem;
    }
    .sett-section {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .sett-label {
        font-size: 0.55rem;
        color: #777;
        letter-spacing: 0.5px;
        font-weight: 600;
    }
    .sett-hint {
        font-size: 0.48rem;
        color: #555;
        line-height: 1.4;
    }
    .sett-expiry-row {
        display: flex;
        gap: 0.3rem;
    }
    .sett-expiry-btn {
        flex: 1;
        padding: 0.3rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        color: #888;
        font-size: 0.58rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.15s;
    }
    .sett-expiry-btn:hover { border-color: rgba(0, 255, 65, 0.3); }
    .sett-expiry-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .sett-input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        padding: 0.35rem 0.5rem;
        color: #fff;
        font-size: 0.65rem;
        font-family: var(--font-mono);
        max-width: 120px;
        outline: none;
    }
    .sett-input:focus { border-color: var(--color-primary); }
    .sett-toggle-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        cursor: pointer;
    }
    .sett-toggle-row input[type="checkbox"] {
        accent-color: var(--color-primary);
        width: 0.75rem;
        height: 0.75rem;
        cursor: pointer;
    }
    .sett-toggle-label {
        font-size: 0.58rem;
        color: #aaa;
        font-weight: 600;
        letter-spacing: 0.5px;
    }
    .sett-footer {
        display: flex;
        gap: 0.4rem;
        padding: 0.6rem 0.8rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
        justify-content: flex-end;
    }
    .sett-btn {
        padding: 0.35rem 0.65rem;
        border-radius: 5px;
        font-size: 0.58rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .sett-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .sett-btn.cancel:hover { border-color: #ff5555; color: #ff5555; }
    .sett-btn.reset {
        background: transparent;
        border: 1px solid transparent;
        color: #666;
        margin-right: auto;
    }
    .sett-btn.reset:hover { color: #ffaa00; }
    .sett-btn.save {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
    }
    .sett-btn.save:hover {
        background: var(--color-primary);
        color: #000;
    }
    .tags-input {
        resize: vertical;
        min-height: 2.5rem;
    }
</style>
