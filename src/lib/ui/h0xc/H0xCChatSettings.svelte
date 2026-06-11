<script>
    import { createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";

    export let show = false;
    /** @type {{ messageExpiryDefault: number, autoDiscovery: boolean, pollingIntervalSeconds: number, autoBlockTags: string[], discoveryEnabled: boolean, muteNotifications: boolean, discoveryScanLimit: number, historyDays: number, showExpired: boolean, hideStaleUsers: boolean, staleUserDays: number, communityReportAutoHide: boolean, communityReportMinReports: number, communityReportMinRatio: number, communityReportWindowDays: number, showBroadcastPreview: boolean }} */
    export let settings = {
        messageExpiryDefault: 30,
        autoDiscovery: true,
        pollingIntervalSeconds: 30,
        autoBlockTags: ["#SPAM"],
        discoveryEnabled: true,
        muteNotifications: false,
        discoveryScanLimit: 2000,
        historyDays: 90,
        showExpired: false,
        hideStaleUsers: true,
        staleUserDays: 90,
        communityReportAutoHide: false,
        communityReportMinReports: 3,
        communityReportMinRatio: 0.4,
        communityReportWindowDays: 30,
        showBroadcastPreview: true,
    };
    /** @type {string[]} */
    export let blockedUsers = [];
    /** @type {"idle"|"scanning"|"paused"|"disabled"} */
    export let discoveryState = "idle";
    /** @type {string} */
    export let lastScanTime = "";
    /** @type {Array<{txid: string, timeSec: number, reason?: number}>} */
    export let localHiddenMessages = [];
    /** @type {Array<{channel: string, timeSec: number, reason?: number, rootName?: string}>} */
    export let localHiddenChannels = [];
    /** @type {Array<{target: string, targetType: number, count: number, channels: string[], maxSeverity: number, expiryTs: number}>} */
    export let communityHidden = [];

    const dispatch = createEventDispatcher();

    /** @type {typeof settings} */
    let draft = { ...settings };
    let draftTagsText = "";

    function syncFromSettings() {
        draft = { ...settings };
        const tags = Array.isArray(settings.autoBlockTags) ? settings.autoBlockTags : ["#SPAM"];
        draftTagsText = tags.join(", ");
    }

    $: if (show) syncFromSettings();

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
            messageExpiryDefault: 30,
            autoDiscovery: true,
            pollingIntervalSeconds: 30,
            autoBlockTags: ["#SPAM"],
            discoveryEnabled: true,
            muteNotifications: false,
            discoveryScanLimit: 2000,
            historyDays: 90,
            showExpired: false,
            hideStaleUsers: true,
            staleUserDays: 90,
            communityReportAutoHide: false,
            communityReportMinReports: 3,
            communityReportMinRatio: 0.4,
            communityReportWindowDays: 30,
            showBroadcastPreview: true,
        };
        draftTagsText = "#SPAM";
    }

    /** @param {string} rootName */
    function unblock(rootName) {
        dispatch("unblock", { rootName });
    }
</script>

