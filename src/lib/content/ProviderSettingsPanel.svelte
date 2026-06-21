<script>
    import { onMount, onDestroy } from "svelte";
    import { fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { open as shellOpen } from "@tauri-apps/plugin-shell";
    import { vaultStatus } from "../../stores.js";
    import { addToolNotification } from "../stores/notifications.js";
    import eyeOpen from "../../assets/eye-open.png";
    import eyeClosed from "../../assets/eye-closed.png";

    let cacheStatus = null;
    let cacheDir = "";
    let clearing = false;
    let clearMsg = "";

    let vaultExists = false;
    let vaultUnlocked = false;
    let tokenPresence = null;
    let vaultMsg = "";
    let vaultError = "";
    let removingToken = false;

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

    let showPublishedModal = false;
    let modalProvider = "";
    let modalLoading = false;
    let modalError = "";
    let modalSuccess = "";
    let modalItems = [];
    let modalPage = 1;
    let modalHasNext = false;
    let modalQuery = "";
    let selectedCids = [];
    let unpinningCid = "";
    let unpinningBulk = false;
    let mounted = false;
    let lastVaultUnlocked = null;
    let showPinataToken = false;
    let showFilebaseToken = false;

    $: {
        vaultExists = $vaultStatus.exists;
        vaultUnlocked = $vaultStatus.unlocked;
    }

    $: if (mounted && lastVaultUnlocked !== vaultUnlocked) {
        lastVaultUnlocked = vaultUnlocked;
        refreshAfterVaultStateChange();
    }

    function normalizeSettings(settings) {
        return {
            selected_publish_provider: settings?.selected_publish_provider || "manual",
            gateways: {
                viewing_gateways: settings?.gateways?.viewing_gateways || [],
            },
            pinata_api_token: settings?.pinata_api_token === "[in vault]" ? "" : settings?.pinata_api_token || "",
            pinata_api_url: settings?.pinata_api_url || "https://api.pinata.cloud",
            kubo_endpoint: settings?.kubo_endpoint || "http://127.0.0.1:5001",
            filebase_token: settings?.filebase_token === "[in vault]" ? "" : settings?.filebase_token || "",
            filebase_endpoint: settings?.filebase_endpoint || "https://rpc.filebase.io",
        };
    }

    function friendlyError(err) {
        const text = String(err || "");
        if (/vault is locked/i.test(text) || /provider token vault is locked/i.test(text)) {
            return "Unlock your vault to use saved provider tokens.";
        }
        if (/vault does not exist/i.test(text)) {
            return "Create a vault in Tools → Wallet before saving provider tokens.";
        }
        return text;
    }

    function isVaultAccessError(err) {
        const text = String(err || "");
        return /vault is locked/i.test(text) || /provider token vault is locked/i.test(text);
    }

    async function refreshAfterVaultStateChange() {
        await loadTokenPresence();
        if (vaultUnlocked) {
            await loadProviderSettings();
        }
    }

    async function loadStatus() {
        try {
            cacheStatus = await core.invoke("content_library_cache_status");
            cacheDir = await core.invoke("content_library_get_cache_dir");
        } catch {
            cacheStatus = null;
        }
    }

    async function loadTokenPresence() {
        try {
            tokenPresence = await core.invoke("ipfs_provider_token_presence");
        } catch {
            tokenPresence = null;
        }
    }

    async function loadProviderSettings() {
        try {
            providerSettings = normalizeSettings(await core.invoke("ipfs_get_provider_settings"));
            gatewayInput = providerSettings.gateways.viewing_gateways.join("\n");
        } catch (err) {
            settingsError = friendlyError(err);
        }
    }

    function markEdited() {
        settingsEdited = true;
        settingsMsg = "";
        settingsError = "";
    }

    function notifyVaultNeedsSave(reason = "Provider token changed") {
        addToolNotification(
            "Save Hemp0x Vault",
            `${reason}. Save your Hemp0x Vault file after important changes.`,
            "warning",
            null,
            true,
        );
    }

    async function autosaveActiveVaultExport(reason = "Provider token changed") {
        try {
            const result = await core.invoke("vault_autosave_active_export_path");
            if (result?.saved) {
                return true;
            }
        } catch (err) {
            vaultError = `Vault setting saved, but portable vault autosave failed: ${err}`;
        }
        notifyVaultNeedsSave(reason);
        return false;
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
            const tokenChanged = Boolean(
                (updated.pinata_api_token && updated.pinata_api_token.trim()) ||
                (updated.filebase_token && updated.filebase_token.trim()),
            );

            providerSettings = normalizeSettings(await core.invoke("ipfs_update_provider_settings", { settings: updated }));
            gatewayInput = providerSettings.gateways.viewing_gateways.join("\n");
            settingsEdited = false;
            settingsMsg = "Settings saved.";
            await loadTokenPresence();
            if (tokenChanged) {
                await autosaveActiveVaultExport("Provider token was saved to your vault");
            }
        } catch (err) {
            settingsError = friendlyError(err);
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        settingsSaving = false;
    }

    async function requestVaultUnlock() {
        if (typeof window === "undefined") return;
        window.dispatchEvent(new CustomEvent("commander-open-vault-unlock"));
    }

    function navigateToWalletVault() {
        if (typeof window === "undefined") return;
        window.dispatchEvent(new CustomEvent("commander-open-tools-wallet"));
    }

    function providerLabel(id) {
        if (id === "pinata") return "Pinata";
        if (id === "filebase") return "Filebase";
        return id;
    }

    async function removeProviderToken(providerId) {
        removingToken = true;
        vaultMsg = "";
        vaultError = "";
        try {
            await core.invoke("ipfs_vault_remove_provider_token", { providerId });
            vaultMsg = `${providerLabel(providerId)} token removed from vault.`;
            await loadTokenPresence();
            await loadProviderSettings();
            await autosaveActiveVaultExport(`${providerLabel(providerId)} token was removed from your vault`);
        } catch (err) {
            vaultError = friendlyError(err);
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        removingToken = false;
    }

    async function testProvider(provider) {
        testingProvider = provider;
        providerTestResults = { ...providerTestResults, [provider]: null };
        try {
            const result = await core.invoke("ipfs_test_publish_provider", { provider });
            providerTestResults = { ...providerTestResults, [provider]: result };
            await loadTokenPresence();
            await loadProviderSettings();
        } catch (err) {
            providerTestResults = {
                ...providerTestResults,
                [provider]: { success: false, message: friendlyError(err) },
            };
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        testingProvider = "";
    }

    function openPublishedModal(provider) {
        modalProvider = provider;
        showPublishedModal = true;
        modalPage = 1;
        modalQuery = "";
        selectedCids = [];
        modalError = "";
        modalSuccess = "";
        loadPublishedPins();
    }

    function closePublishedModal() {
        showPublishedModal = false;
    }

    async function loadPublishedPins() {
        modalLoading = true;
        modalError = "";
        try {
            const result = await core.invoke("ipfs_list_provider_pins", {
                provider: modalProvider,
                page: modalPage,
                query: modalQuery || null,
            });
            modalItems = result.items;
            modalHasNext = result.has_next_page;
        } catch (err) {
            modalError = friendlyError(err);
            modalItems = [];
            modalHasNext = false;
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        modalLoading = false;
    }

    function toggleCid(cid) {
        if (selectedCids.includes(cid)) {
            selectedCids = selectedCids.filter((c) => c !== cid);
        } else {
            selectedCids = [...selectedCids, cid];
        }
    }

    function toggleAll() {
        if (selectedCids.length === modalItems.length && modalItems.length > 0) {
            selectedCids = [];
        } else {
            selectedCids = modalItems.map((item) => item.cid);
        }
    }

    async function unpinCid(cid) {
        unpinningCid = cid;
        modalSuccess = "";
        modalError = "";
        try {
            const result = await core.invoke("ipfs_unpin_provider_cid", {
                provider: modalProvider,
                cid,
            });
            if (result.success) {
                modalSuccess = result.message;
                const item = modalItems.find((i) => i.cid === cid);
                if (item && item.local_package_ids && item.local_package_ids.length > 0) {
                    modalSuccess += " Provider pin removed. Local package CID remains linked.";
                }
            } else {
                modalError = result.message;
            }
            selectedCids = selectedCids.filter((c) => c !== cid);
            await loadPublishedPins();
        } catch (err) {
            modalError = friendlyError(err);
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        unpinningCid = "";
    }

    async function unpinSelected() {
        if (selectedCids.length === 0) return;
        unpinningBulk = true;
        modalSuccess = "";
        modalError = "";
        try {
            const results = await core.invoke("ipfs_unpin_provider_cids", {
                provider: modalProvider,
                cids: selectedCids,
            });
            const successes = results.filter((r) => r.success);
            const failures = results.filter((r) => !r.success);
            if (successes.length > 0) {
                modalSuccess = `Unpinned ${successes.length} CID(s) from provider.`;
            }
            if (failures.length > 0) {
                modalError = `${failures.length} failed: ${failures.map((f) => f.message).join("; ")}`;
            }
            selectedCids = [];
            await loadPublishedPins();
        } catch (err) {
            modalError = friendlyError(err);
            if (isVaultAccessError(err)) requestVaultUnlock();
        }
        unpinningBulk = false;
    }

    function shortCid(cid) {
        if (cid.length <= 16) return cid;
        return cid.slice(0, 8) + "..." + cid.slice(-8);
    }

    async function copyCid(cid) {
        try {
            await navigator.clipboard.writeText(cid);
        } catch {}
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

    function openUrl(url) {
        shellOpen(url).catch(() => {});
    }

    onMount(() => {
        mounted = true;
        lastVaultUnlocked = vaultUnlocked;
        loadStatus();
        loadTokenPresence();
        loadProviderSettings();
    });

    onDestroy(() => {
        mounted = false;
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

    <section class="vault-section">
        <div class="vault-status">
            <span class="vault-label">Vault:</span>
            <span class="vault-indicator" class:unlocked={vaultUnlocked} class:locked={!vaultUnlocked}>
                {vaultUnlocked ? "Unlocked" : "Locked"}
            </span>
        </div>

        <div class="vault-status vault-status-row">
            <span class="vault-label">Provider tokens:</span>
            {#if !vaultExists}
                <span class="vault-helper">No token stored</span>
            {:else if !vaultUnlocked}
                <span class="vault-helper">Unlock vault to check saved tokens</span>
            {:else if tokenPresence?.source === "vault" &&
                (tokenPresence?.providers?.pinata?.stored || tokenPresence?.providers?.filebase?.stored)}
                <span class="vault-indicator unlocked">Stored safely</span>
            {:else}
                <span class="vault-helper">No token stored</span>
            {/if}
        </div>

        {#if !vaultExists}
            <div class="vault-info-row">
                <span class="vault-info">Create a vault in Tools → Wallet before saving provider tokens.</span>
                <button class="cyber-btn ghost small" on:click={navigateToWalletVault}>OPEN WALLET</button>
            </div>
        {:else if !vaultUnlocked}
            <div class="vault-unlock-row">
                <span class="vault-helper">Provider tokens are protected by the vault.</span>
                <button class="cyber-btn small" on:click={requestVaultUnlock}>UNLOCK VAULT</button>
            </div>
        {:else}
            <div class="vault-unlock-row">
                <span class="vault-helper-inline">Provider tokens are available for publish operations.</span>
            </div>
        {/if}

        <div class="vault-footer-note">
            Vault file management is handled in
            <button class="link-btn" on:click={navigateToWalletVault}>Tools → Wallet</button>.
        </div>

        {#if vaultMsg}
            <div class="vault-msg">{vaultMsg}</div>
        {/if}
        {#if vaultError}
            <div class="vault-error">{vaultError}</div>
        {/if}
    </section>

    <div class="provider-grid">
        <section class="provider-card">
            <div class="provider-header">
                <a href="https://pinata.cloud/" target="_blank" rel="noopener noreferrer" class="provider-link" title="https://pinata.cloud/" on:click|preventDefault={() => openUrl("https://pinata.cloud/")}>Pinata</a>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "pinata"}>API</span>
            </div>
            <label class="form-label" for="pinata-token">Provider Token</label>
            <div class="token-input-wrap">
                <input
                    id="pinata-token"
                    class="form-input mono token-input"
                    type={showPinataToken ? "text" : "password"}
                    autocomplete="off"
                    disabled={!vaultExists || !vaultUnlocked}
                    bind:value={providerSettings.pinata_api_token}
                    on:input={markEdited}
                    placeholder={!vaultExists ? "Create a vault in Tools → Wallet first" : !vaultUnlocked ? "Unlock vault to edit" : tokenPresence?.providers?.pinata?.stored ? "Stored in vault — paste new token to replace" : "Paste Pinata JWT"}
                />
                <button
                    class="token-eye-btn"
                    type="button"
                    title={showPinataToken ? "Hide token" : "Show token"}
                    aria-label={showPinataToken ? "Hide Pinata token" : "Show Pinata token"}
                    disabled={!vaultExists || !vaultUnlocked}
                    on:click={() => (showPinataToken = !showPinataToken)}
                >
                    <img src={showPinataToken ? eyeOpen : eyeClosed} alt="" class="token-eye-icon" />
                </button>
            </div>
            <label class="form-label" for="pinata-url">API URL</label>
            <input
                id="pinata-url"
                class="form-input mono"
                type="text"
                bind:value={providerSettings.pinata_api_url}
                on:input={markEdited}
            />
            <div class="provider-actions">
                <button class="cyber-btn small" on:click={saveSettings} disabled={settingsSaving || !settingsEdited || !vaultUnlocked}>
                    {settingsSaving ? "SAVING..." : "SAVE"}
                </button>
                <button class="cyber-btn ghost small" on:click={() => testProvider("pinata")} disabled={testingProvider === "pinata"}>
                    {testingProvider === "pinata" ? "TESTING..." : "TEST"}
                </button>
                <button class="cyber-btn ghost small" on:click={() => openPublishedModal("pinata")}>PINS</button>
                {#if vaultUnlocked && tokenPresence?.providers?.pinata?.stored}
                    <button class="cyber-btn ghost tiny" on:click={() => removeProviderToken("pinata")} disabled={removingToken}>
                        {removingToken ? "..." : "REMOVE"}
                    </button>
                {/if}
            </div>
            {#if providerTestResults.pinata}
                <div class:ok={providerTestResults.pinata.success} class:bad={!providerTestResults.pinata.success} class="test-result">
                    {providerTestResults.pinata.message}
                </div>
            {/if}
        </section>

        <section class="provider-card">
            <div class="provider-header">
                <a href="https://filebase.com/" target="_blank" rel="noopener noreferrer" class="provider-link" title="https://filebase.com/" on:click|preventDefault={() => openUrl("https://filebase.com/")}>Filebase</a>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "filebase"}>API</span>
            </div>
            <label class="form-label" for="filebase-token">Provider Token</label>
            <div class="token-input-wrap">
                <input
                    id="filebase-token"
                    class="form-input mono token-input"
                    type={showFilebaseToken ? "text" : "password"}
                    autocomplete="off"
                    disabled={!vaultExists || !vaultUnlocked}
                    bind:value={providerSettings.filebase_token}
                    on:input={markEdited}
                    placeholder={!vaultExists ? "Create a vault in Tools → Wallet first" : !vaultUnlocked ? "Unlock vault to edit" : tokenPresence?.providers?.filebase?.stored ? "Stored in vault — paste new token to replace" : "Paste Filebase token"}
                />
                <button
                    class="token-eye-btn"
                    type="button"
                    title={showFilebaseToken ? "Hide token" : "Show token"}
                    aria-label={showFilebaseToken ? "Hide Filebase token" : "Show Filebase token"}
                    disabled={!vaultExists || !vaultUnlocked}
                    on:click={() => (showFilebaseToken = !showFilebaseToken)}
                >
                    <img src={showFilebaseToken ? eyeOpen : eyeClosed} alt="" class="token-eye-icon" />
                </button>
            </div>
            <label class="form-label" for="filebase-endpoint">Endpoint</label>
            <input
                id="filebase-endpoint"
                class="form-input mono"
                type="text"
                bind:value={providerSettings.filebase_endpoint}
                on:input={markEdited}
            />
            <div class="provider-actions">
                <button class="cyber-btn small" on:click={saveSettings} disabled={settingsSaving || !settingsEdited || !vaultUnlocked}>
                    {settingsSaving ? "SAVING..." : "SAVE"}
                </button>
                <button class="cyber-btn ghost small" on:click={() => testProvider("filebase")} disabled={testingProvider === "filebase"}>
                    {testingProvider === "filebase" ? "TESTING..." : "TEST"}
                </button>
                <button class="cyber-btn ghost small" on:click={() => openPublishedModal("filebase")}>PINS</button>
                {#if vaultUnlocked && tokenPresence?.providers?.filebase?.stored}
                    <button class="cyber-btn ghost tiny" on:click={() => removeProviderToken("filebase")} disabled={removingToken}>
                        {removingToken ? "..." : "REMOVE"}
                    </button>
                {/if}
            </div>
            {#if providerTestResults.filebase}
                <div class:ok={providerTestResults.filebase.success} class:bad={!providerTestResults.filebase.success} class="test-result">
                    {providerTestResults.filebase.message}
                </div>
            {/if}
        </section>

        <section class="provider-card">
            <div class="provider-header">
                <a href="https://github.com/ipfs/kubo" target="_blank" rel="noopener noreferrer" class="provider-link" title="https://github.com/ipfs/kubo" on:click|preventDefault={() => openUrl("https://github.com/ipfs/kubo")}>Kubo</a>
                <span class="provider-name-sub">(Local Install)</span>
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
                <button class="cyber-btn ghost small" on:click={() => openPublishedModal("installed_kubo")}>PINS</button>
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
            <li>Review each provider's terms before use. We are not responsible for misuse of third-party services.</li>
        </ul>
    </section>

    {#if settingsError}
        <div class="settings-error">{settingsError}</div>
    {/if}
    {#if settingsMsg}
        <div class="settings-msg">{settingsMsg}</div>
    {/if}
</div>

{#if showPublishedModal}
<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={closePublishedModal}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modal-container" on:click|stopPropagation>
        <div class="modal-header">
            <h4 class="modal-title">
                Published Content — {modalProvider === "pinata" ? "Pinata" : modalProvider === "filebase" ? "Filebase" : "Installed Kubo"}
            </h4>
            <button class="cyber-btn ghost small" on:click={closePublishedModal}>CLOSE</button>
        </div>
        <div class="modal-warning">
            Unpinning removes this provider's copy only. It cannot delete content from IPFS if another node has cached or pinned it.
        </div>
        <div class="modal-toolbar">
            <input
                class="form-input mono search-input"
                type="text"
                bind:value={modalQuery}
                on:keyup={(e) => { if (e.key === 'Enter') { modalPage = 1; loadPublishedPins(); } }}
                placeholder="Search by CID or name..."
            />
            <button class="cyber-btn ghost small" on:click={() => { modalPage = 1; loadPublishedPins(); }}>SEARCH</button>
        </div>
        {#if modalLoading}
            <div class="modal-body center">
                <div class="modal-status">Loading...</div>
            </div>
        {:else if modalError && modalItems.length === 0}
            <div class="modal-body center">
                <div class="modal-status error">{modalError}</div>
            </div>
        {:else if modalItems.length === 0}
            <div class="modal-body center">
                <div class="modal-status">No pins found for this provider.</div>
            </div>
        {:else}
            <div class="modal-body">
                <table class="pins-table">
                    <thead>
                        <tr>
                            <th class="col-check"><input type="checkbox" checked={selectedCids.length === modalItems.length && modalItems.length > 0} on:change={toggleAll} /></th>
                            <th class="col-name">Name</th>
                            <th class="col-cid">CID</th>
                            <th class="col-size">Size</th>
                            <th class="col-status">Status</th>
                            <th class="col-local">Local Match</th>
                            <th class="col-actions">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each modalItems as item}
                        <tr>
                            <td class="col-check"><input type="checkbox" checked={selectedCids.includes(item.cid)} on:change={() => toggleCid(item.cid)} /></td>
                            <td class="col-name" title={item.name || ""}>{item.name || "—"}</td>
                            <td class="col-cid mono" title={item.cid}>{shortCid(item.cid)}</td>
                            <td class="col-size">{item.size_bytes != null ? formatSize(item.size_bytes) : "—"}</td>
                            <td class="col-status">{item.status || "—"}</td>
                            <td class="col-local">
                                {#if item.local_package_names && item.local_package_names.length > 0}
                                    <span class="local-badge" title={item.local_package_names.join(", ")}>Matched local package</span>
                                {:else}
                                    —
                                {/if}
                            </td>
                            <td class="col-actions">
                                <button class="cyber-btn ghost tiny" on:click={() => copyCid(item.cid)} title="Copy CID">COPY</button>
                                <button class="cyber-btn ghost tiny" on:click={() => unpinCid(item.cid)} disabled={unpinningCid === item.cid}>
                                    {unpinningCid === item.cid ? "..." : "UNPIN"}
                                </button>
                            </td>
                        </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
        <div class="modal-footer">
            <div class="pagination">
                <button class="cyber-btn ghost small" on:click={() => { modalPage -= 1; loadPublishedPins(); }} disabled={modalPage <= 1 || modalLoading}>PREVIOUS</button>
                <span class="page-info">Page {modalPage}</span>
                <button class="cyber-btn ghost small" on:click={() => { modalPage += 1; loadPublishedPins(); }} disabled={!modalHasNext || modalLoading}>NEXT</button>
            </div>
            <button class="cyber-btn ghost small" on:click={unpinSelected} disabled={selectedCids.length === 0 || unpinningBulk}>
                {unpinningBulk ? "UNPINNING..." : `UNPIN SELECTED (${selectedCids.length})`}
            </button>
        </div>
        {#if modalSuccess}
            <div class="modal-success">{modalSuccess}</div>
        {/if}
        {#if modalError && modalItems.length > 0}
            <div class="modal-error-bar">{modalError}</div>
        {/if}
    </div>
</div>
{/if}

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
    .provider-link,
    .section-subtitle {
        font-size: 0.72rem;
        font-weight: 600;
        color: #ccc;
        letter-spacing: 1px;
        margin: 0 0 0.65rem 0;
    }
    .provider-link {
        text-decoration: none;
        color: var(--color-primary);
        transition: color 0.2s;
        background: none;
        border: none;
        padding: 0;
        cursor: pointer;
        font-family: inherit;
        font-size: 0.72rem;
        font-weight: 600;
        letter-spacing: 1px;
    }
    .provider-link:hover {
        color: #fff;
        text-decoration: underline;
    }
    .token-input-wrap {
        position: relative;
        display: flex;
        align-items: center;
    }
    .token-input {
        padding-right: 2.35rem;
    }
    .token-eye-btn {
        position: absolute;
        right: 0.35rem;
        width: 1.75rem;
        height: 1.75rem;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        background: rgba(0, 0, 0, 0.38);
        cursor: pointer;
        opacity: 0.82;
    }
    .token-eye-btn:hover:not(:disabled) {
        border-color: rgba(0, 255, 65, 0.55);
        opacity: 1;
    }
    .token-eye-btn:disabled {
        cursor: not-allowed;
        opacity: 0.28;
    }
    .token-eye-icon {
        width: 0.95rem;
        height: 0.95rem;
        object-fit: contain;
        filter: brightness(0.9);
    }
    .provider-name-sub {
        font-size: 0.6rem;
        color: #666;
        font-weight: 400;
        margin-left: 0.3rem;
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
        background: rgba(0, 0, 0, 0.55);
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
        align-items: center;
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
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.22);
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
    .cyber-btn.tiny {
        padding: 0.15rem 0.4rem;
        font-size: 0.5rem;
    }
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        padding: 1rem;
    }
    .modal-container {
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        width: 100%;
        max-width: 1080px;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }
    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        flex-shrink: 0;
    }
    .modal-title {
        font-size: 0.72rem;
        font-weight: 600;
        color: #ccc;
        letter-spacing: 1px;
        margin: 0;
        text-transform: uppercase;
    }
    .modal-warning {
        padding: 0.5rem 0.75rem;
        background: rgba(255, 165, 0, 0.05);
        border-bottom: 1px solid rgba(255, 165, 0, 0.15);
        color: #bb9955;
        font-size: 0.62rem;
        line-height: 1.4;
        flex-shrink: 0;
    }
    .modal-toolbar {
        display: flex;
        gap: 0.4rem;
        padding: 0.6rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        flex-shrink: 0;
    }
    .search-input {
        flex: 1;
        min-width: 0;
    }
    .modal-body {
        flex: 1;
        overflow-y: auto;
        padding: 0 1rem;
    }
    .modal-body.center {
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: 120px;
    }
    .modal-status {
        font-size: 0.68rem;
        color: #777;
    }
    .modal-status.error {
        color: #ff6666;
    }
    .pins-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.65rem;
    }
    .pins-table th,
    .pins-table td {
        padding: 0.45rem 0.5rem;
        text-align: left;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }
    .pins-table th {
        color: #888;
        font-weight: 600;
        font-size: 0.56rem;
        letter-spacing: 0.5px;
        text-transform: uppercase;
        position: sticky;
        top: 0;
        background: rgba(2, 4, 3, 0.98);
    }
    .col-check { width: 32px; }
    .col-name { min-width: 100px; max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .col-cid { min-width: 100px; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .col-size { width: 60px; }
    .col-status { width: 70px; }
    .col-local { width: 120px; }
    .col-actions { width: 120px; white-space: nowrap; }
    .local-badge {
        display: inline-block;
        padding: 1px 6px;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        border-radius: 4px;
        font-size: 0.55rem;
    }
    .modal-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.6rem 1rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        gap: 0.5rem;
        flex-wrap: wrap;
        flex-shrink: 0;
    }
    .pagination {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .page-info {
        font-size: 0.62rem;
        color: #777;
    }
    .modal-success {
        padding: 0.4rem 1rem 0.6rem;
        color: var(--color-primary);
        font-size: 0.62rem;
        border-top: 1px solid rgba(0, 255, 65, 0.08);
        flex-shrink: 0;
    }
    .modal-error-bar {
        padding: 0.4rem 1rem 0.6rem;
        color: #ff6666;
        font-size: 0.62rem;
        border-top: 1px solid rgba(255, 102, 102, 0.1);
        flex-shrink: 0;
    }
    .vault-section {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-left: 2px solid rgba(0, 255, 65, 0.16);
        border-radius: 6px;
        padding: 0.85rem 1rem;
        margin-bottom: 0.8rem;
    }
    .vault-status {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-bottom: 0.35rem;
        font-size: 0.65rem;
        color: #aaa;
        flex-wrap: wrap;
    }
    .vault-status-row {
        margin-top: 0.25rem;
    }
    .vault-label {
        color: #888;
        letter-spacing: 0.5px;
    }
    .vault-indicator {
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        padding: 2px 8px;
        border-radius: 4px;
        background: rgba(255, 170, 0, 0.08);
        border: 1px solid rgba(255, 170, 0, 0.3);
        color: #ffaa00;
    }
    .vault-indicator.locked {
        background: rgba(255, 170, 0, 0.08);
        border: 1px solid rgba(255, 170, 0, 0.3);
        color: #ffaa00;
    }
    .vault-indicator.unlocked {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .vault-helper,
    .vault-helper-inline {
        font-size: 0.6rem;
        color: #888;
    }
    .vault-helper-inline {
        margin-right: 0.4rem;
    }
    .vault-info-row {
        display: flex;
        gap: 0.6rem;
        align-items: center;
        flex-wrap: wrap;
        margin-top: 0.4rem;
    }
    .vault-info {
        font-size: 0.6rem;
        color: #bb9955;
    }
    .vault-unlock-row {
        display: flex;
        gap: 0.4rem;
        align-items: center;
        flex-wrap: wrap;
    }
    .vault-msg {
        font-size: 0.62rem;
        color: var(--color-primary);
        margin-top: 0.5rem;
    }
    .vault-error {
        font-size: 0.62rem;
        color: #ff6666;
        margin-top: 0.5rem;
    }
    .vault-footer-note {
        font-size: 0.58rem;
        color: #888;
        margin-top: 0.6rem;
    }
    .link-btn {
        background: none;
        border: none;
        color: var(--color-primary);
        cursor: pointer;
        padding: 0;
        font-size: inherit;
        text-decoration: underline;
        font-family: inherit;
    }
    .link-btn:hover {
        color: #fff;
    }
</style>
