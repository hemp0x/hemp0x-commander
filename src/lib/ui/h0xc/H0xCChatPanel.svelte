<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { onMount } from "svelte";
    import { fade } from "svelte/transition";
    import H0xCIdentityPicker from "./H0xCIdentityPicker.svelte";
    import H0xCChatRoom from "./H0xCChatRoom.svelte";
    import { deriveRootNameFn, isH0xCAsset } from "../../stores/h0xc.js";

    /** @typedef {{ rootName: string, assetName: string, lastSeen: number, messageCount: number }} Participant */

    const dispatch = createEventDispatcher();

    export let show = false;
    export let inline = false;

    /** @type {"loading"|"onboarding"|"identity-picker"|"chat"} */
    let view = "loading";
    let selectedIdentity = "";
    let isGuest = false;
    let ownIdentities = [];
    let identitiesLoading = false;
    let identitiesError = "";

    /** @type {Participant[]} */
    let participants = [];
    /** @type {string[]} */
    let mutedUsers = [];
    /** @type {string[]} */
    let blockedUsers = [];
    let settings = {
        messageExpiryDefault: 0,
        autoDiscovery: true,
        pollingIntervalSeconds: 30,
        autoBlockTags: ["#SPAM"],
        discoveryEnabled: true,
        muteNotifications: false,
        discoveryScanLimit: 2000,
    };
    let lastScanBlock = 0;
    let lastSeenMessageKey = "";
    let lastScanTime = "";

    const SETTINGS_KEY = "h0xc_settings";
    const PARTICIPANTS_KEY = "h0xc_cachedParticipants";
    const MUTED_KEY = "h0xc_mutedUsers";
    const BLOCKED_KEY = "h0xc_blockedUsers";
    const IDENTITY_KEY = "h0xc_selectedIdentity";
    const LAST_SEEN_KEY = "h0xc_lastSeenMessageKey";
    const LAST_SCAN_TIME_KEY = "h0xc_lastScanTime";

    function loadJson(key, fallback) {
        try {
            const raw = localStorage.getItem(key);
            if (raw) {
                const parsed = JSON.parse(raw);
                if (typeof fallback === "object" && !Array.isArray(fallback) && fallback !== null) {
                    return { ...fallback, ...parsed };
                }
                return parsed;
            }
        } catch { /* corrupt */ }
        return fallback;
    }

    function saveJson(key, value) {
        try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* quota */ }
    }

    onMount(() => {
        if (show) initialize();
    });

    $: if (show) initialize();

    let initialized = false;

    async function initialize(force = false) {
        if (!force && initialized && view !== "loading") return;
        initialized = true;

        // Load persisted state
        mutedUsers = loadJson(MUTED_KEY, []);
        blockedUsers = loadJson(BLOCKED_KEY, []);
        participants = loadJson(PARTICIPANTS_KEY, []);
        settings = loadJson(SETTINGS_KEY, {
            messageExpiryDefault: 0,
            autoDiscovery: true,
            pollingIntervalSeconds: 30,
            autoBlockTags: ["#SPAM"],
            discoveryEnabled: true,
            muteNotifications: false,
            discoveryScanLimit: 2000,
        });
        if (!Array.isArray(settings.autoBlockTags) || settings.autoBlockTags.length === 0) {
            settings.autoBlockTags = ["#SPAM"];
        }
        if (typeof settings.discoveryEnabled !== "boolean") settings.discoveryEnabled = true;
        if (typeof settings.muteNotifications !== "boolean") settings.muteNotifications = false;
        if (typeof settings.discoveryScanLimit !== "number" || settings.discoveryScanLimit < 100) settings.discoveryScanLimit = 2000;
        settings.discoveryScanLimit = Math.min(settings.discoveryScanLimit, 2000);
        lastSeenMessageKey = loadJson(LAST_SEEN_KEY, "");
        lastScanTime = loadJson(LAST_SCAN_TIME_KEY, "");
        const savedId = loadJson(IDENTITY_KEY, null);
        const savedGuest = loadJson("h0xc_isGuest", false);

        // Load identities
        identitiesLoading = true;
        identitiesError = "";
        try {
            const assets = await core.invoke("list_assets");
            ownIdentities = (Array.isArray(assets) ? assets : [])
                .filter((/** @type {{name: string}} */ a) => isH0xCAsset(a.name))
                .map((/** @type {{name: string}} */ a) => a.name);
        } catch (err) {
            identitiesError = String(err);
            ownIdentities = [];
        } finally {
            identitiesLoading = false;
        }

        if (ownIdentities.length === 0) {
            view = savedGuest ? "chat" : "onboarding";
            isGuest = savedGuest && view === "chat";
        } else if (savedId && typeof savedId === "string" && ownIdentities.includes(savedId)) {
            selectedIdentity = savedId;
            view = "chat";
        } else if (ownIdentities.length === 1) {
            selectedIdentity = ownIdentities[0];
            saveJson(IDENTITY_KEY, selectedIdentity);
            view = "chat";
        } else {
            view = "identity-picker";
        }
    }

    /** @param {CustomEvent} e */
    function handleSelectIdentity(e) {
        selectedIdentity = e.detail.identity;
        saveJson(IDENTITY_KEY, selectedIdentity);
        view = "chat";
    }

    function handleSwitchIdentity() {
        view = "identity-picker";
    }

    function handleCancelPicker() {
        if (ownIdentities.length > 0) {
            selectedIdentity = ownIdentities[0];
            saveJson(IDENTITY_KEY, selectedIdentity);
            view = "chat";
        } else {
            view = "onboarding";
        }
    }

    function handleGuestFromPicker() {
        enterAsGuest();
    }

    function handleCreateIdentity() {
        close();
        dispatch("createH0xC");
    }

    function enterAsGuest() {
        isGuest = true;
        saveJson("h0xc_isGuest", true);
        view = "chat";
    }

    function backToSetup() {
        isGuest = false;
        saveJson("h0xc_isGuest", false);
        selectedIdentity = "";
        saveJson(IDENTITY_KEY, null);
        if (ownIdentities.length > 0) {
            view = "identity-picker";
        } else {
            view = "onboarding";
        }
    }

    function close() {
        dispatch("close");
    }

    function closeAndSave() {
        saveJson(MUTED_KEY, mutedUsers);
        saveJson(BLOCKED_KEY, blockedUsers);
        saveJson(PARTICIPANTS_KEY, participants);
        saveJson(SETTINGS_KEY, settings);
        saveJson(IDENTITY_KEY, selectedIdentity);
        saveJson(LAST_SEEN_KEY, lastSeenMessageKey);
        saveJson(LAST_SCAN_TIME_KEY, lastScanTime);
        close();
    }

    function persistState() {
        saveJson(MUTED_KEY, mutedUsers);
        saveJson(BLOCKED_KEY, blockedUsers);
        saveJson(PARTICIPANTS_KEY, participants);
        saveJson(SETTINGS_KEY, settings);
        saveJson(LAST_SEEN_KEY, lastSeenMessageKey);
        saveJson(LAST_SCAN_TIME_KEY, lastScanTime);
    }

    $: if (participants) persistState();
    $: if (mutedUsers) saveJson(MUTED_KEY, mutedUsers);
    $: if (blockedUsers) saveJson(BLOCKED_KEY, blockedUsers);
    $: if (settings) saveJson(SETTINGS_KEY, settings);
    $: if (selectedIdentity) saveJson(IDENTITY_KEY, selectedIdentity);
    $: if (lastSeenMessageKey !== undefined) saveJson(LAST_SEEN_KEY, lastSeenMessageKey);
    $: if (lastScanTime !== undefined) saveJson(LAST_SCAN_TIME_KEY, lastScanTime);
