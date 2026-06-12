<script>
    import { onMount } from "svelte";
    import { fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";

    let cacheStatus = null;
    let cacheDir = "";
    let clearing = false;
    let clearMsg = "";

    let vaultStatus = { exists: false, unlocked: false, has_plaintext_tokens: false, vault_path: "", info: null };
    let vaultPassphrase = "";
    let vaultUnlocking = false;
    let vaultMsg = "";
    let vaultError = "";
    let showMigrateConfirm = false;

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

    async function loadVaultStatus() {
        try {
            vaultStatus = await core.invoke("ipfs_vault_status");
        } catch {
            vaultStatus = { exists: false, unlocked: false, info: null };
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

    async function vaultUnlock() {
        vaultUnlocking = true;
        vaultError = "";
        vaultMsg = "";
        try {
            const ok = await core.invoke("ipfs_unlock_vault", { passphrase: vaultPassphrase });
            if (ok) {
                vaultStatus = { ...vaultStatus, unlocked: true };
                vaultMsg = "Vault unlocked. Provider tokens are now available for publish operations.";
                vaultPassphrase = "";
            } else {
                vaultError = "Incorrect passphrase.";
            }
        } catch (err) {
            vaultError = String(err);
        }
        vaultUnlocking = false;
    }

    async function vaultLock() {
        try {
            await core.invoke("ipfs_lock_vault");
            vaultStatus = { ...vaultStatus, unlocked: false };
            vaultMsg = "Vault locked.";
        } catch (err) {
            vaultError = String(err);
        }
    }

    async function vaultSetup() {
        if (!vaultPassphrase || vaultPassphrase.length < 8) {
            vaultError = "Passphrase must be at least 8 characters.";
            return;
        }
        if (!confirm("WARNING: If you lose this passphrase, your encrypted tokens cannot be recovered. There is no reset. Are you sure you want to create a vault?")) {
            return;
        }
        vaultUnlocking = true;
        vaultError = "";
        vaultMsg = "";
        try {
            const result = await core.invoke("vault_setup", { passphrase: vaultPassphrase });
            await core.invoke("ipfs_unlock_vault", { passphrase: vaultPassphrase });
            await loadVaultStatus();
            vaultMsg = `Vault created (${result.kdf_profile}) and unlocked for this session.`;
            vaultPassphrase = "";
            if (vaultStatus.has_plaintext_tokens) {
                vaultMsg += " Plaintext tokens detected — use Migrate to move them to the vault.";
            }
        } catch (err) {
            vaultError = String(err);
        }
        vaultUnlocking = false;
    }

    async function confirmMigrate() {
        showMigrateConfirm = true;
    }

    async function executeMigrate() {
        showMigrateConfirm = false;
        vaultUnlocking = true;
        vaultError = "";
        vaultMsg = "";
        try {
            const result = await core.invoke("ipfs_migrate_provider_tokens_to_vault", { passphrase: vaultPassphrase });
            await loadVaultStatus();
            await loadProviderSettings();
            vaultMsg = result.message;
            vaultPassphrase = "";
        } catch (err) {
            vaultError = String(err);
        }
        vaultUnlocking = false;
    }

    async function cancelMigrate() {
        showMigrateConfirm = false;
        vaultPassphrase = "";
    }

    async function migrateTokens() {
        if (!vaultPassphrase) {
            vaultError = "Enter your vault passphrase to migrate tokens.";
            return;
        }
        await confirmMigrate();
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
            modalError = String(err);
            modalItems = [];
            modalHasNext = false;
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
            modalError = String(err);
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
            modalError = String(err);
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

    onMount(() => {
        loadStatus();
        loadVaultStatus();
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
        {#if vaultStatus.exists && vaultStatus.unlocked}
            Vault unlocked. Provider tokens are read from encrypted storage.
        {:else if vaultStatus.exists}
            Vault locked. Unlock to enable publish operations.
        {:else if vaultStatus.has_plaintext_tokens}
            Plaintext provider tokens detected. Set up a vault and migrate them.
        {:else}
            No vault configured. Set up an encrypted vault to protect your API tokens.
        {/if}
    </div>

    <section class="vault-section">
        <h4 class="section-subtitle">VAULT</h4>
        {#if vaultStatus.exists}
            <div class="vault-status">
                <span class="vault-indicator" class:unlocked={vaultStatus.unlocked}>
                    {vaultStatus.unlocked ? "UNLOCKED" : "LOCKED"}
                </span>
                {#if vaultStatus.info}
                    {@const slot = vaultStatus.info.key_slots?.[0]}
                    <span class="vault-meta">
                        v{vaultStatus.info.version} | {slot?.kdf_profile ?? "?"}
                        {#if slot?.kdf_iterations} | {slot.kdf_iterations} iter{/if}
                        {#if slot?.kdf_log_n} | N=2^{slot.kdf_log_n}{/if}
                    </span>
                {/if}
            </div>
            {#if !vaultStatus.unlocked}
                <div class="vault-unlock-row">
                    <input
                        type="password"
                        class="form-input mono"
                        autocomplete="off"
                        bind:value={vaultPassphrase}
                        on:keydown={(e) => e.key === "Enter" && vaultUnlock()}
                        placeholder="Vault passphrase"
                    />
                    <button class="cyber-btn small" on:click={vaultUnlock} disabled={vaultUnlocking || !vaultPassphrase}>
                        {vaultUnlocking ? "..." : "UNLOCK"}
                    </button>
                </div>
            {:else}
                <button class="cyber-btn ghost small" on:click={vaultLock}>LOCK VAULT</button>
            {/if}
            {#if vaultStatus.vault_path}
                <div class="vault-meta" style="margin-top:0.4rem;">Path: <code>{vaultStatus.vault_path}</code></div>
            {/if}
            {#if vaultStatus.has_plaintext_tokens}
                <div class="vault-migrate-row">
                    <p class="provider-desc" style="margin:0;">Plaintext tokens still present in settings file. Migrate them to the vault.</p>
                    <button class="cyber-btn small" on:click={confirmMigrate} disabled={vaultUnlocking || !vaultStatus.unlocked}>
                        MIGRATE TOKENS
                    </button>
                </div>
            {/if}
        {:else}
            <p class="provider-desc">Create an encrypted vault to protect provider API tokens. Uses scrypt (memory-hard) with AES-256-GCM. PBKDF2-HMAC-SHA512 also supported for Core Next compatibility.</p>
            <div class="vault-unlock-row">
                <input
                    type="password"
                    class="form-input mono"
                    autocomplete="off"
                    bind:value={vaultPassphrase}
                    on:keydown={(e) => e.key === "Enter" && vaultSetup()}
                    placeholder="New vault passphrase (min 8 chars)"
                />
                <button class="cyber-btn small" on:click={vaultSetup} disabled={vaultUnlocking || !vaultPassphrase || vaultPassphrase.length < 8}>
                    {vaultUnlocking ? "..." : "CREATE VAULT"}
                </button>
            </div>
        {/if}
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
                <span class="provider-name">Pinata</span>
                <span class="provider-status" class:active={providerSettings.selected_publish_provider === "pinata"}>API</span>
            </div>
            <label class="form-label" for="pinata-token">JWT / API Token</label>
            <input
                id="pinata-token"
                class="form-input mono"
                type="password"
                autocomplete="off"
                disabled={vaultStatus.exists && !vaultStatus.unlocked}
                bind:value={providerSettings.pinata_api_token}
                on:input={markEdited}
                placeholder={vaultStatus.exists && !vaultStatus.unlocked ? "Unlock vault to edit" : "Paste Pinata JWT"}
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
                <button class="cyber-btn ghost small" on:click={() => openPublishedModal("pinata")}>PINS</button>
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
                disabled={vaultStatus.exists && !vaultStatus.unlocked}
                bind:value={providerSettings.filebase_token}
                on:input={markEdited}
                placeholder={vaultStatus.exists && !vaultStatus.unlocked ? "Unlock vault to edit" : "Paste Filebase token"}
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
                <button class="cyber-btn ghost small" on:click={() => openPublishedModal("filebase")}>PINS</button>
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

{#if showMigrateConfirm}
    <div class="modal-overlay" on:click={cancelMigrate} on:keydown={(e) => e.key === "Escape" && cancelMigrate()}>
        <div class="modal-content" on:click|stopPropagation style="max-width: 420px;">
            <h4 style="margin:0 0 0.5rem;">Migrate Tokens to Vault</h4>
            <p style="font-size:0.75rem; color:#999;">This will move your API tokens into the encrypted vault and remove them from the plaintext settings file. This action cannot be undone without re-entering your tokens. Continue?</p>
            <div style="display:flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem;">
                <button class="cyber-btn ghost small" on:click={cancelMigrate}>CANCEL</button>
                <button class="cyber-btn small" on:click={executeMigrate}>MIGRATE</button>
            </div>
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
        background: #0a0f0d;
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
        background: #0a0f0d;
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
        gap: 0.75rem;
        margin-bottom: 0.5rem;
    }
    .vault-indicator {
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 1px;
        padding: 2px 8px;
        border-radius: 4px;
        background: rgba(255, 85, 85, 0.1);
        border: 1px solid rgba(255, 85, 85, 0.3);
        color: #ff5555;
    }
    .vault-indicator.unlocked {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .vault-meta {
        font-size: 0.55rem;
        color: #666;
    }
    .vault-unlock-row {
        display: flex;
        gap: 0.4rem;
        align-items: center;
    }
    .vault-unlock-row .form-input {
        flex: 1;
        min-width: 0;
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
</style>