{#if show}
    <div
        class="sett-overlay"
        role="dialog"
        aria-modal="true"
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        tabindex="0"
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div class="sett-panel" on:click|stopPropagation on:keydown|stopPropagation role="document">
            <div class="sett-header">
                <span class="sett-title">CHAT SETTINGS</span>
                <button class="sett-close" on:click={close}>&times;</button>
            </div>

            <div class="sett-body">
                <div class="sett-section">
                    <span class="sett-label">MESSAGE EXPIRY (DEFAULT)</span>
                    <div class="sett-expiry-row">
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 0} on:click={() => setExpiry(0)}>None</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 1} on:click={() => setExpiry(1)}>1 Day</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 7} on:click={() => setExpiry(7)}>7 Days</button>
                        <button class="sett-expiry-btn" class:active={draft.messageExpiryDefault === 30} on:click={() => setExpiry(30)}>30 Days</button>
                    </div>
                    <p class="sett-hint">Applies to new messages. Expired messages remain on-chain but are hidden by wallets that respect expiry metadata.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">CHAT HISTORY WINDOW</span>
                    <div class="sett-expiry-row">
                        <button class="sett-expiry-btn" class:active={draft.historyDays === 30} on:click={() => { draft.historyDays = 30; draft = draft; }}>30 Days</button>
                        <button class="sett-expiry-btn" class:active={draft.historyDays === 90} on:click={() => { draft.historyDays = 90; draft = draft; }}>90 Days</button>
                        <button class="sett-expiry-btn" class:active={draft.historyDays === 180} on:click={() => { draft.historyDays = 180; draft = draft; }}>180 Days</button>
                        <button class="sett-expiry-btn" class:active={draft.historyDays === 0} on:click={() => { draft.historyDays = 0; draft = draft; }}>All</button>
                    </div>
                    <p class="sett-hint">How far back to show messages in the chat feed. Older messages are filtered in the UI only — nothing is deleted from the chain. You can also click "Load older messages" in the chat to extend the view.</p>
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.showExpired} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">SHOW EXPIRED MESSAGES</span>
                    </label>
                    <p class="sett-hint">Expired messages remain on-chain but are hidden by default. Enable this to see them in the chat feed.</p>
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.showBroadcastPreview} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">SHOW BROADCAST PREVIEW</span>
                    </label>
                    <p class="sett-hint">When enabled, a compact confirmation panel shows before each send with the channel, message preview, encoded hex, expiry, and an irreversibility warning.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">STALE USER FILTER</span>
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.hideStaleUsers} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">HIDE STALE USERS</span>
                    </label>
                    {#if draft.hideStaleUsers}
                        <div class="sett-expiry-row" style="margin-top: 0.3rem;">
                            <button class="sett-expiry-btn" class:active={draft.staleUserDays === 30} on:click={() => { draft.staleUserDays = 30; draft = draft; }}>30 Days</button>
                            <button class="sett-expiry-btn" class:active={draft.staleUserDays === 90} on:click={() => { draft.staleUserDays = 90; draft = draft; }}>90 Days</button>
                            <button class="sett-expiry-btn" class:active={draft.staleUserDays === 180} on:click={() => { draft.staleUserDays = 180; draft = draft; }}>180 Days</button>
                        </div>
                    {/if}
                    <p class="sett-hint">Users with no messages for this period are hidden from the user list. They reappear when they send again. No data is deleted.</p>
                </div>

                <div class="sett-section">
                    <div class="field-row">
                        <div class="field-group narrow-inline">
                            <span class="sett-label">DISCOVERY SCAN LIMIT</span>
                            <input
                                type="number"
                                class="cyber-input"
                                bind:value={draft.discoveryScanLimit}
                                min="100"
                                max="2000"
                                step="100"
                            />
                        </div>
                        <div class="field-group narrow-inline">
                            <span class="sett-label">POLLING (SEC)</span>
                            <input
                                type="number"
                                class="cyber-input"
                                bind:value={draft.pollingIntervalSeconds}
                                min="10"
                                max="300"
                                step="5"
                            />
                        </div>
                    </div>
                    <p class="sett-hint">Max participants returned per discovery scan. Background scans use ≤200. This does not affect message history, which loads via Core message RPCs independently.</p>
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.discoveryEnabled} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">ENABLE DISCOVERY</span>
                    </label>
                    <p class="sett-hint">Scan for new .H0XC channels and participants. When disabled, messages still load from already known/subscribed channels via Core message RPCs.</p>
                    {#if !draft.discoveryEnabled}
                        <div class="sett-discovery-note">
                            <span class="sett-discovery-note-icon">ℹ</span>
                            <span>Discovery is off. You can still see messages from channels you're already subscribed to.</span>
                        </div>
                    {/if}
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" checked={!draft.muteNotifications} on:change={() => { draft.muteNotifications = !draft.muteNotifications; draft = draft; }} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">{draft.muteNotifications ? "NOTIFICATIONS MUTED" : "NOTIFICATIONS ACTIVE"}</span>
                    </label>
                    <p class="sett-hint">New message notifications for H0XC. Discovery continues even when notifications are muted.</p>
                </div>

                <div class="sett-section">
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.autoDiscovery} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">AUTO-DISCOVERY</span>
                    </label>
                    <p class="sett-hint">Automatically scan for new .H0XC participants when the chat is open and idle.</p>
                </div>

                {#if discoveryState !== "idle" || lastScanTime}
                    <div class="sett-section">
                        <span class="sett-label">DISCOVERY STATE</span>
                        <div class="sett-discovery-state">
                            <span class="sett-discovery-dot" class:scanning={discoveryState === "scanning"} class:disabled={discoveryState === "disabled"} class:paused={discoveryState === "paused"}></span>
                            <span class="sett-discovery-state-text">{discoveryState}{lastScanTime ? ` · last: ${lastScanTime}` : ""}</span>
                        </div>
                    </div>
                {/if}

                <div class="sett-section">
                    <span class="sett-label">AUTO-BLOCK TAGS</span>
                    <p class="sett-hint">Channels whose authority-holder address is tagged with a configured qualifier are hidden automatically. This requires Core to resolve channel authority addresses. Local block/unblock below always works as a fallback.</p>
                    <textarea
                        class="cyber-input tags-input"
                        bind:value={draftTagsText}
                        placeholder="#SPAM"
                        rows="2"
                    ></textarea>
                    <p class="sett-hint">Separate multiple tags with commas. Example: #SPAM, #BAN, #SCAM</p>
                    <p class="sett-hint sett-limitation">If Core cannot resolve authority holders, tag auto-blocking is inactive. Use the local block list below for a guaranteed fallback.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">LOCAL BLOCK LIST</span>
                    {#if blockedUsers.length === 0}
                        <p class="sett-hint">No locally blocked H0XC identities.</p>
                    {:else}
                        <div class="blocked-list">
                            {#each blockedUsers as rootName}
                                <div class="blocked-row">
                                    <span class="blocked-name">[{rootName.toUpperCase()}]</span>
                                    <button class="blocked-unblock" on:click={() => unblock(rootName)}>Unblock</button>
                                </div>
                            {/each}
                        </div>
                    {/if}
                    <p class="sett-hint">Local blocks are private to this Commander install and can be changed at any time.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">COMMUNITY REPORTS</span>
                    <label class="sett-toggle-row">
                        <input type="checkbox" bind:checked={draft.communityReportAutoHide} />
                        <span class="checkbox-visual"></span>
                        <span class="sett-toggle-label">AUTO-HIDE BY COMMUNITY REPORTS</span>
                    </label>
                    <p class="sett-hint">When enabled, messages/channels with enough community reports are automatically hidden locally. Reports from blocked/muted users are ignored.</p>
                    {#if draft.communityReportAutoHide}
                        <div class="field-row" style="margin-top: 0.3rem;">
                            <div class="field-group narrow-inline">
                                <span class="sett-label">MIN REPORTS</span>
                                <input type="number" class="cyber-input" bind:value={draft.communityReportMinReports} min="1" max="20" />
                            </div>
                            <div class="field-group narrow-inline">
                                <span class="sett-label">MIN RATIO</span>
                                <input type="number" class="cyber-input" bind:value={draft.communityReportMinRatio} min="0.1" max="1.0" step="0.1" />
                            </div>
                            <div class="field-group narrow-inline">
                                <span class="sett-label">WINDOW (DAYS)</span>
                                <input type="number" class="cyber-input" bind:value={draft.communityReportWindowDays} min="1" max="365" />
                            </div>
                        </div>
                        <p class="sett-hint">Auto-hide triggers when unique reporting channels ≥ min reports AND ratio of reporting channels / recent participants ≥ min ratio.</p>
                    {/if}
                </div>

                <div class="sett-section">
                    <span class="sett-label">LOCALLY HIDDEN MESSAGES</span>
                    {#if localHiddenMessages.length === 0}
                        <p class="sett-hint">No locally hidden messages.</p>
                    {:else}
                        <div class="blocked-list">
                            {#each localHiddenMessages as entry}
                                <div class="blocked-row">
                                    <span class="blocked-name" title={entry.txid}>{entry.txid.slice(0, 12)}...</span>
                                    <button class="blocked-unblock" on:click={() => dispatch("unhideMessage", { txid: entry.txid })}>Unhide</button>
                                </div>
                            {/each}
                        </div>
                    {/if}
                    <p class="sett-hint">Messages you have reported or hidden locally. Unhiding removes the local override.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">LOCALLY HIDDEN CHANNELS</span>
                    {#if localHiddenChannels.length === 0}
                        <p class="sett-hint">No locally hidden channels.</p>
                    {:else}
                        <div class="blocked-list">
                            {#each localHiddenChannels as entry}
                                <div class="blocked-row">
                                    <span class="blocked-name">[{entry.channel}]</span>
                                    <button class="blocked-unblock" on:click={() => dispatch("unhideChannel", { channel: entry.channel })}>Unhide</button>
                                </div>
                            {/each}
                        </div>
                    {/if}
                    <p class="sett-hint">Channels/users you have reported or hidden locally. Unhiding removes the local override.</p>
                </div>

                <div class="sett-section">
                    <span class="sett-label">COMMUNITY HIDDEN</span>
                    {#if communityHidden.length === 0}
                        <p class="sett-hint">No entries are hidden by community reports.</p>
                    {:else}
                        <div class="blocked-list">
                            {#each communityHidden as entry}
                                <div class="blocked-row">
                                    <span class="blocked-name" title={entry.target}>
                                        {entry.targetType === 1 ? entry.target.slice(0, 12) + "..." : "Channel fingerprint " + entry.target.slice(0, 12) + "..."}
                                        · {entry.count} reports
                                    </span>
                                    <button class="blocked-unblock" on:click={() => dispatch("allowCommunityHidden", { target: entry.target })}>Allow</button>
                                </div>
                            {/each}
                        </div>
                    {/if}
                    <p class="sett-hint">Allow keeps a community-hidden item visible for this Commander install.</p>
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
    .sett-overlay {
        position: absolute;
        inset: 0;
        z-index: 100;
        display: flex;
        align-items: stretch;
        justify-content: stretch;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(4px);
        padding: 0.5rem;
    }
    .sett-panel {
        width: 100%;
        height: 100%;
        max-width: 100%;
        max-height: 100%;
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 8px;
        box-shadow: 0 16px 48px rgba(0, 0, 0, 0.85);
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .sett-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.85rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
        background: rgba(0, 0, 0, 0.25);
        flex-shrink: 0;
    }
    .sett-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.2px;
    }
    .sett-close {
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
    .sett-close:hover { color: #fff; }

    .sett-body {
        padding: 0.6rem 0.85rem;
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        overflow-y: auto;
        overflow-x: hidden;
        flex: 1 1 0%;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .sett-body::-webkit-scrollbar { width: 6px; }
    .sett-body::-webkit-scrollbar-track { background: transparent; }
    .sett-body::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.35); border-radius: 3px; }
    .sett-body::-webkit-scrollbar-thumb:hover { background: rgba(0, 255, 65, 0.55); }

    .sett-section {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .sett-label {
        color: #888;
        font-size: 0.6rem;
        letter-spacing: 0.5px;
        font-weight: 600;
    }
    .sett-hint {
        margin: 0;
        color: #666;
        font-size: 0.55rem;
        line-height: 1.45;
    }
    .sett-limitation {
        color: #8a8a5a;
        font-style: italic;
    }
    .sett-discovery-note {
        display: flex;
        align-items: flex-start;
        gap: 0.35rem;
        background: rgba(0, 255, 65, 0.04);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 5px;
        padding: 0.4rem 0.55rem;
        margin-top: 0.15rem;
    }
    .sett-discovery-note-icon {
        color: var(--color-primary);
        font-size: 0.6rem;
        flex-shrink: 0;
        margin-top: 0.05rem;
    }
    .sett-discovery-note span {
        font-size: 0.55rem;
        color: #888;
        line-height: 1.4;
    }
    .sett-discovery-state {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 5px;
    }
    .sett-discovery-dot {
        width: 7px;
        height: 7px;
        border-radius: 50%;
        background: #444;
        flex-shrink: 0;
    }
    .sett-discovery-dot.scanning {
        background: var(--color-primary);
        box-shadow: 0 0 6px var(--color-primary);
        animation: disco-pulse 1.2s infinite ease-in-out;
    }
    .sett-discovery-dot.disabled { background: #ff5555; }
    .sett-discovery-dot.paused { background: #ffaa00; }
    @keyframes disco-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
    .sett-discovery-state-text {
        font-size: 0.55rem;
        color: #888;
        font-family: var(--font-mono);
    }

    .field-row {
        display: flex;
        align-items: flex-end;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
        flex: 1;
        min-width: 0;
    }
    .field-group.narrow-inline {
        flex: 1 1 0%;
        min-width: 140px;
    }

    .cyber-input {
        width: 100%;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 0, 0, 0.45);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #ddd;
        font-family: var(--font-mono);
        font-size: 0.65rem;
        box-sizing: border-box;
        outline: none;
        transition: all 0.15s;
    }
    .cyber-input:focus { border-color: var(--color-primary); }
    .cyber-input::placeholder { color: #555; }
    .tags-input {
        resize: vertical;
        min-height: 2.2rem;
    }

    .sett-expiry-row {
        display: flex;
        gap: 0.3rem;
    }
    .sett-expiry-btn {
        flex: 1;
        padding: 0.3rem 0.25rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        color: #888;
        font-size: 0.58rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.15s;
        letter-spacing: 0.3px;
    }
    .sett-expiry-btn:hover { border-color: rgba(255, 255, 255, 0.2); color: #aaa; }
    .sett-expiry-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.35);
        color: var(--color-primary);
    }

    .sett-toggle-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        cursor: pointer;
    }
    .sett-toggle-row input[type="checkbox"] {
        display: none;
    }
    .checkbox-visual {
        width: 14px;
        height: 14px;
        border: 2px solid #444;
        border-radius: 4px;
        transition: all 0.15s;
        position: relative;
        flex-shrink: 0;
    }
    .sett-toggle-row input:checked + .checkbox-visual {
        background: var(--color-primary);
        border-color: var(--color-primary);
        box-shadow: 0 0 8px var(--color-primary);
    }
    .sett-toggle-row input:checked + .checkbox-visual::after {
        content: "✓";
        position: absolute;
        top: -1px;
        left: 1px;
        font-size: 10px;
        color: #000;
        font-weight: bold;
    }
    .sett-toggle-label {
        font-size: 0.6rem;
        color: #aaa;
        font-weight: 600;
        letter-spacing: 0.5px;
    }

    .blocked-list {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
        gap: 0.3rem;
        max-height: 14rem;
        overflow-y: auto;
        padding: 0.3rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.25) transparent;
    }
    .blocked-list::-webkit-scrollbar { width: 5px; }
    .blocked-list::-webkit-scrollbar-track { background: transparent; }
    .blocked-list::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.25); border-radius: 3px; }
    .blocked-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.5rem;
        padding: 0.3rem 0.45rem;
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 5px;
        background: rgba(0, 0, 0, 0.3);
    }
    .blocked-name {
        color: #bbb;
        font-size: 0.58rem;
        font-family: var(--font-mono);
    }
    .blocked-unblock {
        border: 1px solid rgba(0, 255, 65, 0.3);
        border-radius: 4px;
        background: rgba(0, 255, 65, 0.06);
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.52rem;
        cursor: pointer;
        padding: 0.18rem 0.35rem;
        text-transform: uppercase;
        font-weight: 600;
        letter-spacing: 0.3px;
        transition: all 0.15s;
    }
    .blocked-unblock:hover {
        background: rgba(0, 255, 65, 0.12);
    }

    .sett-footer {
        display: flex;
        gap: 0.4rem;
        padding: 0.55rem 0.85rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
        justify-content: flex-end;
        flex-shrink: 0;
    }
    .sett-btn {
        padding: 0.4rem 0.7rem;
        border-radius: 6px;
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        border: 1px solid transparent;
    }
    .sett-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border-color: rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .sett-btn.cancel:hover { border-color: #ff5555; color: #ff5555; }
    .sett-btn.reset {
        background: transparent;
        border-color: transparent;
        color: #666;
        margin-right: auto;
    }
    .sett-btn.reset:hover { color: #ffaa00; }
    .sett-btn.save {
        background: rgba(0, 255, 65, 0.08);
        border-color: rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
    }
    .sett-btn.save:hover {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.2);
    }
</style>
