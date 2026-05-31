<script>
    import { onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { formatBalance } from "./utils.js";
    import { nodeStatus } from "../stores.js"; // Import Store

    let label = "";
    let addresses = [];
    let status = "";
    let tauriReady = false;
    let showChange = false;
    let isExpanded = false; // EXPANSION STATE

    $: isNodeOnline = $nodeStatus.online;

    async function refreshList() {
        if (!tauriReady) {
            status = "Tauri backend not available.";

            return;
        }
        try {
            addresses = await core.invoke("get_receive_addresses", {
                showChange,
            });
            status = "";
        } catch (err) {
            // Show friendly message for connection errors
            const errStr = String(err || "");
            if (
                errStr.includes("couldn't connect") ||
                errStr.includes("EOF reached") ||
                errStr.includes("RPC")
            ) {
                status = "Node not connected - Start node to load addresses";
            } else {
                status = `Error: ${err}`;
            }
        }
    }

    async function generateAddr() {
        if (!tauriReady) {
            status = "Tauri backend not available.";
            return;
        }
        try {
            const addr = await core.invoke("new_address", { label });
            status = `Created: ${addr}`;
            label = "";
            await refreshList();
        } catch (err) {
            status = `Error: ${err}`;
        }
    }

    async function copyAddr(addr) {
        try {
            await navigator.clipboard.writeText(addr);
            status = "Copied address.";
        } catch {
            status = "Copy failed.";
        }
    }

    function toggleExpand() {
        isExpanded = !isExpanded;
    }

    onMount(() => {
        tauriReady =
            typeof core?.isTauri === "function" ? core.isTauri() : false;

        if (isNodeOnline) {
            refreshList();
        } else {
            status = "Node offline - Start node to load addresses";
        }
    });

    // React to online status changes
    $: if (tauriReady) {
        if (isNodeOnline) {
            refreshList();
        } else {
            addresses = [];
            status = "Node offline - Start node to load addresses";
        }
    }

    // Throttled auto-refresh on balance/tx changes (not per-poll)
    import { walletInfo } from "../stores.js";
    let lastAddressRefresh = 0;
    $: if ($walletInfo && isNodeOnline && Date.now() - lastAddressRefresh > 15000) {
        lastAddressRefresh = Date.now();
        refreshList();
    }

    // --- SORTING ---
    let sortColumn = "label"; // 'label', 'address', 'balance'
    let sortDirection = "asc"; // 'asc', 'desc'

    function toggleSort(col) {
        if (sortColumn === col) {
            sortDirection = sortDirection === "asc" ? "desc" : "asc";
        } else {
            sortColumn = col;
            sortDirection = "asc";
        }
    }

    $: sortedAddresses = [...addresses].sort((a, b) => {
        let valA = a[sortColumn];
        let valB = b[sortColumn];

        if (sortColumn === "balance") {
            // Handle numeric balance sorting
            valA = Number(valA || 0);
            valB = Number(valB || 0);
        } else {
            // Handle text sorting
            valA = (valA || "").toString().toLowerCase();
            valB = (valB || "").toString().toLowerCase();
        }

        if (valA < valB) return sortDirection === "asc" ? -1 : 1;
        if (valA > valB) return sortDirection === "asc" ? 1 : -1;
        return 0;
    });
</script>

<div class="view-receive">
    <!-- TOP: NEW ADDRESS -->
    <div class="glass-panel panel-strong gen-area cyber-panel">
        <header class="panel-header">
            <span class="hud-title mono">[ GENERATE ADDRESS ]</span>
            <span class="hint mono">CREATE NEW RECEIVING IDENTIFIER</span>
        </header>

        <div class="gen-body">
            <div class="input-group">
                <label for="new-label" class="field-label"
                    >LABEL (OPTIONAL)</label
                >
                <div class="input-wrapper brackets">
                    <input
                        id="new-label"
                        type="text"
                        bind:value={label}
                        placeholder="MINING_PAYOUTS..."
                        class="input-glass mono"
                    />
                </div>
            </div>

            <div class="controls-row">
                <label class="check-label">
                    <input
                        type="checkbox"
                        bind:checked={showChange}
                        on:change={refreshList}
                    />
                    <span class="custom-check"></span>
                    INCLUDE CHANGE ADDR
                </label>

                <div class="btn-group">
                    <button
                        class="btn-gen ghost"
                        class:disabled={!isNodeOnline}
                        disabled={!isNodeOnline}
                        on:click={refreshList}
                    >
                        REFRESH
                    </button>
                    <button
                        class="btn-gen cyber-btn"
                        class:disabled={!isNodeOnline}
                        disabled={!isNodeOnline}
                        on:click={generateAddr}
                    >
                        {isNodeOnline ? "[ GENERATE ]" : "[ NOT CONNECTED ]"}
                    </button>
                </div>
            </div>
        </div>
    </div>

    <!-- BOTTOM: LIST -->
    <div
        class="glass-panel panel-strong list-area cyber-panel"
        class:expanded={isExpanded}
    >
        <header class="panel-header">
            <div class="header-left">
                <span class="hud-title mono">[ MY ADDRESSES ]</span>
                <button
                    class="btn-expand"
                    title={isExpanded ? "Collapse" : "Expand Full Screen"}
                    on:click={toggleExpand}
                >
                    <span class="expand-icon">{isExpanded ? "▼" : "▲"}</span>
                </button>
            </div>
            <span class="hint mono">CLICK TO COPY</span>
        </header>

        <!-- TECH HEADER -->
        <div class="header-row addr-grid-header">
            <span
                class="sortable"
                class:active={sortColumn === "label"}
                on:click={() => toggleSort("label")}
                role="button"
                tabindex="0"
                on:keydown={(e) => e.key === "Enter" && toggleSort("label")}
            >
                LABEL
            </span>
            <span>ADDRESS</span>
            <span
                class="right sortable"
                class:active={sortColumn === "balance"}
                on:click={() => toggleSort("balance")}
                role="button"
                tabindex="0"
                on:keydown={(e) => e.key === "Enter" && toggleSort("balance")}
            >
                BALANCE
            </span>
        </div>

        <div class="scroll-body">
            {#each sortedAddresses as item, i}
                <div
                    class="data-row addr-row"
                    role="button"
                    tabindex="0"
                    on:click={() => copyAddr(item.address)}
                    on:keydown={(e) =>
                        (e.key === "Enter" || e.key === " ") &&
                        copyAddr(item.address)}
                    title="Click to Copy"
                    in:fly={{ y: 20, duration: 300, delay: i * 50 }}
                >
                    <span class="dim label-text">{item.label || "-"}</span>
                    <span class="mono addr">{item.address}</span>
                    <span class="mono val right"
                        >{formatBalance(item.balance)}</span
                    >
                </div>
            {/each}
        </div>

        {#if status}
            <div
                class="status-bar mono"
                class:error={status.startsWith("Error")}
                role="status"
            >
                <span class="blink">></span>
                {status}
            </div>
        {/if}
    </div>
</div>

<style>
    .view-receive {
        display: flex;
        flex-direction: column;
        gap: 0.8rem;
        height: 100%;
        min-height: 0;
        overflow: hidden;
        box-sizing: border-box;
    }

    /* --- LAYOUT --- */
    .gen-area {
        flex: 0 0 auto;
    }
    .list-area {
        flex: 1;
        min-height: 0; /* KEY FIX: Allow shrinking to fit window */
        display: flex;
        flex-direction: column;
        transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1); /* PRO TRANSITION */
    }

    .list-area.expanded {
        position: fixed;
        inset: 0.5rem; /* Floating margin */
        z-index: 9999;
        margin: 0;
        border-radius: 8px;
        background: rgba(8, 12, 10, 0.95);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(0, 255, 65, 0.3);
        box-shadow: 0 0 50px rgba(0, 0, 0, 0.8);
        padding: 1.5rem;
        box-sizing: border-box;
    }
    .list-area.expanded .scroll-body {
        min-height: 0; /* Ensure internal scrolling works */
    }

    /* --- CYBER PANEL (asset-page style) --- */
    .cyber-panel {
        background: linear-gradient(
            180deg,
            rgba(8, 14, 12, 0.95) 0%,
            rgba(5, 10, 8, 0.98) 100%
        );
        border: 1px solid rgba(0, 255, 65, 0.15);
        box-shadow:
            0 0 40px rgba(0, 0, 0, 0.5),
            inset 0 0 30px rgba(0, 255, 65, 0.02);
        position: relative;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .panel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 5px 1rem;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }
    .hud-title {
        color: var(--color-primary);
        font-size: 0.75rem;
        letter-spacing: 2px;
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
    }
    .hint {
        color: #555;
        font-size: 0.6rem;
        letter-spacing: 1px;
    }

    /* --- GENERATE BODY --- */
    .gen-body {
        padding: 0.8rem 1rem;
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
    }

    @media (max-height: 700px) {
        .gen-body {
            padding: 0.6rem 0.8rem;
            gap: 0.5rem;
        }
        .panel-header {
            padding: 4px 0.8rem;
        }
        .input-glass {
            padding: 0.45rem 0.6rem;
        }
    }
    .field-label {
        font-size: 0.65rem;
        color: #888;
        margin-bottom: 0.2rem;
        display: block;
        letter-spacing: 0.5px;
    }
    .input-wrapper {
        position: relative;
    }

    .input-glass {
        width: 100%;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.45rem 0.6rem;
        border-radius: 6px;
        outline: none;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        transition: all 0.2s;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.1);
    }

    .controls-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-top: 0.3rem;
    }
    .btn-group {
        display: flex;
        gap: 0.6rem;
    }

    /* Checkbox */
    .check-label {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        font-size: 0.7rem;
        color: #888;
        cursor: pointer;
        letter-spacing: 0.5px;
    }
    .check-label input {
        display: none;
    }
    .custom-check {
        width: 14px;
        height: 14px;
        border: 1px solid #555;
        border-radius: 2px;
        display: inline-block;
        position: relative;
    }
    .check-label input:checked + .custom-check {
        background: var(--color-primary);
        border-color: var(--color-primary);
        box-shadow: 0 0 5px var(--color-primary);
    }

    /* Buttons */
    .btn-gen {
        padding: 0.45rem 1rem;
        font-size: 0.7rem;
        font-weight: 600;
        letter-spacing: 1px;
        cursor: pointer;
        transition: all 0.2s;
        border-radius: 6px;
    }

    .btn-gen.ghost {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .btn-gen.ghost:hover {
        border-color: #fff;
        color: #fff;
    }
    .btn-gen.cyber-btn {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
    }
    .btn-gen.cyber-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.2);
    }
    .btn-gen.disabled,
    .btn-gen:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    /* --- ADDRESS LIST --- */
    .addr-grid-header {
        display: grid;
        grid-template-columns: 180px 1fr 100px; /* Wider label column */
        padding: 0.6rem 1.2rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.02);
        color: var(--color-muted);
        font-size: 0.7rem;
        font-weight: bold;
        letter-spacing: 1px;
    }
    .sortable {
        cursor: pointer;
        user-select: none;
        transition: color 0.2s;
    }
    .sortable:hover {
        color: #fff;
    }
    .sortable.active {
        color: var(--color-primary);
    }
    .scroll-body {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
        border-right: 1px solid rgba(255, 255, 255, 0.02);
    }

    .data-row {
        display: grid;
        grid-template-columns: 180px 1fr 140px; /* Wider balance column */
        padding: 0.8rem 1.2rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        align-items: center;
        transition: all 0.15s;
        cursor: copy;
    }
    .data-row:hover {
        background: rgba(0, 255, 65, 0.05);
    }
    .data-row:active {
        background: rgba(0, 255, 65, 0.2);
        transform: scale(0.998);
    }

    .label-text {
        font-size: 0.8rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .addr {
        color: #ccc;
        font-size: 0.85rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .val {
        color: var(--color-primary);
        font-size: 0.85rem;
    }
    .right {
        text-align: right;
    }

    /* STATUS BAR */
    .status-bar {
        margin: 0;
        padding: 0.6rem 1rem;
        background: rgba(0, 0, 0, 0.4);
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        color: var(--color-primary);
        font-size: 0.75rem;
        flex-shrink: 0;
    }
    .status-bar.error {
        color: #ff5555;
    }
    .blink {
        animation: blink 1s infinite;
    }
    @keyframes blink {
        50% {
            opacity: 0;
        }
    }

    /* GLOBAL SCROLLBAR RE-APPLY */
    .scroll-body::-webkit-scrollbar {
        width: 8px;
    }
    .scroll-body::-webkit-scrollbar-track {
        background: rgba(0, 255, 65, 0.06);
        border-left: 1px solid rgba(0, 255, 65, 0.1);
    }
    .scroll-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.3);
        border-radius: 0;
    }
    .btn-expand {
        background: none;
        border: none;
        color: var(--color-primary);
        font-size: 0.8rem;
        cursor: pointer;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        margin-left: 1rem;
        padding: 0;
    }
    .header-left {
        display: flex;
        align-items: center;
    }
</style>
