<script>
    import { onMount, onDestroy, createEventDispatcher } from "svelte";
    import { fade, fly } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { emit } from "@tauri-apps/api/event";
    import { nodeStatus, systemStatus, networkInfo } from "../../stores.js"; // Import Stores
    import ModalConfirm from "../modals/ModalConfirm.svelte";

    $: tauriReady = $systemStatus.tauriReady;
    $: isNodeOnline = $nodeStatus.online;
    $: networkMode = $networkInfo.chain || "mainnet";

    export let isVisible = false;

    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    // --- PEER PROTECTION ---
    let banList = [];
    let banListLoading = false;
    let banResult = null;
    let banningInProgress = false;
    let banListRefreshTimer;
    let autoBanIntervalId = null;
    let autoPeerProtectionEnabled = true;
    let peerSettingLoading = false;
    const autoBanInterval = 120000; // 2 minutes

    // Unban Modal State
    let showUnbanModal = false;
    let unbanTarget = "";

    let addnodeAddresses = [];
    async function loadAddnodeInfo() {
        try {
            addnodeAddresses = await core.invoke("get_addnode_hosts");
        } catch {
            addnodeAddresses = [];
        }
    }

    function isAddnodeProtected(address) {
        const normalizedBan = address.replace(/\/\d+$/, "").trim().toLowerCase();
        return addnodeAddresses.some(a => {
            const host = a.trim().replace(/\/\d+$/, "").trim().toLowerCase();
            return host === normalizedBan;
        });
    }

    async function loadBanList() {
        if (!tauriReady || !isNodeOnline) return; // Prevent offline error
        banListLoading = true;
        try {
            banList = await core.invoke("get_banned_peers");
        } catch (e) {
            if (isVisible) {
                // Only show toast if looking at it
                showToast("Failed to load ban list", "error");
            }
        }
        banListLoading = false;
    }

    async function banOldPeers(silent = false) {
        if (!tauriReady || !isNodeOnline) return;
        banningInProgress = true;
        try {
            const res = await core.invoke("ban_old_peers");
            banResult = res;
            if (!silent && res.banned_count > 0) {
                showToast(`Banned ${res.banned_count} old peers`, "success");
                loadBanList();
            } else if (!silent) {
                showToast("No new invalid or outdated peers found", "success");
            }
        } catch (e) {
            if (!silent) showToast("Peer check failed", "error");
        }
        banningInProgress = false;
    }

    async function loadPeerProtectionSetting() {
        if (!tauriReady) return;
        try {
            const settings = await core.invoke("load_app_settings");
            autoPeerProtectionEnabled = settings.auto_peer_protection_enabled !== false;
            restartAutoBanCheck();
        } catch {
            autoPeerProtectionEnabled = true;
        }
    }

    async function setPeerProtectionEnabled(value) {
        autoPeerProtectionEnabled = value;
        peerSettingLoading = true;
        try {
            const settings = await core.invoke("load_app_settings");
            settings.auto_peer_protection_enabled = value;
            await core.invoke("save_app_settings", { settings });
            restartAutoBanCheck();
            showToast(value ? "Automatic peer protection enabled" : "Automatic peer protection disabled", "info");
        } catch (e) {
            showToast("Failed to save peer protection setting", "error");
        }
        peerSettingLoading = false;
    }

    function initiateUnban(address) {
        unbanTarget = address;
        showUnbanModal = true;
    }

    async function processUnban() {
        showUnbanModal = false;
        try {
            await core.invoke("unban_peer", { address: unbanTarget });
            showToast(`Unbanned ${unbanTarget}`, "success");
            loadBanList();
        } catch (e) {
            showToast("Unban failed", "error");
        }
        unbanTarget = "";
    }

    function startAutoBanCheck() {
        if (autoBanIntervalId || !autoPeerProtectionEnabled || !tauriReady || !isNodeOnline) return;
        autoBanIntervalId = setInterval(() => {
            banOldPeers(true);
        }, autoBanInterval);
    }

    function stopAutoBanCheck() {
        if (autoBanIntervalId) {
            clearInterval(autoBanIntervalId);
            autoBanIntervalId = null;
        }
    }

    function restartAutoBanCheck() {
        stopAutoBanCheck();
        startAutoBanCheck();
    }

    // --- DIAGNOSTICS ---
    let pingHost = "hemp0x.com";
    let pingLoading = false;

    let portHost = "127.0.0.1";
    let portNum = 80;
    let portLoading = false;

    // Network Result Modal State
    let showNetworkResultModal = false;
    let networkResultTitle = "";
    let networkResultContent = "";

    function openNetworkResult(title, content) {
        networkResultTitle = title;
        networkResultContent = content;
        showNetworkResultModal = true;
    }

    async function runPing() {
        if (!tauriReady || !pingHost) return;
        pingLoading = true;
        try {
            const res = await core.invoke("execute_ping", {
                host: pingHost,
            });
            openNetworkResult(`PING: ${pingHost}`, res);
        } catch (e) {
            openNetworkResult(`PING FAILED: ${pingHost}`, `Error: ${e}`);
        }
        pingLoading = false;
    }

    async function checkPort() {
        if (!tauriReady || !portHost || !portNum) return;
        portLoading = true;
        try {
            const isOpen = await core.invoke("check_open_port", {
                host: portHost,
                port: Number(portNum),
            });
            const status = isOpen ? "OPEN / REACHABLE" : "CLOSED / BLOCKED";
            const icon = isOpen ? "✅" : "⛔";
            const desc = isOpen
                ? `Successfully connected to ${portHost}:${portNum}.\n\nIf checking localhost, the service is running.\nIf checking remote, the port is accessible.`
                : `Could not connect to ${portHost}:${portNum}.\n\n- Service might be down\n- Firewall might be blocking\n- Port might be closed`;

            openNetworkResult(`${icon} PORT ${status}`, desc);
        } catch (e) {
            openNetworkResult(
                "⚠️ PORT CHECK ERROR",
                `Failed to check port: ${e}`,
            );
        }
        portLoading = false;
    }

    // --- NETWORK MODE ---
    let pendingNetworkMode = "";
    let showNetworkModal = false;

    async function changeNetwork(mode) {
        if (!tauriReady) return;
        // Don't do anything if clicking the already active mode
        if (mode === networkMode) return;

        pendingNetworkMode = mode;
        showNetworkModal = true;
    }

    async function confirmNetworkSwitch() {
        showNetworkModal = false;
        const mode = pendingNetworkMode;

        try {
            const res = await core.invoke("set_network_mode", { mode });
            showToast(res, "success");
            networkMode = mode;

            // Emit event for global UI update
            try {
                await emit("network-changed", { mode });
            } catch (e) {
                console.error("Failed to emit network change", e);
            }

            showToast(
                "Config updated. Please restart node manually if required.",
                "info",
            );
        } catch (err) {
            showToast("Failed to switch network: " + err, "error");
        }
        pendingNetworkMode = "";
    }

    function cancelNetworkSwitch() {
        showNetworkModal = false;
        pendingNetworkMode = "";
    }

    // Reactivity
    $: if (isVisible) {
        loadBanList();
        if (banListRefreshTimer) clearInterval(banListRefreshTimer);
        banListRefreshTimer = setInterval(loadBanList, 30000); // 30s refresh
    } else {
        if (banListRefreshTimer) clearInterval(banListRefreshTimer);
    }

    $: if (autoPeerProtectionEnabled && tauriReady && isNodeOnline) {
        startAutoBanCheck();
    } else {
        stopAutoBanCheck();
    }

    onMount(() => {
        loadPeerProtectionSetting();
        loadAddnodeInfo();
    });

    onDestroy(() => {
        if (banListRefreshTimer) clearInterval(banListRefreshTimer);
        stopAutoBanCheck();
    });
