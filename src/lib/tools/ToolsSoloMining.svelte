<script>
    import { onMount, onDestroy, afterUpdate } from "svelte";
    import { fly } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { systemStatus } from "../../stores.js";
    import {
        stratumStatus,
        STRATUM_POLL_INTERVAL,
        formatHashrate,
    } from "../stores/stratum.js";
    import WalletAddressPicker from "../ui/WalletAddressPicker.svelte";

    $: tauriReady = $systemStatus.tauriReady;

    let payoutMode = "wallet";
    let selectedWalletAddress = "";
    let customPayoutAddress = "";
    let bindAddress = "127.0.0.1";
    let port = 3333;
    let validationError = "";
    let customWarning = false;
    /** @type {any[]} */
    let walletAddresses = [];
    /** @type {{ label: string, address: string, scope: string }[]} */
    let bindCandidates = [];
    let loadingAddresses = false;
    let addressesError = "";
    let showChange = false;
    let rpcPortIssue = "";
    let rpcPortRepairing = false;

    /** @type {ReturnType<typeof setInterval> | undefined} */
    let pollTimer;

    // Dropdown state
    let bindDropdownOpen = false;
    /** @type {HTMLElement | undefined} */
    let bindDropdownRef;
    /** @type {HTMLElement | undefined} */
    let submissionTableRef;

    async function pollStatus() {
        if (!tauriReady) return;
        try {
            const status = await core.invoke("get_stratum_status");
            stratumStatus.set(status);
        } catch (e) {}
    }

    async function loadWalletAddresses() {
        if (!tauriReady) return;
        loadingAddresses = true;
        addressesError = "";
        try {
            walletAddresses = await core.invoke("get_receive_addresses", {
                showChange: showChange,
            });
            if (!selectedWalletAddress && walletAddresses.length > 0) {
                selectedWalletAddress = walletAddressValue(walletAddresses[0]);
            }
        } catch (e) {
            addressesError = "Could not load addresses";
        }
        loadingAddresses = false;
    }

    async function loadBindCandidates() {
        if (!tauriReady) return;
        try {
            bindCandidates = await core.invoke("get_stratum_bind_candidates");
        } catch (e) {}
    }

    function activeConfigValue(text, key) {
        const wanted = key.toLowerCase();
        for (const line of String(text || "").split(/\r?\n/)) {
            const trimmed = line.trim();
            if (!trimmed || trimmed.startsWith("#")) continue;
            const eq = trimmed.indexOf("=");
            if (eq <= 0) continue;
            if (trimmed.slice(0, eq).trim().toLowerCase() === wanted) {
                return trimmed.slice(eq + 1).trim();
            }
        }
        return "";
    }

    function setConfigValue(text, key, value) {
        const lines = String(text || "").split(/\r?\n/);
        const wanted = key.toLowerCase();
        let changed = false;
        const next = lines.map((line) => {
            const trimmed = line.trim();
            if (!trimmed || trimmed.startsWith("#")) return line;
            const eq = trimmed.indexOf("=");
            if (eq <= 0) return line;
            if (trimmed.slice(0, eq).trim().toLowerCase() !== wanted) return line;
            changed = true;
            return `${key}=${value}`;
        });
        if (!changed) {
            if (next.length > 0 && next[next.length - 1].trim() !== "") next.push("");
            next.push(`${key}=${value}`);
        }
        return next.join("\n");
    }

    async function refreshRpcPortStatus() {
        if (!tauriReady) return;
        try {
            const conf = await core.invoke("read_config");
            const rpcport = activeConfigValue(conf, "rpcport");
            if (!rpcport) {
                rpcPortIssue = "Solo mining needs rpcport=42068 in hemp.conf so Commander and Core use the same local RPC endpoint.";
            } else if (rpcport !== "42068") {
                rpcPortIssue = `Solo mining expects rpcport=42068. The active config currently uses rpcport=${rpcport}.`;
            } else {
                rpcPortIssue = "";
            }
        } catch {
            rpcPortIssue = "Commander could not read hemp.conf to verify rpcport=42068.";
        }
    }

    async function repairRpcPortAndRestart() {
        if (!tauriReady || rpcPortRepairing) return;
        rpcPortRepairing = true;
        try {
            const conf = await core.invoke("read_config");
            const updated = setConfigValue(conf, "rpcport", "42068");
            await core.invoke("write_config", { contents: updated });
            showToast("rpcport=42068 saved. Restarting Core...", "info");
            try {
                await core.invoke("stop_node");
            } catch {}
            for (let i = 0; i < 60; i++) {
                await new Promise((resolve) => setTimeout(resolve, 1000));
                try {
                    const info = await core.invoke("get_data_folder_info");
                    if (!info?.lock_exists) break;
                } catch {
                    break;
                }
            }
            await core.invoke("start_node");
            const ready = await core.invoke("wait_for_daemon_ready", { timeoutMs: 120000 });
            if (!ready?.ready) {
                throw new Error(ready?.rpc_error || "Core did not become ready after restart.");
            }
            await refreshRpcPortStatus();
            await pollStatus();
            showToast("Core restarted with rpcport=42068", "success");
        } catch (e) {
            showToast(`RPC port repair failed: ${e}`, "error");
        } finally {
            rpcPortRepairing = false;
        }
    }

    async function generateNewAddress() {
        if (!tauriReady) return;
        try {
            const address = await core.invoke("new_address", { label: "Solo Mining" });
            await loadWalletAddresses();
            selectedWalletAddress = address;
            showToast("New address created", "success");
        } catch (e) {
            showToast(`Failed: ${e}`, "error");
        }
    }

    /**
     * @param {string} addr
     */
    async function validatePayoutAddress(addr) {
        if (!tauriReady || !addr || !String(addr).trim()) return false;
        validationError = "";
        try {
            const valid = await core.invoke("validate_stratum_address", {
                address: String(addr).trim(),
            });
            if (!valid) {
                validationError = "Invalid Hemp0x address";
                return false;
            }
            return true;
        } catch (e) {
            validationError = `Validation error: ${e}`;
            return false;
        }
    }

    /**
     * @param {string | { address?: string } | null | undefined} item
     */
    function walletAddressValue(item) {
        if (typeof item === "string") return item;
        return item?.address || "";
    }

    /**
     * @param {string | undefined} address
     */
    function compactAddress(address) {
        if (!address || address.length <= 18) return address || "";
        return `${address.slice(0, 8)}...${address.slice(-6)}`;
    }

    /**
     * @param {string} value
     */
    function compactDigest(value) {
        if (!value || value.length <= 24) return value || "";
        return `${value.slice(0, 16)}...${value.slice(-8)}`;
    }

    /**
     * @param {{ label: string, scope: string }} cand
     */
    function bindCandidateLabel(cand) {
        return `${cand.label} (${cand.scope})`;
    }

    /**
     * @param {any} result
     */
    function submitResultLabel(result) {
        if (!result) return "--";
        return result;
    }

    /**
     * @param {string | undefined} status
     */
    function subStatusClass(status) {
        switch (status) {
            case "accepted": return "sub-accepted";
            case "pending": return "sub-pending";
            case "inconclusive": return "sub-inconclusive";
            case "stale_orphan": return "sub-stale-orphan";
            case "rejected":
            case "rpc_error":
            case "assembly_error":
            case "submit_error":
                return "sub-rejected";
            default: return "";
        }
    }

    /**
     * @param {string | undefined} status
     */
    function subStatusLabel(status) {
        switch (status) {
            case "accepted": return "Accepted";
            case "pending": return "Pending";
            case "inconclusive": return "Inconclusive";
            case "stale_orphan": return "Stale/Orphan";
            case "rejected": return "Rejected";
            case "rpc_error": return "RPC Error";
            case "assembly_error": return "Assembly";
            case "submit_error": return "Submit Error";
            default: return status || "--";
        }
    }

    /**
     * @param {number} ts
     */
    function formatTimestamp(ts) {
        if (!ts) return "--";
        const d = new Date(ts * 1000);
        const pad = /** @param {number} n */ (n) => String(n).padStart(2, "0");
        return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
    }

    /**
     * @param {number | undefined} hs
     */
    function workerHashrateLabel(hs) {
        return formatHashrate(hs);
    }

    /**
     * @param {string} address
     */
    function selectWalletAddress(address) {
        selectedWalletAddress = address;
        validatePayoutAddress(address);
    }

    /**
     * @param {{ address: string }} candidate
     */
    function selectBindAddress(candidate) {
        bindAddress = candidate.address;
        bindDropdownOpen = false;
    }

    /**
     * @param {string} mode
     */
    function setPayoutMode(mode) {
        payoutMode = mode;
        validationError = "";
        if (mode === "wallet" && !selectedWalletAddress && walletAddresses.length > 0) {
            selectedWalletAddress = walletAddressValue(walletAddresses[0]);
        }
    }

    function toggleShowChange() {
        showChange = !showChange;
        loadWalletAddresses();
    }

    $: activePayoutAddress = payoutMode === "wallet"
        ? selectedWalletAddress
        : customPayoutAddress;

    $: customWarning = payoutMode === "custom" && activePayoutAddress.trim().length > 0;

    $: selectedBindCandidate = bindCandidates.find(
        /** @param {{ address: string, scope: string }} c */ (c) => c.address === bindAddress,
    );
    $: lanWarning = (() => {
        const c = selectedBindCandidate;
        if (typeof c !== "object" || c === null) return false;
        return c.scope === "lan";
    })();

    async function startServer() {
        if (!tauriReady) return;
        validationError = "";
        await refreshRpcPortStatus();
        if (rpcPortIssue) {
            showToast(rpcPortIssue, "warning");
            return;
        }
        const addr = activePayoutAddress.trim();
        if (!addr) {
            showToast("Select or enter a payout address", "error");
            return;
        }
        const valid = await validatePayoutAddress(addr);
        if (!valid) {
            showToast(validationError || "Invalid payout address", "error");
            return;
        }
        try {
            const status = await core.invoke("start_stratum_server", {
                payoutAddress: addr,
                bindAddress: bindAddress,
                port: port,
            });
            stratumStatus.set(status);
            showToast("Stratum server started", "success");
        } catch (e) {
            showToast(`Start failed: ${e}`, "error");
            await pollStatus();
        }
    }

    async function stopServer() {
        if (!tauriReady) return;
        try {
            const status = await core.invoke("stop_stratum_server");
            stratumStatus.set(status);
            showToast("Stratum server stopped", "info");
        } catch (e) {
            showToast(`Stop failed: ${e}`, "error");
            await pollStatus();
        }
    }

    async function resetStats() {
        if (!tauriReady) return;
        try {
            const status = await core.invoke("reset_stratum_stats");
            stratumStatus.set(status);
            showToast("Stats reset", "info");
        } catch (e) {
            showToast(`Reset failed: ${e}`, "error");
        }
    }

    $: isRunning =
        $stratumStatus.state === "RUNNING" ||
        $stratumStatus.state === "STARTING";
    $: nodeAvailable = $stratumStatus.node_rpc_ok !== false;
    $: canStart =
        tauriReady && activePayoutAddress.trim().length > 0 && !isRunning;
    $: canStop = tauriReady && isRunning;

    $: connectionString = `stratum+tcp://${$stratumStatus.bind_address || bindAddress}:${$stratumStatus.port || port}`;
    $: displayState = isRunning && !nodeAvailable ? "DEGRADED" : $stratumStatus.state;
    $: workerFormat = activePayoutAddress.trim()
        ? `${activePayoutAddress.trim()}.worker1`
        : "<address>.worker1";
    $: workerPassword = "s";

    $: hashrateLabel = formatHashrate($stratumStatus.estimated_hashrate_hs);
    $: sharesMinLabel =
        $stratumStatus.shares_per_minute != null
            ? $stratumStatus.shares_per_minute.toFixed(1)
            : "0.0";
    $: staleCount = $stratumStatus.stale_orphan_submissions ?? 0;
    $: inconclusiveCount = $stratumStatus.inconclusive_submissions ?? 0;

    $: reversedHistory = $stratumStatus.submission_history
        ? [...$stratumStatus.submission_history].reverse()
        : [];

    let prevSubCount = 0;
    afterUpdate(() => {
        const cur = reversedHistory.length;
        if (cur > 0 && cur !== prevSubCount && submissionTableRef) {
            submissionTableRef.scrollTop = submissionTableRef.scrollHeight;
        }
        prevSubCount = cur;
    });

    // Toast
    import { createEventDispatcher } from "svelte";
    const dispatch = createEventDispatcher();
    /**
     * @param {string} msg
     * @param {"info" | "success" | "warning" | "error"} [type]
     */
    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    /**
     * @param {string} text
     */
    function copyToClipboard(text) {
        if (!text) return;
        navigator.clipboard.writeText(text).then(
            () => showToast("Copied", "success"),
            () => showToast("Copy failed", "error"),
        );
    }

    // Outside click handler for dropdowns
    /** @param {MouseEvent} event */
    function handleClickOutside(event) {
        if (bindDropdownRef && event.target instanceof Node && !bindDropdownRef.contains(event.target)) {
            bindDropdownOpen = false;
        }
    }

    // Keyboard escape to close dropdowns
    /** @param {KeyboardEvent} event */
    function handleKeydown(event) {
        if (event.key === "Escape") {
            bindDropdownOpen = false;
        }
    }

    onMount(async () => {
        pollTimer = setInterval(pollStatus, STRATUM_POLL_INTERVAL);
        await pollStatus();
        await loadWalletAddresses();
        await loadBindCandidates();
        await refreshRpcPortStatus();
        document.addEventListener("click", handleClickOutside);
        document.addEventListener("keydown", handleKeydown);
    });

    onDestroy(() => {
        if (pollTimer) clearInterval(pollTimer);
        document.removeEventListener("click", handleClickOutside);
        document.removeEventListener("keydown", handleKeydown);
    });