</script>

{#if show}
    {#if inline}
        <div
            class="h0xc-panel h0xc-panel-inline"
            in:fade={{ duration: 120 }}
            role="document"
        >
            {#if view === "loading"}
                <div class="panel-loading">
                    <div class="panel-spinner"></div>
                    <div class="panel-loading-text">Loading H0XC Community...</div>
                </div>
            {:else if view === "onboarding"}
                <div class="panel-onboarding" transition:fade={{ duration: 100 }}>
                    <div class="onboard-header">
                        <span class="onboard-title">H0XC COMMUNITY CHAT</span>
                        <button class="onboard-close" on:click={close}>&times;</button>
                    </div>
                    <div class="onboard-body">
                        <div class="onboard-hero">
                            <span class="onboard-icon">◈</span>
                            <span class="onboard-header-text">Welcome to H0XC Community Chat</span>
                        </div>
                        <div class="onboard-desc">
                            H0XC is a decentralized community chat where messages are stored on-chain as asset messages.
                            Each participant owns a sub-asset ending in H0XC (e.g. YOURROOT/H0XC) under their root asset.
                        </div>
                        <div class="onboard-warning">
                            <span class="warn-icon">⚠</span>
                            <span class="warn-text">
                                This chat is <strong>fully public</strong>. All messages are written to the blockchain <strong>permanently</strong> and are <strong>not encrypted</strong>.
                                For end-to-end encrypted messaging, visit <a class="warn-link" href="https://hemp0x.social" target="_blank" rel="noopener">hemp0x.social</a>.
                            </span>
                        </div>
                        <div class="onboard-steps">
                            <div class="onboard-step">
                                <span class="step-num">1</span>
                                <div class="step-text">
                                    <span class="step-title">Own a root asset</span>
                                    <span class="step-desc">Create or acquire any root asset (e.g. YOURNAME) first.</span>
                                </div>
                            </div>
                            <div class="onboard-step">
                                <span class="step-num">2</span>
                                <div class="step-text">
                                    <span class="step-title">Create YOURROOT/H0XC sub-asset</span>
                                    <span class="step-desc">From the root asset's detail view, create a sub-asset named H0XC. The full name becomes YOURROOT/H0XC.</span>
                                    <code class="step-command">issue YOURROOT/H0XC 1</code>
                                </div>
                            </div>
                            <div class="onboard-step">
                                <span class="step-num">3</span>
                                <div class="step-text">
                                    <span class="step-title">Join the chat</span>
                                    <span class="step-desc">Reopen Community Chat and select your .H0XC identity to start participating.</span>
                                </div>
                            </div>
                        </div>
                        <div class="onboard-actions">
                            <button class="onboard-btn create" on:click={handleCreateIdentity}>
                                Open Asset Create Flow
                            </button>
                            <button class="onboard-btn refresh" on:click={() => initialize(true)}>
                                Refresh Identities
                            </button>
                            <button class="onboard-btn guest" on:click={enterAsGuest}>
                                Enter as Guest
                            </button>
                        </div>
                    </div>
                </div>
            {:else if view === "identity-picker"}
                <div class="h0xc-panel-inner">
                    <div class="panel-side-header">
                        <span class="psis-title">H0XC COMMUNITY CHAT</span>
                        <button class="psis-close" on:click={closeAndSave}>&times;</button>
                    </div>
                    <H0xCIdentityPicker
                        identities={ownIdentities}
                        loading={identitiesLoading}
                        error={identitiesError}
                        on:select={handleSelectIdentity}
                        on:cancel={handleCancelPicker}
                        on:create={handleCreateIdentity}
                        on:guest={handleGuestFromPicker}
                    />
                </div>
            {:else if view === "chat"}
                <div class="h0xc-panel-inner">
                        <H0xCChatRoom
                            identity={selectedIdentity}
                            {isGuest}
                            onSwitchIdentity={handleSwitchIdentity}
                            onBackToSetup={backToSetup}
                            onClose={closeAndSave}
                            on:manageTags={() => dispatch("manageTags")}
                            bind:participants
                            bind:mutedUsers
                            bind:blockedUsers
                            bind:settings
                            bind:lastScanBlock
                            bind:lastSeenMessageKey
                            bind:lastScanTime
                        />
                </div>
            {/if}
        </div>
    {:else}
        <div
            class="h0xc-panel-overlay"
            role="dialog"
            aria-modal="true"
            on:click={closeAndSave}
            on:keydown={(e) => e.key === "Escape" && closeAndSave()}
            tabindex="0"
        >
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
                class="h0xc-panel"
                in:fade={{ duration: 150 }}
                on:click|stopPropagation
                on:keydown|stopPropagation
                role="document"
            >
                {#if view === "loading"}
                    <div class="panel-loading">
                        <div class="panel-spinner"></div>
                        <div class="panel-loading-text">Loading H0XC Community...</div>
                    </div>
                {:else if view === "onboarding"}
                    <div class="panel-onboarding" transition:fade={{ duration: 120 }}>
                        <div class="onboard-header">
                            <span class="onboard-title">H0XC COMMUNITY CHAT</span>
                            <button class="onboard-close" on:click={close}>&times;</button>
                        </div>
                        <div class="onboard-body">
                            <div class="onboard-hero">
                                <span class="onboard-icon">◈</span>
                                <span class="onboard-header-text">Welcome to H0XC Community Chat</span>
                            </div>
                            <div class="onboard-desc">
                                H0XC is a decentralized community chat where messages are stored on-chain as asset messages.
                                Each participant owns a sub-asset ending in H0XC (e.g. YOURROOT/H0XC) under their root asset.
                            </div>
                            <div class="onboard-warning">
                                <span class="warn-icon">⚠</span>
                                <span class="warn-text">
                                    This chat is <strong>fully public</strong>. All messages are written to the blockchain <strong>permanently</strong> and are <strong>not encrypted</strong>.
                                    For end-to-end encrypted messaging, visit <a class="warn-link" href="https://hemp0x.social" target="_blank" rel="noopener">hemp0x.social</a>.
                                </span>
                            </div>
                            <div class="onboard-steps">
                                <div class="onboard-step">
                                    <span class="step-num">1</span>
                                    <div class="step-text">
                                        <span class="step-title">Own a root asset</span>
                                        <span class="step-desc">Create or acquire any root asset (e.g. YOURNAME) first.</span>
                                    </div>
                                </div>
                                <div class="onboard-step">
                                    <span class="step-num">2</span>
                                    <div class="step-text">
                                        <span class="step-title">Create YOURROOT/H0XC sub-asset</span>
                                        <span class="step-desc">From the root asset's detail view, create a sub-asset named H0XC. The full name becomes YOURROOT/H0XC.</span>
                                        <code class="step-command">issue YOURROOT/H0XC 1</code>
                                    </div>
                                </div>
                                <div class="onboard-step">
                                    <span class="step-num">3</span>
                                    <div class="step-text">
                                        <span class="step-title">Join the chat</span>
                                        <span class="step-desc">Reopen Community Chat and select your .H0XC identity to start participating.</span>
                                    </div>
                                </div>
                            </div>
                            <div class="onboard-actions">
                                <button class="onboard-btn create" on:click={handleCreateIdentity}>
                                    Open Asset Create Flow
                                </button>
                                <button class="onboard-btn refresh" on:click={() => initialize(true)}>
                                    Refresh Identities
                                </button>
                                <button class="onboard-btn guest" on:click={enterAsGuest}>
                                    Enter as Guest
                                </button>
                            </div>
                        </div>
                    </div>
                {:else if view === "identity-picker"}
                    <div class="h0xc-panel-inner">
                        <div class="panel-side-header">
                            <span class="psis-title">H0XC COMMUNITY CHAT</span>
                            <button class="psis-close" on:click={closeAndSave}>&times;</button>
                        </div>
                        <H0xCIdentityPicker
                            identities={ownIdentities}
                            loading={identitiesLoading}
                            error={identitiesError}
                            on:select={handleSelectIdentity}
                            on:cancel={handleCancelPicker}
                            on:create={handleCreateIdentity}
                            on:guest={handleGuestFromPicker}
                        />
                    </div>
                {:else if view === "chat"}
                    <div class="h0xc-panel-inner">
                        <H0xCChatRoom
                            identity={selectedIdentity}
                            {isGuest}
                            onSwitchIdentity={handleSwitchIdentity}
                            onBackToSetup={backToSetup}
                            onClose={closeAndSave}
                            on:manageTags={() => dispatch("manageTags")}
                            bind:participants
                            bind:mutedUsers
                            bind:blockedUsers
                            bind:settings
                            bind:lastScanBlock
                            bind:lastSeenMessageKey
                            bind:lastScanTime
                        />
                    </div>
                {/if}
            </div>
        </div>
    {/if}
{/if}

<style>
    .h0xc-panel-overlay {
        position: fixed;
        inset: 0;
        z-index: 450;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1.5rem;
        background: rgba(0, 0, 0, 0.8);
        backdrop-filter: blur(3px);
    }
    .h0xc-panel {
        width: min(52rem, 94vw);
        height: min(38rem, 85vh);
        overflow: hidden;
        display: flex;
        flex-direction: column;
        background: linear-gradient(180deg, #080b09 0%, #0c110d 100%);
        border: 1px solid rgba(0, 255, 65, 0.28);
        border-radius: 10px;
        box-shadow:
            0 0 120px rgba(0, 0, 0, 0.9),
            0 0 40px rgba(0, 255, 65, 0.12);
    }
    .h0xc-panel-inline {
        width: 100%;
        height: 100%;
        max-width: none;
        border-radius: 6px;
        border: 1px solid rgba(0, 255, 65, 0.15);
        box-shadow: none;
        background: rgba(8, 11, 9, 0.98);
        overflow: hidden;
    }
    .h0xc-panel-inner {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
    }
    .panel-loading {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex: 1;
        gap: 0.6rem;
    }
    .panel-spinner {
        width: 24px;
        height: 24px;
        border: 2px solid rgba(0, 255, 65, 0.2);
        border-top-color: var(--color-primary);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
    .panel-loading-text {
        font-size: 0.62rem;
        color: #666;
    }
    .panel-side-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.7rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
    }
    .psis-title {
        font-size: 0.65rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }
    .psis-close {
        background: none;
        border: none;
        color: #666;
        font-size: 1.2rem;
        cursor: pointer;
        line-height: 1;
    }
    .psis-close:hover { color: #fff; }

    /* Onboarding */
    .panel-onboarding {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
    }
    .onboard-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.35rem 0.6rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
        background: rgba(0, 0, 0, 0.25);
        flex-shrink: 0;
    }
    .onboard-title {
        font-size: 0.62rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }
    .onboard-close {
        background: none;
        border: none;
        color: #666;
        font-size: 1.1rem;
        cursor: pointer;
        line-height: 1;
        transition: color 0.15s;
    }
    .onboard-close:hover { color: #fff; }
    .onboard-body {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: stretch;
        padding: 0.7rem 1rem 1rem;
        overflow-y: auto;
        gap: 0.6rem;
        text-align: center;
        min-height: 0;
    }
    .onboard-hero {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
    }
    .onboard-icon {
        font-size: 1.2rem;
        color: var(--color-primary);
        opacity: 0.7;
    }
    .onboard-header-text {
        font-size: 0.88rem;
        font-weight: 700;
        color: #ddd;
        letter-spacing: 0.5px;
    }
    .onboard-desc {
        font-size: 0.7rem;
        color: #888;
        line-height: 1.55;
        padding: 0 0.5rem;
    }
    .onboard-warning {
        display: flex;
        align-items: flex-start;
        gap: 0.4rem;
        background: rgba(255, 170, 0, 0.04);
        border: 1px solid rgba(255, 170, 0, 0.12);
        border-radius: 6px;
        padding: 0.55rem 0.7rem;
        text-align: left;
    }
    .warn-icon {
        font-size: 0.75rem;
        color: #ffaa00;
        flex-shrink: 0;
        margin-top: 0.05rem;
    }
    .warn-text {
        font-size: 0.62rem;
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
    .onboard-steps {
        display: grid;
        grid-template-columns: 1fr;
        gap: 0.5rem;
        width: 100%;
        text-align: left;
        margin-top: 0.2rem;
    }
    @media (min-width: 460px) {
        .onboard-steps {
            grid-template-columns: 1fr 1fr;
        }
    }
    .onboard-step {
        display: flex;
        gap: 0.55rem;
        align-items: flex-start;
        background: rgba(0, 0, 0, 0.15);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 6px;
        padding: 0.55rem 0.7rem;
    }
    .step-num {
        width: 22px;
        height: 22px;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.62rem;
        font-weight: 700;
        flex-shrink: 0;
    }
    .step-text {
        display: flex;
        flex-direction: column;
        gap: 0.12rem;
    }
    .step-title {
        font-size: 0.68rem;
        color: #ddd;
        font-weight: 600;
        letter-spacing: 0.3px;
    }
    .step-desc {
        font-size: 0.6rem;
        color: #888;
        line-height: 1.5;
    }
    .step-command {
        display: inline-block;
        margin-top: 0.3rem;
        padding: 0.25rem 0.5rem;
        background: rgba(0, 0, 0, 0.35);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 4px;
        font-family: var(--font-mono);
        font-size: 0.58rem;
        color: #aaa;
        letter-spacing: 0.3px;
    }
    .onboard-actions {
        display: flex;
        gap: 0.45rem;
        margin-top: 0.3rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        justify-content: center;
        width: 100%;
    }
    .onboard-btn {
        padding: 0.45rem 0.8rem;
        border-radius: 5px;
        font-size: 0.62rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .onboard-btn.create {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
    }
    .onboard-btn.create:hover {
        background: var(--color-primary);
        color: #000;
    }
    .onboard-btn.refresh {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .onboard-btn.refresh:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: #aaa;
    }
    .onboard-btn.guest {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.12);
        color: #aaa;
    }
    .onboard-btn.guest:hover {
        border-color: rgba(255, 255, 255, 0.25);
        color: #fff;
    }
</style>