</script>

<div class="tool-grid network-overhaul">
    <!-- 1. PEER PROTECTION (With Merged Ban List) -->
    <div class="update-panel">
        <h3 class="update-title">&#x1F6E1; PEER PROTECTION</h3>
        <p
            class="section-desc"
            style="margin-bottom: 1rem; color: #888; font-size: 0.8rem;"
        >
            Automatically checks <strong>unsolicited</strong> connected peers every 120 seconds. Peers
            running old Hemp0x Core builds or non-Hemp0x subversions are banned
            for 24 hours. Peers from configured <code>addnode</code> entries are protected and
            excluded from automatic bans. Core maintains its own persistent ban list.
        </p>

        <div
            class="action-row"
            style="display: flex; gap: 1rem; align-items: center; margin-bottom: 1rem;"
        >
            <label class="peer-toggle">
                <input
                    type="checkbox"
                    checked={autoPeerProtectionEnabled}
                    disabled={peerSettingLoading}
                    on:change={(e) => setPeerProtectionEnabled(e.currentTarget.checked)}
                />
                <span>AUTO PROTECT</span>
            </label>
            <button
                class="cyber-btn small"
                on:click={() => banOldPeers(false)}
                disabled={banningInProgress}
            >
                {banningInProgress ? "SCANNING..." : "🔍 CHECK NOW"}
            </button>
            <!-- Refresh logic is now automatic, button removed -->
        </div>

        {#if banResult && banResult.banned_count > 0}
            <div
                class="ban-result"
                style="margin-bottom: 1rem; padding: 0.8rem; background: rgba(0,255,65,0.05); border-radius: 8px; border: 1px solid rgba(0,255,65,0.2);"
            >
                <strong style="color: var(--color-primary);"
                    >✓ Banned {banResult.banned_count} outdated peer(s):</strong
                >
                <ul
                    style="margin: 0.5rem 0 0 1rem; font-size: 0.75rem; color: #aaa;"
                >
                    {#each banResult.banned_peers as peer}
                        <li class="mono">{peer}</li>
                    {/each}
                </ul>
            </div>
        {/if}

        <!-- BAN LIST INTEGRATED HERE -->
        <div
            class="ban-list-container"
            style="border-top: 1px solid rgba(255,255,255,0.1); padding-top: 1rem; margin-top: 0.5rem;"
        >
            <h4
                style="font-size: 0.85rem; color: #aaa; margin-bottom: 0.5rem; display: flex; justify-content: space-between;"
            >
                <span>🚫 BANNED PEERS</span>
                <span
                    style="font-size: 0.75rem; font-weight: normal; color: #666;"
                    >(Refreshes every 30s)</span
                >
            </h4>

            {#if banListLoading && banList.length === 0}
                <div
                    style="text-align: center; padding: 1rem; color: #666; font-style: italic; font-size: 0.8rem;"
                >
                    Loading list...
                </div>
            {:else if banList.length === 0}
                <div
                    style="text-align: center; padding: 1rem; color: #666; font-size: 0.8rem;"
                >
                    No banned peers
                </div>
            {:else}
                <div
                    class="ban-list-scroll"
                    style="max-height: 200px; overflow-y: auto; padding-right: 5px;"
                >
                    {#each banList as ban}
                        <div
                            class="ban-entry"
                            style="display: flex; justify-content: space-between; align-items: center; padding: 0.5rem; background: rgba(0,0,0,0.2); border-radius: 4px; margin-bottom: 0.3rem;"
                        >
                            <div style="overflow: hidden;">
                                <div
                                    class="mono"
                                    style="color: #ddd; font-size: 0.75rem;"
                                >
                                    {ban.address}
                                    {#if isAddnodeProtected(ban.address)}
                                        <span
                                            class="addnode-protected-badge"
                                            title="This IP appears in configured addnode entries"
                                        >addnode</span>
                                    {/if}
                                </div>
                                <div style="font-size: 0.65rem; color: #777;">
                                    Reason: {ban.ban_reason}
                                </div>
                            </div>
                            <button
                                class="cyber-btn small ghost"
                                on:click={() => initiateUnban(ban.address)}
                                style="font-size: 0.6rem; padding: 0.2rem 0.5rem; height: auto;"
                                >UNBAN</button
                            >
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    </div>

    <!-- 2. DIAGNOSTIC TOOLS -->
    <div class="update-panel">
        <h3 class="update-title" style="margin-bottom: 1rem;">
            🛠️ DIAGNOSTICS
        </h3>

        <!-- PING -->
        <div class="tool-row" style="margin-bottom: 1.5rem;">
            <h4
                style="font-size: 0.9rem; color: var(--color-primary); margin-bottom: 0.5rem;"
            >
                Ping Test
            </h4>
            <div style="display: flex; gap: 0.5rem;">
                <div class="input-wrapper" style="flex:1;">
                    <input
                        type="text"
                        class="input-glass"
                        bind:value={pingHost}
                        placeholder="Host"
                        on:keydown={(e) => e.key === "Enter" && runPing()}
                    />
                </div>
                <button
                    class="cyber-btn small"
                    on:click={runPing}
                    disabled={pingLoading}
                >
                    {pingLoading ? "PINGING..." : "PING"}
                </button>
            </div>
        </div>

        <!-- PORT CHECK -->
        <div class="tool-row">
            <h4
                style="font-size: 0.9rem; color: var(--color-primary); margin-bottom: 0.5rem;"
            >
                Port Checker
            </h4>
            <div style="display: flex; gap: 0.5rem;">
                <div class="input-wrapper" style="flex:2;">
                    <input
                        type="text"
                        class="input-glass"
                        bind:value={portHost}
                        placeholder="Host"
                    />
                </div>
                <div class="input-wrapper" style="flex:1;">
                    <input
                        type="number"
                        class="input-glass"
                        bind:value={portNum}
                        placeholder="Port"
                    />
                </div>
                <button
                    class="cyber-btn small"
                    on:click={checkPort}
                    disabled={portLoading}
                >
                    {portLoading ? "..." : "CHECK"}
                </button>
            </div>
        </div>
    </div>

    <!-- 3. NETWORK MODE (Moved to Bottom) -->
    <div class="update-panel">
        <div
            style="display: flex; justify-content: space-between; align-items: center;"
        >
            <div>
                <h3 class="update-title" style="margin-bottom: 0.2rem;">
                    📡 NETWORK MODE
                </h3>
                <div style="font-size: 0.75rem; color: #888;">
                    Switching requires restart
                </div>
            </div>
            <div class="network-selector" style="display: flex; gap: 0.5rem;">
                <button
                    class="cyber-btn small {networkMode === 'mainnet'
                        ? ''
                        : 'ghost'}"
                    on:click={() => changeNetwork("mainnet")}>MAINNET</button
                >
                <button
                    class="cyber-btn small {networkMode === 'regtest'
                        ? 'info'
                        : 'ghost'}"
                    on:click={() => changeNetwork("regtest")}>REGTEST</button
                >
            </div>
        </div>
    </div>
</div>

<!-- NETWORK RESULT MODAL -->
{#if showNetworkResultModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => (showNetworkResultModal = false)}
        on:keydown={(e) =>
            e.key === "Escape" && (showNetworkResultModal = false)}
    >
        <div class="modal-staged" style="width: 500px; max-width: 90vw;">
            <div class="modal-header">
                <h3
                    class="mono"
                    style="font-size: 1rem; color: var(--color-primary);"
                >
                    {networkResultTitle}
                </h3>
                <button
                    class="btn-close-x"
                    on:click={() => (showNetworkResultModal = false)}>✕</button
                >
            </div>
            <div class="modal-body" style="padding: 1rem;">
                <div
                    style="max-height: 300px; overflow-y: auto; background: rgba(0,0,0,0.3); border-radius: 4px; padding: 0.5rem;"
                >
                    <pre
                        style="white-space: pre-wrap; font-family: monospace; font-size: 0.75rem; line-height: 1.2; color: #ccc; margin: 0;">{networkResultContent}</pre>
                </div>
                <div
                    style="display: flex; justify-content: flex-end; margin-top: 1rem;"
                >
                    <button
                        class="cyber-btn"
                        on:click={() => (showNetworkResultModal = false)}
                        >CLOSE</button
                    >
                </div>
            </div>
        </div>
    </div>
{/if}

<!-- UNBAN CONFIRM MODAL -->
<ModalConfirm
    isOpen={showUnbanModal}
    type="UNBAN PEER"
    payload={{ UnbanTarget: unbanTarget }}
    on:close={() => (showUnbanModal = false)}
    on:confirm={processUnban}
/>

{#if showNetworkModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={cancelNetworkSwitch}
        on:keydown={(e) => e.key === "Escape" && cancelNetworkSwitch()}
    >
        <div class="modal-staged">
            <div class="modal-header">
                <h3>⚠️ RESTART REQUIRED</h3>
            </div>
            <div class="modal-body">
                <p>
                    Switching to <strong class="mono"
                        >{pendingNetworkMode.toUpperCase()}</strong
                    > will modify your configuration.
                </p>
                <p class="desc">
                    You must manually stop and restart the application for this
                    to take effect.
                </p>
            </div>
            <div class="modal-actions">
                <button class="cyber-btn" on:click={confirmNetworkSwitch}
                    >CONFIRM & RESTART</button
                >
                <button class="cyber-btn ghost" on:click={cancelNetworkSwitch}
                    >CANCEL</button
                >
            </div>
        </div>
    </div>
{/if}

<style>
    /* --- NETWORK TAB STYLES --- */
    .update-panel {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 8px;
        padding: 0.6rem 0.8rem; /* User Request: Compact layout */
    }
    .update-title {
        font-size: 1rem;
        color: var(--color-primary);
        margin: 0 0 1.2rem 0;
        letter-spacing: 2px;
    }
    .tool-grid.network-overhaul {
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }

    .input-wrapper {
        position: relative;
        width: 100%;
        display: flex;
        gap: 4px;
    }
    .input-glass {
        width: 100%;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #fff;
        padding: 0.6rem 0.8rem;
        font-size: 0.9rem;
        border-radius: 4px;
        outline: none;
        transition: all 0.2s;
        font-family: monospace;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
    }

    /* BUTTONS */
    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.8rem 1.5rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        font-size: 0.8rem;
        white-space: nowrap;
    }
    .cyber-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.22);
    }
    .cyber-btn.ghost {
        border-color: rgba(255, 255, 255, 0.2);
        color: #aaa;
        background: transparent;
    }
    .cyber-btn.ghost:hover {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }
    .cyber-btn.small {
        padding: 0.4rem 0.8rem;
        font-size: 0.75rem;
    }
    .btn-close-x {
        background: transparent;
        border: none;
        color: #555;
        font-size: 1.2rem;
        cursor: pointer;
        line-height: 1;
    }
    .btn-close-x:hover {
        color: #fff;
    }

    /* MODALS */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.85); /* Darker overlay */
        backdrop-filter: blur(4px);
        z-index: 10000;
        display: flex;
        justify-content: center;
        align-items: center;
        animation: toolsNetworkFadeIn 0.2s ease-out;
    }
    .modal-staged {
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
        border-radius: 8px;
        width: 400px;
        max-width: 90vw;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        animation: slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    }
    @keyframes toolsNetworkFadeIn {
        from {
            opacity: 0;
        }
        to {
            opacity: 1;
        }
    }
    .modal-header {
        background: rgba(0, 255, 65, 0.04);
        padding: 1rem 1.5rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .modal-header h3 {
        margin: 0;
        font-size: 0.95rem;
        letter-spacing: 1px;
        color: #fff;
    }
    .modal-body {
        padding: 1.5rem;
    }
    .modal-actions {
        padding: 1rem 1.5rem;
        background: rgba(0, 0, 0, 0.3);
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        display: flex;
        justify-content: flex-end;
        gap: 1rem;
    }
    .desc {
        font-size: 0.75rem;
        color: #888;
        margin: 0;
    }
    .peer-toggle {
        display: inline-flex;
        align-items: center;
        gap: 0.45rem;
        padding: 0.4rem 0.65rem;
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        color: #aaa;
        font-size: 0.68rem;
        letter-spacing: 0.08em;
        user-select: none;
    }
    .peer-toggle input {
        accent-color: var(--color-primary);
    }
    .addnode-protected-badge {
        display: inline-block;
        font-size: 0.55rem;
        padding: 0.05rem 0.3rem;
        margin-left: 0.35rem;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 3px;
        color: var(--color-primary);
        font-weight: 600;
        letter-spacing: 0.5px;
        vertical-align: middle;
    }
</style>