</script>

<div class="solo-mining-panel" in:fly={{ y: 20, duration: 300 }}>
    <!-- Header -->
    <div class="mining-header">
        <h3 class="mining-title">SOLO MINING</h3>
        <div class="header-right">
            <span
                class="status-badge"
                class:running={$stratumStatus.state === "RUNNING" && nodeAvailable}
                class:error={$stratumStatus.state === "ERROR"}
                class:starting={$stratumStatus.state === "STARTING"}
                class:degraded={isRunning && !nodeAvailable}
            >
                {displayState}
            </span>
        </div>
    </div>

    {#if isRunning}
        <p class="mining-description">
            Shares are validated locally against the connected daemon. Valid block
            candidates are submitted automatically to the daemon for inclusion in the
            blockchain.
        </p>
    {:else}
        <p class="mining-description">
            Local Stratum endpoint with miner setup guide. Rewards go to the
            address configured in your miner. The address selector is a setup
            helper only.
        </p>
    {/if}

    {#if rpcPortIssue}
        <div class="alert-panel alert-warn node-down-banner">
            <div class="alert-header">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                    <line x1="12" y1="9" x2="12" y2="13" />
                    <line x1="12" y1="17" x2="12.01" y2="17" />
                </svg>
                <span>RPC PORT REQUIRED</span>
            </div>
            <p class="alert-message">{rpcPortIssue}</p>
            <button class="repair-rpc-btn" on:click={repairRpcPortAndRestart} disabled={rpcPortRepairing}>
                {rpcPortRepairing ? "UPDATING..." : "ADD RPCPORT AND RESTART CORE"}
            </button>
        </div>
    {:else if isRunning && !nodeAvailable}
        <div class="alert-panel alert-warn node-down-banner">
            <div class="alert-header">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                    <line x1="12" y1="9" x2="12" y2="13" />
                    <line x1="12" y1="17" x2="12.01" y2="17" />
                </svg>
                <span>NODE UNAVAILABLE</span>
            </div>
            <p class="alert-message">Local node is not responding. Mining is paused. Connected miners will receive work once the node recovers.</p>
        </div>
    {:else if !isRunning && !nodeAvailable}
        <div class="alert-panel alert-warn node-down-banner">
            <div class="alert-header">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                    <line x1="12" y1="9" x2="12" y2="13" />
                    <line x1="12" y1="17" x2="12.01" y2="17" />
                </svg>
                <span>NODE OFFLINE</span>
            </div>
            <p class="alert-message">Start your local node before starting solo mining.</p>
        </div>
    {/if}

    <!-- Main layout -->
    <div class="layout-columns">
        <!-- Left column: Configuration -->
        <div class="column-left">
            <!-- Payout Section -->
            <div class="panel-section">
                <h4 class="section-label">WORKER ADDRESS</h4>

                <div class="mode-toggle">
                    <button
                        class="toggle-btn"
                        class:active={payoutMode === "wallet"}
                        on:click={() => setPayoutMode("wallet")}
                    >
                        Wallet
                    </button>
                    <button
                        class="toggle-btn"
                        class:active={payoutMode === "custom"}
                        on:click={() => setPayoutMode("custom")}
                    >
                        Custom
                    </button>
                </div>

                {#if payoutMode === "wallet"}
                    {#if loadingAddresses}
                        <span class="hint-text">Loading addresses...</span>
                    {:else if addressesError}
                        <span class="error-text">{addressesError}</span>
                    {:else if walletAddresses.length === 0}
                        <div class="empty-addresses">
                            <span class="hint-text">No wallet addresses available</span>
                            <button
                                class="small-btn"
                                on:click={generateNewAddress}
                                disabled={!tauriReady}
                            >
                                Generate Address
                            </button>
                        </div>
                    {:else}
                        <WalletAddressPicker
                            id="solo-mining-worker-address"
                            label="Payout Address"
                            bind:value={selectedWalletAddress}
                            addresses={walletAddresses}
                            nodeOnline={nodeAvailable}
                            defaultSortColumn="balance"
                            defaultSortDirection="desc"
                            on:select={(event) => selectWalletAddress(event.detail.address)}
                            on:generate={generateNewAddress}
                        />

                        <!-- Change address toggle -->
                        <div class="field-row compact">
                            <label class="field-toggle">
                                <input
                                    type="checkbox"
                                    bind:checked={showChange}
                                    on:change={toggleShowChange}
                                    disabled={loadingAddresses}
                                />
                                <span>Show change addresses</span>
                            </label>
                        </div>
                    {/if}
                {:else}
                    <input
                        type="text"
                        class="control-input mono"
                        bind:value={customPayoutAddress}
                        placeholder="Enter payout address"
                        on:input={() => { validationError = ""; }}
                        on:blur={() => validatePayoutAddress(customPayoutAddress)}
                    />
                {/if}

                {#if validationError}
                    <div class="validation-error">{validationError}</div>
                {/if}

                {#if customWarning}
                    <div class="custom-warning">
                        Block rewards go to this address via the coinbase. Commander cannot
                        verify it belongs to this wallet.
                    </div>
                {/if}
            </div>

            <!-- Bind Address Section -->
            <div class="panel-section">
                <h4 class="section-label">BIND ADDRESS</h4>
                {#if bindCandidates.length > 0}
                    <div class="dropdown" bind:this={bindDropdownRef}>
                        <button
                            type="button"
                            class="dropdown-trigger"
                            on:click={() => {
                                if (!isRunning) bindDropdownOpen = !bindDropdownOpen;
                            }}
                            disabled={isRunning}
                            aria-haspopup="listbox"
                            aria-expanded={bindDropdownOpen}
                        >
                            <span class="dropdown-trigger-text">
                                {bindCandidateLabel(selectedBindCandidate || { label: bindAddress, scope: "local" })}
                            </span>
                            <svg class="dropdown-chevron" class:open={bindDropdownOpen} width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                                <polyline points="6 9 12 15 18 9" />
                            </svg>
                        </button>
                        {#if bindDropdownOpen}
                            <div class="dropdown-menu" role="listbox">
                                {#each bindCandidates as cand}
                                    <button
                                        type="button"
                                        class="dropdown-item"
                                        class:selected={cand.address === bindAddress}
                                        role="option"
                                        aria-selected={cand.address === bindAddress}
                                        on:click={() => selectBindAddress(cand)}
                                    >
                                        <span class="dropdown-item-text">{bindCandidateLabel(cand)}</span>
                                    </button>
                                {/each}
                            </div>
                        {/if}
                    </div>
                {:else}
                    <input
                        type="text"
                        class="control-input mono"
                        bind:value={bindAddress}
                        disabled={isRunning || true}
                    />
                    <span class="hint-text">127.0.0.1 (default)</span>
                {/if}

                {#if lanWarning && isRunning}
                    <div class="custom-warning lan-warning">
                        LAN binding is active. Miners on your local network can connect.
                        Only use this on a trusted, firewalled network.
                    </div>
                {/if}
            </div>

            <!-- Port Section -->
            <div class="panel-section port-section">
                <h4 class="section-label">PORT</h4>
                <input
                    type="number"
                    class="control-input mono port-input"
                    bind:value={port}
                    min="1024"
                    max="65535"
                    disabled={isRunning}
                />
            </div>

            <!-- Controls -->
            <div class="control-buttons">
                <button
                    class="cyber-btn"
                    on:click={startServer}
                    disabled={!canStart}
                >
                    START
                </button>
                <button
                    class="cyber-btn danger"
                    on:click={stopServer}
                    disabled={!canStop}
                >
                    STOP
                </button>
                {#if isRunning}
                    <button
                        class="cyber-btn reset-btn"
                        on:click={resetStats}
                        disabled={!tauriReady}
                    >
                        RESET
                    </button>
                {/if}
            </div>

            <!-- Connection -->
            <div class="panel-section connection-section">
                <h4 class="section-label">CONNECTION</h4>
                <p class="connection-hint">
                    Set your miner to use these connection settings.
                </p>
                <div class="connection-rows">
                    <div class="conn-row">
                        <span class="conn-label">URL</span>
                        <code class="conn-value mono" title={connectionString}>{connectionString}</code>
                        <button
                            class="copy-btn"
                            on:click={() => copyToClipboard(connectionString)}
                            title="Copy Stratum URL"
                        >
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                            </svg>
                        </button>
                    </div>
                    <div class="conn-row">
                        <span class="conn-label">Worker</span>
                        <code class="conn-value mono" title={workerFormat}>{workerFormat}</code>
                        <button
                            class="copy-btn"
                            on:click={() => copyToClipboard(workerFormat)}
                            title="Copy Worker Name"
                        >
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                            </svg>
                        </button>
                    </div>
                    <div class="conn-row">
                        <span class="conn-label">Password</span>
                        <code class="conn-value mono">{workerPassword}</code>
                        <button
                            class="copy-btn"
                            on:click={() => copyToClipboard(workerPassword)}
                            title="Copy Password"
                        >
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <!-- Right column: Stats + status -->
        <div class="column-right">
            <!-- Primary Stats -->
            <div class="stats-grid-compact">
                <div class="stat-tile">
                    <span class="stat-label">Workers</span>
                    <span class="stat-value">{$stratumStatus.worker_count}</span>
                </div>
                <div class="stat-tile">
                    <span class="stat-label" title="Confirmed block submissions">Accepted</span>
                    <span class="stat-value sub-accepted">{$stratumStatus.accepted_submissions}</span>
                </div>
                <div class="stat-tile">
                    <span class="stat-label" title="Invalid or stale shares rejected by stratum server">Rejects</span>
                    <span class="stat-value rejected">{$stratumStatus.rejected_shares}</span>
                </div>
                <div class="stat-tile">
                    <span class="stat-label">Blocks</span>
                    <span class="stat-value">{$stratumStatus.blocks_found}</span>
                </div>
                <div class="stat-tile stat-tile-hashrate">
                    <span
                        class="stat-label"
                        title="Estimated from accepted shares and assigned share difficulty. Miner-reported hashrate is not trusted."
                    >
                        Est. Hashrate
                    </span>
                    <span class="stat-value">{hashrateLabel}</span>
                    <span class="stat-detail">{sharesMinLabel} shares/min</span>
                </div>
                <div class="stat-tile">
                    <span
                        class="stat-label"
                        title="Stale: {staleCount} displaced by another chain block. Review: {inconclusiveCount} unable to confirm."
                    >
                        Stale / Review
                    </span>
                    <span class="stat-value">{staleCount}&thinsp;/&thinsp;{inconclusiveCount}</span>
                </div>
            </div>
            <!-- Compact Status Chips -->
            {#if $stratumStatus.current_height != null}
                <div class="panel-section">
                    <h4 class="section-label">STATUS</h4>
                    <div class="status-chips">
                        <div class="chip">
                            <span class="chip-label">Height</span>
                            <span class="chip-value">{$stratumStatus.current_height}</span>
                        </div>
                        <div class="chip">
                            <span class="chip-label">Job</span>
                            <span class="chip-value">#{$stratumStatus.last_job_id}</span>
                        </div>
                        <div class="chip">
                            <span class="chip-label">Template</span>
                            <span class="chip-value">
                                {$stratumStatus.template_age_secs != null ? `${$stratumStatus.template_age_secs}s` : '--'}
                            </span>
                        </div>
                        <div class="chip">
                            <span class="chip-label">Candidates</span>
                            <span class="chip-value">{$stratumStatus.block_candidates}</span>
                        </div>
                    </div>
                </div>
            {/if}

            <!-- Recent Submissions -->
            {#if $stratumStatus.submission_history && $stratumStatus.submission_history.length > 0}
                <div class="panel-section submissions-section">
                    <h4 class="section-label">RECENT SUBMISSIONS</h4>
                    <div class="submission-table" bind:this={submissionTableRef}>
                        <div class="sub-row sub-header-row">
                            <span class="sub-col-time">Time</span>
                            <span class="sub-col-h">Height</span>
                            <span class="sub-col-digest">Block Digest</span>
                            <span class="sub-col-worker">Worker</span>
                            <span class="sub-col-status">Status</span>
                        </div>
                        {#each reversedHistory as sub}
                            <div class="sub-row">
                                <span class="sub-col-time mono">{formatTimestamp(sub.timestamp || 0)}</span>
                                <span class="sub-col-h mono">{sub.height ?? '--'}</span>
                                <span
                                    class="sub-col-digest mono"
                                    title={sub.digest || ''}
                                >
                                    {sub.digest ? compactDigest(sub.digest) : '--'}
                                </span>
                                <span class="sub-col-worker mono truncate" title={sub.worker_id || ''}>
                                    {sub.worker_id || '--'}
                                </span>
                                <span class="sub-col-status">
                                    <span
                                        class="sub-status-badge {subStatusClass(sub.status)}"
                                        title={sub.status === 'stale_orphan' ? (sub.result || sub.error || 'Stale or orphaned block') : ''}
                                    >
                                        {subStatusLabel(sub.status)}
                                    </span>
                                    {#if sub.result && sub.status !== 'accepted' && sub.status !== 'stale_orphan'}
                                        <span class="sub-result-text" title={sub.result}>{sub.result}</span>
                                    {:else if sub.error && sub.status !== 'stale_orphan'}
                                        <span class="sub-result-text" title={sub.error}>{sub.error}</span>
                                    {/if}
                                </span>
                            </div>
                        {/each}
                    </div>
                </div>
            {/if}

            <!-- Last Block Submission (legacy, kept for backward compat) -->
            {#if ($stratumStatus.last_submit_result || $stratumStatus.last_submitted_block_height) && (!$stratumStatus.submission_history || $stratumStatus.submission_history.length === 0)}
                <div class="panel-section">
                    <h4 class="section-label">LAST SUBMISSION</h4>
                    <div class="status-chips">
                        <div class="chip">
                            <span class="chip-label">Height</span>
                            <span class="chip-value">
                                {$stratumStatus.last_submitted_block_height ?? '--'}
                            </span>
                        </div>
                        <div class="chip">
                            <span class="chip-label">Result</span>
                            <span
                                class="chip-value"
                                class:submit-accepted={$stratumStatus.last_submit_result === "accepted"}
                            class:submit-rejected={$stratumStatus.last_submit_result &&
                                $stratumStatus.last_submit_result !== "accepted" &&
                                $stratumStatus.last_submit_result !== "inconclusive" &&
                                !$stratumStatus.last_submit_result.startsWith("RPC error")}
                            class:submit-inconclusive={$stratumStatus.last_submit_result === "inconclusive"}
                                class:submit-error={$stratumStatus.last_submit_result &&
                                    $stratumStatus.last_submit_result.startsWith("RPC error")}
                            >
                                {submitResultLabel($stratumStatus.last_submit_result)}
                            </span>
                        </div>
                        <div class="chip wide">
                            <span class="chip-label">Block Digest</span>
                            <span class="chip-value mono digest" title={$stratumStatus.last_submitted_block_digest || ''}>
                                {$stratumStatus.last_submitted_block_digest
                                    ? compactDigest($stratumStatus.last_submitted_block_digest)
                                    : '--'}
                            </span>
                        </div>
                    </div>
                </div>
            {/if}

            <!-- Warnings / Errors -->
            {#if $stratumStatus.template_error}
                <div class="alert-panel alert-warn">
                    <div class="alert-header">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                            <line x1="12" y1="9" x2="12" y2="13" />
                            <line x1="12" y1="17" x2="12.01" y2="17" />
                        </svg>
                        <span>TEMPLATE WARNING</span>
                    </div>
                    <p class="alert-message mono">{$stratumStatus.template_error}</p>
                </div>
            {/if}

            {#if $stratumStatus.last_error}
                <div class="alert-panel alert-error">
                    <div class="alert-header">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                            <circle cx="12" cy="12" r="10" />
                            <line x1="15" y1="9" x2="9" y2="15" />
                            <line x1="9" y1="9" x2="15" y2="15" />
                        </svg>
                        <span>ERROR</span>
                    </div>
                    <p class="alert-message mono">{$stratumStatus.last_error}</p>
                </div>
            {/if}
        </div>
    </div>

    <!-- Workers -->
    {#if $stratumStatus.workers.length > 0}
        <div class="worker-panel">
            <div class="worker-header-row">
                <h4 class="section-label">CONNECTED WORKERS</h4>
                <span class="worker-count">{$stratumStatus.workers.length}</span>
            </div>
            <div class="worker-table">
                <div class="worker-row worker-header">
                    <span>Worker</span>
                    <span>Payout</span>
                    <span class="numeric">Accepted</span>
                    <span class="numeric">Rejected</span>
                    <span class="numeric">Difficulty</span>
                    <span class="numeric">Hashrate</span>
                    <span class="numeric">Last Seen</span>
                </div>
                {#each $stratumStatus.workers as worker}
                    <div class="worker-row">
                        <span class="mono truncate" title={worker.worker_name}>
                            {worker.worker_name}
                        </span>
                        <span class="mono truncate" title={worker.wallet}>
                            <span class="responsive-full">{worker.wallet}</span>
                            <span class="responsive-compact">{compactAddress(worker.wallet)}</span>
                        </span>
                        <span class="numeric">{worker.accepted_shares}</span>
                        <span class="numeric rejected">{worker.rejected_shares}</span>
                        <span class="numeric">{worker.difficulty != null ? worker.difficulty.toFixed(2) : '--'}</span>
                        <span class="numeric">{workerHashrateLabel(worker.estimated_hashrate_hs)}</span>
                        <span class="numeric">
                            {worker.last_seen
                                ? `${Math.floor((Date.now() / 1000 - worker.last_seen))}s`
                                : '--'}
                        </span>
                    </div>
                {/each}
            </div>
        </div>
    {:else if isRunning}
        <div class="worker-panel empty">
            <div class="worker-header-row">
                <h4 class="section-label">CONNECTED WORKERS</h4>
            </div>
            <div class="empty-state">
                <span class="hint-text">No workers connected</span>
                <span class="hint-text sub">Configure your miner with the connection details above</span>
            </div>
        </div>
    {/if}
</div>

<style lang="css">
    .solo-mining-panel {
        display: flex;
        flex-direction: column;
        gap: 0.7rem;
        padding: 0.25rem 0;
    }

    /* Header */
    .mining-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.5rem;
    }
    .header-right {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .mining-title {
        font-size: 0.9rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0;
        font-weight: 700;
    }

    .mining-description {
        color: #777;
        font-size: 0.7rem;
        line-height: 1.5;
        margin: 0;
        padding: 0.4rem 0.6rem;
        background: rgba(0, 0, 0, 0.35);
        border-left: 2px solid var(--color-primary);
        border-radius: 0 4px 4px 0;
    }

    .status-badge {
        font-size: 0.6rem;
        padding: 0.12rem 0.45rem;
        border-radius: 3px;
        background: rgba(85, 85, 85, 0.25);
        color: #777;
        letter-spacing: 1px;
        font-family: var(--font-mono);
        font-weight: 600;
        text-transform: uppercase;
    }
    .status-badge.running {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
    }
    .status-badge.starting {
        background: rgba(255, 200, 0, 0.08);
        border: 1px solid rgba(255, 200, 0, 0.2);
        color: #ffc800;
    }
    .status-badge.error {
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.25);
        color: #ff7777;
    }
    .status-badge.degraded {
        background: rgba(255, 200, 0, 0.08);
        border: 1px solid rgba(255, 200, 0, 0.18);
        color: #ffc800;
    }

    /* Layout */
    .layout-columns {
        display: grid;
        grid-template-columns: 1fr 1.6fr;
        gap: 0.7rem;
    }
    @media (max-width: 800px) {
        .layout-columns {
            grid-template-columns: 1fr;
        }
    }
    @media (max-width: 520px) {
        .mining-header {
            flex-wrap: wrap;
            gap: 0.4rem;
        }
        .header-right {
            margin-left: auto;
        }
    }

    .column-left,
    .column-right {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .column-right {
        overflow: hidden;
        min-height: 0;
    }

    /* Panel sections */
    .panel-section {
        background: rgba(0, 0, 0, 0.28);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 5px;
        padding: 0.5rem 0.65rem;
        flex-shrink: 0;
    }

    .port-section {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .port-section .section-label {
        margin: 0;
    }

    .section-label {
        color: #555;
        font-size: 0.6rem;
        letter-spacing: 1px;
        margin: 0 0 0.4rem 0;
        text-transform: uppercase;
        font-weight: 600;
    }

    /* Mode toggle */
    .mode-toggle {
        display: flex;
        gap: 0;
        margin-bottom: 0.4rem;
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 4px;
        overflow: hidden;
    }
    .toggle-btn {
        flex: 1;
        background: transparent;
        border: none;
        color: #555;
        padding: 0.3rem 0.4rem;
        font-size: 0.65rem;
        cursor: pointer;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        transition: all 0.15s;
        font-weight: 600;
    }
    .toggle-btn.active {
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }
    .toggle-btn:disabled {
        opacity: 0.35;
        cursor: not-allowed;
    }

    /* Inputs */
    .control-input {
        background: rgba(0, 0, 0, 0.45);
        border: 1px solid rgba(0, 255, 65, 0.12);
        color: #ddd;
        padding: 0.4rem 0.55rem;
        border-radius: 4px;
        font-size: 0.78rem;
        outline: none;
        transition: border-color 0.15s;
        width: 100%;
        box-sizing: border-box;
    }
    .control-input:focus {
        border-color: var(--color-primary);
    }
    .control-input:disabled {
        opacity: 0.45;
    }
    .port-input {
        max-width: 90px;
    }

    /* Custom Dropdown */
    .dropdown {
        position: relative;
        width: 100%;
    }
    .dropdown-trigger {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.4rem;
        width: 100%;
        background: rgba(0, 0, 0, 0.45);
        border: 1px solid rgba(0, 255, 65, 0.12);
        color: #ddd;
        padding: 0.4rem 0.55rem;
        border-radius: 4px;
        font-size: 0.78rem;
        cursor: pointer;
        outline: none;
        transition: border-color 0.15s;
        box-sizing: border-box;
        text-align: left;
        text-transform: none;
    }
    .dropdown-trigger:focus,
    .dropdown-trigger:hover:not(:disabled) {
        border-color: rgba(0, 255, 65, 0.35);
    }
    .dropdown-trigger:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .dropdown-trigger-text {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        text-transform: none;
    }
    .dropdown-chevron {
        flex-shrink: 0;
        color: #555;
        transition: transform 0.15s;
    }
    .dropdown-chevron.open {
        transform: rotate(180deg);
    }

    .dropdown-menu {
        position: absolute;
        top: calc(100% + 2px);
        left: 0;
        right: 0;
        max-height: 220px;
        overflow-y: auto;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        z-index: 50;
        display: flex;
        flex-direction: column;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    }
    .dropdown-item {
        background: transparent;
        border: none;
        color: #bbb;
        padding: 0.4rem 0.55rem;
        font-size: 0.75rem;
        cursor: pointer;
        text-align: left;
        transition: all 0.1s;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        display: flex;
        align-items: center;
        gap: 0.3rem;
        text-transform: none;
    }
    .dropdown-item:last-child {
        border-bottom: none;
    }
    .dropdown-item:hover {
        background: rgba(0, 255, 65, 0.06);
        color: #fff;
    }
    .dropdown-item.selected {
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }
    .dropdown-item-text {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        text-transform: none;
    }
    .responsive-compact {
        display: none;
    }
    @media (max-width: 1280px) {
        .responsive-full {
            display: none;
        }
        .responsive-compact {
            display: inline;
        }
    }

    /* Toggle row */
    .field-row.compact {
        margin-top: 0.35rem;
    }
    .field-toggle {
        display: inline-flex;
        align-items: center;
        gap: 0.35rem;
        cursor: pointer;
        font-size: 0.68rem;
        color: #666;
        user-select: none;
    }
    .field-toggle input[type="checkbox"] {
        accent-color: var(--color-primary);
        width: 0.75rem;
        height: 0.75rem;
        cursor: pointer;
    }
    .field-toggle input[type="checkbox"]:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .field-toggle input[type="checkbox"]:disabled + span {
        opacity: 0.5;
    }

    /* Hints / errors */
    .hint-text {
        color: #555;
        font-size: 0.68rem;
    }
    .hint-text.sub {
        color: #444;
        font-size: 0.62rem;
    }
    .error-text {
        color: #ff6666;
        font-size: 0.68rem;
    }
    .validation-error {
        color: #ff6666;
        font-size: 0.68rem;
        margin-top: 0.25rem;
    }

    .custom-warning {
        margin-top: 0.35rem;
        font-size: 0.65rem;
        color: #e8a500;
        background: rgba(255, 170, 0, 0.07);
        border: 1px solid rgba(255, 170, 0, 0.18);
        padding: 0.3rem 0.45rem;
        border-radius: 3px;
        line-height: 1.4;
    }
    .lan-warning {
        color: #4da6ff;
        background: rgba(77, 166, 255, 0.07);
        border-color: rgba(77, 166, 255, 0.18);
    }

    .empty-addresses {
        display: flex;
        flex-direction: column;
        gap: 0.35rem;
    }
    .small-btn {
        background: rgba(0, 255, 65, 0.07);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
        padding: 0.25rem 0.5rem;
        border-radius: 3px;
        font-size: 0.65rem;
        cursor: pointer;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        transition: all 0.15s;
        font-weight: 600;
        align-self: flex-start;
    }
    .small-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.14);
    }
    .small-btn:disabled {
        opacity: 0.35;
        cursor: not-allowed;
    }

    /* Control buttons */
    .control-buttons {
        display: flex;
        gap: 0.4rem;
        flex-wrap: wrap;
    }

    .cyber-btn {
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.45rem 0.9rem;
        letter-spacing: 1px;
        font-weight: 700;
        transition: all 0.15s;
        cursor: pointer;
        text-transform: uppercase;
        font-size: 0.7rem;
        border-radius: 4px;
        font-family: var(--font-mono);
    }
    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.22);
    }
    .cyber-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }
    .cyber-btn.danger {
        border-color: #ff5555;
        color: #ff6666;
        background: rgba(255, 68, 68, 0.05);
    }
    .cyber-btn.danger:hover:not(:disabled) {
        background: #ff5555;
        color: #000;
        box-shadow: 0 0 10px rgba(255, 68, 68, 0.25);
    }
    .cyber-btn.reset-btn {
        border-color: #4da6ff;
        color: #4da6ff;
        background: rgba(77, 166, 255, 0.05);
        margin-left: auto;
    }
    .cyber-btn.reset-btn:hover:not(:disabled) {
        background: #4da6ff;
        color: #000;
        box-shadow: 0 0 10px rgba(77, 166, 255, 0.25);
    }

    /* Connection */
    .connection-section {
        padding-bottom: 0.55rem;
    }
    .connection-hint {
        margin: -0.15rem 0 0.4rem;
        color: #555;
        font-size: 0.64rem;
        line-height: 1.35;
    }
    .connection-rows {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .conn-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .conn-label {
        color: #4a4a4a;
        font-size: 0.62rem;
        min-width: 52px;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        font-weight: 600;
        flex-shrink: 0;
    }
    .conn-value {
        color: var(--color-primary);
        font-size: 0.75rem;
        background: rgba(0, 0, 0, 0.35);
        padding: 0.15rem 0.4rem;
        border-radius: 3px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        text-transform: none;
    }
    .copy-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: #666;
        padding: 0.12rem 0.25rem;
        border-radius: 4px;
        cursor: pointer;
        display: flex;
        align-items: center;
        transition: all 0.15s;
        flex-shrink: 0;
    }
    .copy-btn:hover {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.25);
    }

    /* Stats grid */
    .stats-grid-compact {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 0.35rem;
        flex-shrink: 0;
    }
    @media (max-width: 800px) {
        .stats-grid-compact {
            grid-template-columns: repeat(2, 1fr);
        }
    }

    .stat-tile {
        background: rgba(0, 0, 0, 0.28);
        border: 1px solid rgba(0, 255, 65, 0.08);
        border-radius: 4px;
        padding: 0.35rem 0.45rem;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.1rem;
    }
    .stat-label {
        color: #4a4a4a;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
        text-transform: uppercase;
        cursor: help;
        font-weight: 600;
    }
    .stat-value {
        color: #eee;
        font-size: 0.95rem;
        font-weight: 700;
        font-family: var(--font-mono);
    }
    .stat-value.rejected {
        color: #ff6666;
    }
    .stat-value.sub-accepted {
        color: var(--color-primary);
    }

    .stat-tile-hashrate {
        position: relative;
    }
    .stat-detail {
        font-size: 0.52rem;
        color: #555;
        font-family: var(--font-mono);
        margin-top: 0.08rem;
        line-height: 1;
    }

    /* Status chips (compact stat blocks) */
    .status-chips {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 0.3rem;
    }
    .chip {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 3px;
        padding: 0.3rem 0.4rem;
        display: flex;
        flex-direction: column;
        gap: 0.05rem;
        min-width: 0;
    }
    .chip.wide {
        grid-column: 1 / -1;
    }
    .chip-label {
        color: #444;
        font-size: 0.55rem;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        font-weight: 600;
    }
    .chip-value {
        color: #bbb;
        font-size: 0.75rem;
        font-family: var(--font-mono);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .chip-value.submit-accepted {
        color: var(--color-primary);
    }
    .chip-value.submit-rejected {
        color: #ff6666;
    }
    .chip-value.submit-inconclusive {
        color: #e8a500;
    }
    .chip-value.submit-error {
        color: #ffcc00;
    }
    .chip-value.digest {
        font-size: 0.7rem;
    }

    /* Alert panels */
    .alert-panel {
        border-radius: 4px;
        padding: 0.4rem 0.6rem;
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
        flex-shrink: 0;
    }
    .alert-warn {
        background: rgba(255, 170, 0, 0.06);
        border: 1px solid rgba(255, 170, 0, 0.18);
    }
    .alert-error {
        background: rgba(255, 68, 68, 0.06);
        border: 1px solid rgba(255, 68, 68, 0.2);
    }
    .alert-header {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        font-size: 0.6rem;
        letter-spacing: 0.5px;
        font-weight: 700;
        text-transform: uppercase;
    }
    .alert-warn .alert-header {
        color: #e8a500;
    }
    .alert-error .alert-header {
        color: #ff6666;
    }
    .alert-message {
        color: #aaa;
        font-size: 0.7rem;
        margin: 0;
        word-break: break-all;
        line-height: 1.4;
    }
    .repair-rpc-btn {
        align-self: flex-start;
        margin-top: 0.15rem;
        padding: 0.42rem 0.75rem;
        border: 1.5px solid rgba(0, 255, 65, 0.75);
        border-radius: 5px;
        background: rgba(0, 255, 65, 0.08);
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.62rem;
        font-weight: 800;
        letter-spacing: 0.9px;
        text-transform: uppercase;
        cursor: pointer;
    }
    .repair-rpc-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.14);
        box-shadow: 0 0 14px rgba(0, 255, 65, 0.18);
    }
    .repair-rpc-btn:disabled {
        opacity: 0.45;
        cursor: wait;
    }

    /* Worker panel */
    .worker-panel {
        background: rgba(0, 0, 0, 0.28);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 5px;
        padding: 0.5rem 0.65rem;
    }
    .worker-panel.empty {
        padding: 0.4rem 0.65rem;
    }
    .worker-header-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 0.35rem;
    }
    .worker-header-row .section-label {
        margin: 0;
    }
    .worker-count {
        font-size: 0.6rem;
        color: #444;
        font-family: var(--font-mono);
        font-weight: 700;
        background: rgba(0, 0, 0, 0.3);
        padding: 0.1rem 0.35rem;
        border-radius: 3px;
    }

    .worker-table {
        display: flex;
        flex-direction: column;
        gap: 1px;
    }
    .worker-row {
        display: grid;
        grid-template-columns: 0.9fr 1.2fr 0.7fr 0.7fr 0.7fr 0.7fr 0.7fr;
        gap: 0.25rem;
        padding: 0.25rem 0.35rem;
        font-size: 0.7rem;
        color: #999;
        border-radius: 2px;
        align-items: center;
    }
    .worker-row:hover {
        background: rgba(255, 255, 255, 0.03);
    }
    .worker-header {
        color: #444;
        font-size: 0.58rem;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        font-weight: 600;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        margin-bottom: 1px;
        padding-bottom: 0.3rem;
    }
    .worker-row .numeric {
        text-align: right;
    }
    .worker-row .rejected {
        color: #ff6666;
    }
    .truncate {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.15rem;
        padding: 0.5rem 0;
    }

    /* Submission history table */
    .submissions-section {
        display: flex;
        flex-direction: column;
        flex: 1 1 0;
        min-height: 0;
    }
    .submission-table {
        display: flex;
        flex-direction: column;
        gap: 1px;
        flex: 1;
        min-height: 0;
        overflow-y: auto;
    }
    .sub-row {
        display: grid;
        grid-template-columns: 0.7fr 0.5fr 1.3fr 1.2fr 1.1fr;
        gap: 0.25rem;
        padding: 0.25rem 0.35rem;
        font-size: 0.67rem;
        color: #999;
        border-radius: 2px;
        align-items: center;
    }
    .sub-row:hover {
        background: rgba(255, 255, 255, 0.03);
    }
    .sub-header-row {
        color: #444;
        font-size: 0.56rem;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        font-weight: 600;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        margin-bottom: 1px;
        padding-bottom: 0.3rem;
    }
    .sub-col-time,
    .sub-col-h {
        white-space: nowrap;
    }
    .sub-col-digest {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 0.62rem;
    }
    .sub-col-worker {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .sub-col-status {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        overflow: hidden;
    }
    .sub-status-badge {
        font-size: 0.55rem;
        padding: 0.1rem 0.3rem;
        border-radius: 2px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.3px;
        white-space: nowrap;
        flex-shrink: 0;
    }
    .sub-status-badge.sub-accepted {
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }
    .sub-status-badge.sub-rejected {
        background: rgba(255, 68, 68, 0.12);
        color: #ff6666;
    }
    .sub-status-badge.sub-pending {
        background: rgba(255, 200, 0, 0.1);
        color: #ffcc00;
    }
    .sub-status-badge.sub-inconclusive {
        background: rgba(255, 170, 0, 0.08);
        color: #e8a500;
    }
    .sub-status-badge.sub-stale-orphan {
        background: rgba(255, 68, 68, 0.08);
        color: #ff9933;
    }
    .sub-result-text {
        font-size: 0.55rem;
        color: #555;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    /* Node-down banner */
    .node-down-banner {
        margin-top: 0.2rem;
        flex-shrink: 0;
    }
    .node-down-banner .alert-message {
        font-size: 0.65rem;
    }

    /* Stale/orphan badge with tooltip hint */
    .sub-status-badge[title]:not([title=""]) {
        cursor: help;
        border-bottom: 1px dotted rgba(255, 255, 255, 0.15);
    }
</style>
