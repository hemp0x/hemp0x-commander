<script>
    import { onMount } from "svelte";
    import { fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";

    let cacheStatus = null;
    let cacheDir = "";
    let clearing = false;
    let clearMsg = "";

    let providerSettings = {
        selected_publish_provider: "manual",
        gateways: { viewing_gateways: [] },
        pinata_api_token: "",
        pinata_api_url: "https://api.pinata.cloud",
        kubo_endpoint: "http://127.0.0.1:5001",
        filebase_token: "",
        filebase_endpoint: "https://rpc.filebase.io",
    };

    let gatewayInput = "";
    let settingsEdited = false;
    let settingsSaving = false;
    let settingsMsg = "";
    let settingsError = "";
    let testingProvider = "";
    let providerTestResults = {};

    function normalizeSettings(settings) {
        return {
            selected_publish_provider: settings?.selected_publish_provider || "manual",
            gateways: {
                viewing_gateways: settings?.gateways?.viewing_gateways || [],
            },
            pinata_api_token: settings?.pinata_api_token || "",
            pinata_api_url: settings?.pinata_api_url || "https://api.pinata.cloud",
            kubo_endpoint: settings?.kubo_endpoint || "http://127.0.0.1:5001",
            filebase_token: settings?.filebase_token || "",
            filebase_endpoint: settings?.filebase_endpoint || "https://rpc.filebase.io",
        };
    }

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
            providerSettings = normalizeSettings(await core.invoke("ipfs_get_provider_settings"));
            gatewayInput = providerSettings.gateways.viewing_gateways.join("\n");
        } catch (err) {
            settingsError = String(err);
        }
    }

    function markEdited() {
        settingsEdited = true;
        settingsMsg = "";
        settingsError = "";
    }

    function formatSize(bytes) {
        if (!bytes) return "0 B";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(0) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
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
                ...providerSettings,
                gateways: { viewing_gateways: gateways },
            };

            providerSettings = normalizeSettings(await core.invoke("ipfs_update_provider_settings", { settings: updated }));
            gatewayInput = providerSettings.gateways.viewing_gateways.join("\n");
            settingsEdited = false;
            settingsMsg = "Settings saved.";
        } catch (err) {
            settingsError = String(err);
        }
        settingsSaving = false;
    }

    async function testProvider(provider) {
        testingProvider = provider;
        providerTestResults = { ...providerTestResults, [provider]: null };
        try {
            const result = await core.invoke("ipfs_test_publish_provider", { provider });
            providerTestResults = { ...providerTestResults, [provider]: result };
        } catch (err) {
            providerTestResults = {
                ...providerTestResults,
                [provider]: { success: false, message: String(err) },
            };
        }
        testingProvider = "";
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

    onMount(() => {
        loadStatus();
        loadProviderSettings();
    });
</script>

<div class="provider-settings" in:fade={{ duration: 150 }}>
    <div class="settings-header">
        <div>
            <h3 class="panel-title">IPFS SETTINGS</h3>
            <p class="panel-subtitle">Configure publish providers, viewing gateways, and local cache.</p>
        </div>
        <button class="cyber-btn small" on:click={saveSettings} disabled={settingsSaving || !settingsEdited}>
            {settingsSaving ? "SAVING..." : "SAVE SETTINGS"}
        </button>
    </div>

    <div class="notice-bar">
        Provider tokens are stored locally in app settings for now. They will move to encrypted vault storage in a later hardening pass.
    </div>

    <div class="provider-grid">
        <section class="provider-card">
            <div class="provider-header">
                <span class="provider-name">Pinata</span>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "pinata"}>API</span>
            </div>
            <label class="form-label" for="pinata-token">JWT / API Token</label>
            <input
                id="pinata-token"
                class="form-input mono"
                type="password"
                autocomplete="off"
                bind:value={providerSettings.pinata_api_token}
                on:input={markEdited}
                placeholder="Paste Pinata JWT"
            />
            <label class="form-label" for="pinata-url">API URL</label>
            <input
                id="pinata-url"
                class="form-input mono"
                type="text"
                bind:value={providerSettings.pinata_api_url}
                on:input={markEdited}
            />
            <div class="provider-actions">
                <button class="cyber-btn ghost small" on:click={() => testProvider("pinata")} disabled={testingProvider === "pinata"}>
                    {testingProvider === "pinata" ? "TESTING..." : "TEST"}
                </button>
            </div>
            {#if providerTestResults.pinata}
                <div class:ok={providerTestResults.pinata.success} class:bad={!providerTestResults.pinata.success} class="test-result">
                    {providerTestResults.pinata.message}
                </div>
            {/if}
        </section>

        <section class="provider-card">
            <div class="provider-header">
                <span class="provider-name">Filebase</span>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "filebase"}>API</span>
            </div>
            <label class="form-label" for="filebase-token">Access Token</label>
            <input
                id="filebase-token"
                class="form-input mono"
                type="password"
                autocomplete="off"
                bind:value={providerSettings.filebase_token}
                on:input={markEdited}
                placeholder="Paste Filebase token"
            />
            <label class="form-label" for="filebase-endpoint">Endpoint</label>
            <input
                id="filebase-endpoint"
                class="form-input mono"
                type="text"
                bind:value={providerSettings.filebase_endpoint}
                on:input={markEdited}
            />
            <div class="provider-actions">
                <button class="cyber-btn ghost small" on:click={() => testProvider("filebase")} disabled={testingProvider === "filebase"}>
                    {testingProvider === "filebase" ? "TESTING..." : "TEST"}
                </button>
            </div>
            {#if providerTestResults.filebase}
                <div class:ok={providerTestResults.filebase.success} class:bad={!providerTestResults.filebase.success} class="test-result">
                    {providerTestResults.filebase.message}
                </div>
            {/if}
        </section>

        <section class="provider-card">
            <div class="provider-header">
                <span class="provider-name">Installed Kubo</span>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "installed_kubo"}>LOCAL</span>
            </div>
            <p class="provider-desc">Commander connects to a Kubo daemon you install and run separately.</p>
            <label class="form-label" for="kubo-endpoint">API Endpoint</label>
            <input
                id="kubo-endpoint"
                class="form-input mono"
                type="text"
                bind:value={providerSettings.kubo_endpoint}
                on:input={markEdited}
            />
            <div class="provider-actions">
                <button class="cyber-btn ghost small" on:click={() => testProvider("installed_kubo")} disabled={testingProvider === "installed_kubo"}>
                    {testingProvider === "installed_kubo" ? "TESTING..." : "TEST"}
                </button>
            </div>
            {#if providerTestResults.installed_kubo}
                <div class:ok={providerTestResults.installed_kubo.success} class:bad={!providerTestResults.installed_kubo.success} class="test-result">
                    {providerTestResults.installed_kubo.message}
                </div>
            {/if}
        </section>
    </div>

    <section class="gateway-section">
        <h4 class="section-subtitle">VIEWING GATEWAYS</h4>
        <div class="gateway-desc">Public gateways used for CID fetching in the CID Viewer. One gateway per line.</div>
        <textarea
            class="gateway-textarea mono"
            bind:value={gatewayInput}
            on:input={markEdited}
            rows="4"
            placeholder="https://dweb.link/ipfs/
https://ipfs.io/ipfs/"
        ></textarea>
    </section>

    {#if cacheStatus !== null}
        <section class="cache-section" in:fade={{ duration: 150 }}>
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
                <div class="cache-stat-row wide">
                    <span class="cache-stat-label">Location</span>
                    <span class="cache-stat-value mono">{cacheDir}</span>
                </div>
            </div>
            <div class="cache-actions">
                <button class="cyber-btn ghost small" on:click={clearCache} disabled={clearing || cacheStatus.entry_count === 0}>
                    {clearing ? "CLEARING..." : "CLEAR CACHE"}
                </button>
                <button class="cyber-btn ghost small" on:click={openCacheFolder}>OPEN FOLDER</button>
            </div>
            {#if clearMsg}
                <div class="clear-msg">{clearMsg}</div>
            {/if}
        </section>
    {/if}

    <section class="privacy-section">
        <h4 class="section-subtitle">SAFETY</h4>
        <ul class="privacy-list">
            <li>Only publish content you have the right to share.</li>
            <li>Published IPFS content may be public and difficult to remove.</li>
            <li>Public gateway requests reveal requested CIDs to gateway operators.</li>
            <li>Local IPFS nodes may expose your IP address and DHT participation metadata.</li>
        </ul>
    </section>

    {#if settingsError}
        <div class="settings-error">{settingsError}</div>
    {/if}
    {#if settingsMsg}
        <div class="settings-msg">{settingsMsg}</div>
    {/if}
</div>

<style>
    .provider-settings {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1rem 1.2rem;
        max-width: 100%;
        overflow: auto;
    }
    .settings-header {
        display: flex;
        justify-content: space-between;
        gap: 1rem;
        align-items: flex-start;
        margin-bottom: 0.8rem;
    }
    .panel-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0 0 0.25rem 0;
    }
    .panel-subtitle,
    .gateway-desc,
    .provider-desc {
        color: #777;
        font-size: 0.65rem;
        line-height: 1.4;
        margin: 0 0 0.65rem 0;
    }
    .notice-bar {
        padding: 0.5rem 0.75rem;
        background: rgba(255, 165, 0, 0.05);
        border: 1px solid rgba(255, 165, 0, 0.15);
        color: #bb9955;
        font-size: 0.65rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        line-height: 1.4;
    }
    .provider-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
        gap: 0.75rem;
        margin-bottom: 1rem;
    }
    .provider-card,
    .gateway-section,
    .cache-section,
    .privacy-section {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-left: 2px solid rgba(0, 255, 65, 0.16);
        border-radius: 6px;
        padding: 0.85rem 1rem;
        min-width: 0;
    }
    .provider-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.6rem;
    }
    .provider-name,
    .section-subtitle {
        font-size: 0.72rem;
        font-weight: 600;
        color: #ccc;
        letter-spacing: 1px;
        margin: 0 0 0.65rem 0;
    }
    .provider-status {
        font-size: 0.5rem;
        color: #777;
        border: 1px solid rgba(255, 255, 255, 0.12);
        padding: 2px 6px;
        border-radius: 999px;
    }
    .provider-status.active {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.35);
    }
    .form-label {
        display: block;
        font-size: 0.56rem;
        color: #666;
        letter-spacing: 1px;
        margin: 0.55rem 0 0.25rem 0;
    }
    .form-input,
    .gateway-textarea {
        width: 100%;
        box-sizing: border-box;
        background: #020604;
        border: 1px solid rgba(0, 255, 65, 0.22);
        color: #d8d8d8;
        border-radius: 4px;
        padding: 0.45rem 0.6rem;
        font-size: 0.68rem;
        outline: none;
    }
    .form-input:focus,
    .gateway-textarea:focus {
        border-color: var(--color-primary);
    }
    .provider-actions,
    .cache-actions {
        display: flex;
        gap: 0.4rem;
        flex-wrap: wrap;
        margin-top: 0.65rem;
    }
    .test-result,
    .clear-msg,
    .settings-error,
    .settings-msg {
        font-size: 0.62rem;
        margin-top: 0.5rem;
        line-height: 1.35;
    }
    .test-result.ok,
    .settings-msg,
    .clear-msg {
        color: var(--color-primary);
    }
    .test-result.bad,
    .settings-error {
        color: #ff6666;
    }
    .gateway-section,
    .cache-section,
    .privacy-section {
        margin-top: 0.8rem;
    }
    .cache-stats {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
        gap: 0.5rem;
    }
    .cache-stat-row {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
        padding: 0.5rem 0.7rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        min-width: 0;
    }
    .cache-stat-row.wide {
        grid-column: 1 / -1;
    }
    .cache-stat-label {
        font-size: 0.5rem;
        color: #555;
        letter-spacing: 0.5px;
    }
    .cache-stat-value {
        font-size: 0.68rem;
        color: #aaa;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .privacy-list {
        margin: 0;
        padding-left: 1rem;
        color: #777;
        font-size: 0.62rem;
        line-height: 1.5;
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
</style>
