<script>
    import { fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";

    let cacheStatus = null;
    let cacheDir = "";
    let clearing = false;
    let clearMsg = "";

    let providerSettings = null;
    let settingsEdited = false;
    let settingsSaving = false;
    let settingsMsg = "";
    let settingsError = "";
    let gatewayInput = "";

    async function loadStatus() {
        try {
            cacheStatus = await core.invoke("content_library_cache_status");
            cacheDir = await core.invoke("content_library_get_cache_dir");
        } catch {
            cacheStatus = null;
        }
    }

    async function loadProviderSettings() {
        try {
            providerSettings = await core.invoke("ipfs_get_provider_settings");
            gatewayInput = (providerSettings.gateways.viewing_gateways || []).join("\n");
        } catch {
            providerSettings = {
                selected_publish_provider: "manual",
                gateways: { viewing_gateways: [] },
            };
        }
    }

    function formatSize(bytes) {
        if (!bytes) return "0 B";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(0) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    }

    async function clearCache() {
        clearing = true;
        clearMsg = "";
        try {
            await core.invoke("content_library_clear_cache");
            clearMsg = "Cache cleared.";
            await loadStatus();
        } catch (err) {
            clearMsg = "Failed: " + err;
        }
        clearing = false;
    }

    async function openCacheFolder() {
        try {
            await core.invoke("content_library_open_cache_folder");
        } catch {}
    }

    function onGatewayInput() {
        settingsEdited = true;
    }

    async function saveSettings() {
        settingsSaving = true;
        settingsError = "";
        settingsMsg = "";
        try {
            const gateways = gatewayInput
                .split("\n")
                .map((g) => g.trim())
                .filter((g) => g.length > 0);

            const updated = {
                selected_publish_provider: providerSettings.selected_publish_provider,
                gateways: { viewing_gateways: gateways },
            };

            providerSettings = await core.invoke("ipfs_update_provider_settings", { settings: updated });
            gatewayInput = (providerSettings.gateways.viewing_gateways || []).join("\n");
            settingsEdited = false;
            settingsMsg = "Settings saved.";
        } catch (err) {
            settingsError = String(err);
        }
        settingsSaving = false;
    }

    import { onMount } from "svelte";
    onMount(() => {
        loadStatus();
        loadProviderSettings();
    });
</script>

<div class="provider-settings" in:fade={{ duration: 150 }}>
    <h3 class="panel-title">PROVIDER SETTINGS</h3>

    <div class="notice-bar">
        Configured publish providers are not enabled yet. This section allows you to select and configure IPFS providers for publishing content packages.
    </div>

    <div class="provider-grid">
        <div class="provider-card">
            <div class="provider-header">
                <span class="provider-name">Manual CID Import</span>
                <span class="provider-status available">AVAILABLE</span>
            </div>
            <div class="provider-desc">
                Import content by pasting a CID/hash. Creates a local package record for reference and library organization.
            </div>
        </div>

        <div class="provider-card disabled">
            <div class="provider-header">
                <span class="provider-name">Installed Kubo (go-ipfs)</span>
                <span class="provider-status planned">PLANNED</span>
            </div>
            <div class="provider-desc">
                Connect to a locally running Kubo/ipfs daemon for full IPFS node functionality including publishing and pinning.
            </div>
        </div>

        <div class="provider-card disabled">
            <div class="provider-header">
                <span class="provider-name">Pinata / web3.storage</span>
                <span class="provider-status planned">PLANNED</span>
            </div>
            <div class="provider-desc">
                Upload and pin content through third-party pinning services using API keys.
            </div>
        </div>
    </div>

    <div class="gateway-section">
        <h4 class="section-subtitle">VIEWING GATEWAYS</h4>
        <div class="gateway-desc">
            Public gateways used for CID fetching in the CID Viewer. One gateway per line. At least one is required.
        </div>
        <textarea
            class="gateway-textarea mono"
            bind:value={gatewayInput}
            on:input={onGatewayInput}
            rows="4"
            placeholder="https://dweb.link/ipfs/
https://ipfs.io/ipfs/"
        ></textarea>
        <div class="settings-actions">
            <button class="cyber-btn small" on:click={saveSettings} disabled={settingsSaving || !settingsEdited}>
                {settingsSaving ? "SAVING..." : "SAVE GATEWAYS"}
            </button>
            {#if settingsError}
                <span class="settings-error">{settingsError}</span>
            {/if}
            {#if settingsMsg}
                <span class="settings-msg">{settingsMsg}</span>
            {/if}
        </div>
    </div>

    {#if cacheStatus !== null}
        <div class="cache-section" in:fade={{ duration: 150 }}>
            <h4 class="section-subtitle">CONTENT CACHE</h4>
            <div class="cache-stats">
                <div class="cache-stat-row">
                    <span class="cache-stat-label">Entries</span>
                    <span class="cache-stat-value">{cacheStatus.entry_count}</span>
                </div>
                <div class="cache-stat-row">
                    <span class="cache-stat-label">Total Size</span>
                    <span class="cache-stat-value">{formatSize(cacheStatus.total_size_bytes)}</span>
                </div>
                <div class="cache-stat-row">
                    <span class="cache-stat-label">Location</span>
                    <span class="cache-stat-value mono" style="font-size:0.55rem;overflow:hidden;text-overflow:ellipsis;">{cacheDir}</span>
                </div>
            </div>
            <div class="cache-actions">
                <button class="cyber-btn ghost small" on:click={clearCache} disabled={clearing || cacheStatus.entry_count === 0}>
                    {clearing ? "CLEARING..." : "CLEAR CACHE"}
                </button>
                <button class="cyber-btn ghost small" on:click={openCacheFolder}>
                    OPEN FOLDER
                </button>
                <button class="cyber-btn ghost small" on:click={loadStatus}>REFRESH</button>
            </div>
            {#if clearMsg}
                <div class="clear-msg">{clearMsg}</div>
            {/if}
            {#if cacheStatus.entries.length > 0}
                <div class="cache-entries">
                    {#each cacheStatus.entries as entry}
                        <div class="cache-entry-row">
                            <span class="cache-entry-cid mono" title={entry.cid}>
                                {entry.cid.length > 28 ? entry.cid.slice(0, 14) + "..." + entry.cid.slice(-14) : entry.cid}
                            </span>
                            <span class="cache-entry-type">{entry.content_type}</span>
                            <span class="cache-entry-size">{formatSize(entry.size_bytes)}</span>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}

    <div class="privacy-section">
        <h4 class="section-subtitle">PRIVACY NOTES</h4>
        <ul class="privacy-list">
            <li>
                <strong>Public gateway requests</strong> reveal requested CIDs to gateway operators. Use with care.
            </li>
            <li>
                <strong>Local IPFS nodes</strong> may expose your IP address and DHT participation metadata to the IPFS network.
            </li>
            <li>
                <strong>Third-party pinning services</strong> have access to the content you upload and your usage patterns.
            </li>
            <li>
                You are responsible for the content you publish and retrieve. Commander does not censor or filter.
            </li>
        </ul>
    </div>
</div>

<style>
    .provider-settings {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1rem 1.2rem;
        max-width: 100%;
    }
    .panel-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0 0 0.8rem 0;
    }
    .notice-bar {
        padding: 0.5rem 0.75rem;
        background: rgba(255, 165, 0, 0.08);
        border: 1px solid rgba(255, 165, 0, 0.2);
        color: #cca;
        font-size: 0.65rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        line-height: 1.4;
    }
    .provider-grid {
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
        margin-bottom: 1.2rem;
    }
    .provider-card {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        padding: 0.75rem 1rem;
    }
    .provider-card.disabled {
        opacity: 0.75;
    }
    .provider-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.35rem;
    }
    .provider-name {
        font-size: 0.75rem;
        font-weight: 600;
        color: #ccc;
    }
    .provider-status {
        font-size: 0.55rem;
        padding: 2px 8px;
        border-radius: 4px;
        letter-spacing: 1px;
        font-weight: 600;
    }
    .provider-status.available {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
    }
    .provider-status.planned {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
    }
    .provider-desc {
        font-size: 0.65rem;
        color: #888;
        line-height: 1.4;
        margin-bottom: 0.25rem;
    }
    .privacy-section {
        padding: 0.8rem 0;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .section-subtitle {
        color: #666;
        font-size: 0.65rem;
        letter-spacing: 1px;
        margin: 0 0 0.6rem 0;
        text-transform: uppercase;
    }
    .gateway-section {
        padding: 0.6rem 0;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        margin-bottom: 0.8rem;
    }
    .gateway-desc {
        font-size: 0.6rem;
        color: #888;
        margin-bottom: 0.5rem;
        line-height: 1.4;
    }
    .gateway-textarea {
        width: 100%;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.5rem 0.7rem;
        border-radius: 4px;
        font-size: 0.65rem;
        outline: none;
        resize: vertical;
        box-sizing: border-box;
        line-height: 1.6;
    }
    .gateway-textarea:focus {
        border-color: var(--color-primary);
    }
    .settings-actions {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-top: 0.5rem;
        flex-wrap: wrap;
    }
    .settings-error {
        font-size: 0.55rem;
        color: #ff6666;
    }
    .settings-msg {
        font-size: 0.55rem;
        color: var(--color-primary);
    }
    .cache-section {
        padding: 0.6rem 0;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        margin-bottom: 0.8rem;
    }
    .cache-stats {
        display: flex;
        gap: 0.75rem;
        margin-bottom: 0.5rem;
    }
    .cache-stat-row {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        flex: 1;
        min-width: 0;
    }
    .cache-stat-label {
        font-size: 0.5rem;
        color: #555;
        letter-spacing: 0.5px;
    }
    .cache-stat-value {
        font-size: 0.65rem;
        color: #aaa;
    }
    .cache-actions {
        display: flex;
        gap: 0.4rem;
        margin-bottom: 0.5rem;
    }
    .clear-msg {
        font-size: 0.6rem;
        color: var(--color-primary);
        margin-bottom: 0.4rem;
    }
    .cache-entries {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
        max-height: 200px;
        overflow-y: auto;
    }
    .cache-entry-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.2rem 0.4rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.03);
        border-radius: 3px;
        font-size: 0.55rem;
    }
    .cache-entry-cid {
        color: #888;
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .cache-entry-type {
        color: #666;
        flex-shrink: 0;
    }
    .cache-entry-size {
        color: #555;
        flex-shrink: 0;
    }
    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.45rem 1rem;
        letter-spacing: 1px;
        font-weight: bold;
        font-size: 0.65rem;
        cursor: pointer;
        text-transform: uppercase;
        transition: all 0.2s;
        border-radius: 4px;
    }
    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
    }
    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .cyber-btn.ghost {
        border-color: rgba(255, 255, 255, 0.2);
        color: #aaa;
        background: transparent;
    }
    .cyber-btn.ghost:hover:not(:disabled) {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }
    .cyber-btn.small {
        padding: 0.25rem 0.65rem;
        font-size: 0.55rem;
    }
    .privacy-list {
        margin: 0;
        padding-left: 1.2rem;
        font-size: 0.6rem;
        color: #777;
        line-height: 1.6;
    }
    .privacy-list li {
        margin: 0.3rem 0;
    }
    .privacy-list li strong {
        color: #aaa;
    }
</style>
