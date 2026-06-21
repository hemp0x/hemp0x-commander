<script>
    import { onMount } from "svelte";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { systemStatus } from "../../stores.js";
    import CopyIcon from "../ui/CopyIcon.svelte";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    function openJournal() {
        dispatch("switch", "JOURNAL");
    }

    const PAGE_SIZE = 50;
    let items = [];
    let total = 0;
    let hasMore = false;
    let loading = false;
    let skip = 0;
    let filterCategory = "ALL";
    let searchQuery = "";
    let expandedTxid = null;

    $: visibleItems = (() => {
        if (!searchQuery) return items;
        const q = searchQuery.toLowerCase();
        return items.filter((tx) => {
            if (tx.txid && tx.txid.toLowerCase().includes(q)) return true;
            if (tx.address && tx.address.toLowerCase().includes(q)) return true;
            if (tx.asset && tx.asset.toLowerCase().includes(q)) return true;
            return false;
        });
    })();

    async function loadPage(reset = true) {
        if (!tauriReady || loading) return;
        loading = true;
        if (reset) {
            skip = 0;
            items = [];
        }
        try {
            const result = await core.invoke("get_transaction_history", {
                count: PAGE_SIZE,
                skip,
                category: filterCategory === "ALL" ? null : filterCategory,
            });
            if (reset) {
                items = result.items;
            } else {
                items = [...items, ...result.items];
            }
            total = result.total;
            hasMore = result.has_more;
        } catch (err) {
            showToast("Failed to load transactions: " + err, "error");
        }
        loading = false;
    }

    function loadMore() {
        if (!hasMore || loading) return;
        skip += PAGE_SIZE;
        loadPage(false);
    }

    function refresh() {
        searchQuery = "";
        loadPage(true);
    }

    async function copyValue(value, label) {
        try {
            await navigator.clipboard.writeText(value);
            showToast(`${label} copied`, "success");
        } catch {
            showToast("Copy failed - use Ctrl+C", "error");
        }
    }

    function openExplorer(target) {
        if (!target) return;
        window.dispatchEvent(
            new CustomEvent("commander-open-explorer", {
                detail: { target },
            }),
        );
    }

    function toggleDetails(txid) {
        expandedTxid = expandedTxid === txid ? null : txid;
    }

    function formatAmount(amountStr) {
        const n = parseFloat(amountStr);
        if (isNaN(n)) return amountStr;
        return n.toFixed(8);
    }

    const categoryColors = {
        "receive": "color: #00ff41;",
        "send": "color: #ff5555;",
        "generate": "color: #ffaa00;",
        "immature": "color: #ffaa00;",
        "orphan": "color: #888;",
        "move": "color: #00ccff;",
    };

    onMount(() => {
        if (tauriReady) loadPage(true);
    });
</script>

<div class="history-view">
    <div class="history-controls">
        <div class="filter-row">
            <div class="filter-group">
                <label for="hist-filter-cat">TYPE</label>
                <select id="hist-filter-cat" class="input-glass" bind:value={filterCategory} on:change={() => loadPage(true)}>
                    <option value="ALL">ALL</option>
                    <option value="send">send</option>
                    <option value="receive">receive</option>
                    <option value="generate">generate</option>
                    <option value="immature">immature</option>
                    <option value="move">move</option>
                </select>
            </div>
            <div class="filter-group search-group">
                <label for="hist-search">SEARCH</label>
                <input
                    id="hist-search"
                    type="text"
                    class="input-glass mono"
                    placeholder="txid / address / asset..."
                    bind:value={searchQuery}
                />
            </div>
            <div class="filter-group actions-right">
                <button class="cyber-btn ghost" on:click={openJournal} title="Open transaction journal">
                    JOURNAL
                </button>
                <button class="cyber-btn ghost" on:click={refresh} disabled={loading}>
                    REFRESH
                </button>
            </div>
        </div>
    </div>

    <div class="history-list">
        {#if loading && items.length === 0}
            <div class="empty-state">Loading transactions...</div>
        {:else if visibleItems.length === 0}
            <div class="empty-state">
                {items.length === 0 && !loading ? "No transactions found. Send or receive funds to see history here." : "No transactions match current filters/search."}
            </div>
        {:else}
            <div class="history-table-wrap">
                <table class="history-table">
                    <thead>
                        <tr>
                            <th>DATE</th>
                            <th>TYPE</th>
                            <th>AMOUNT</th>
                            <th>ASSET</th>
                            <th>CONF</th>
                            <th>ADDRESS</th>
                            <th>TXID</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each visibleItems as tx}
                            <tr class="history-row" class:expanded={expandedTxid === tx.txid}>
                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                <!-- svelte-ignore a11y_no_static_element_interactions -->
                                <td class="date-cell" on:click={() => toggleDetails(tx.txid)}>{tx.date}</td>
                                <td class="type-cell" style={categoryColors[tx.type] || ""} on:click={() => toggleDetails(tx.txid)}>
                                    {tx.type}
                                </td>
                                <td class="amount-cell" class:negative={tx.type === "send"} on:click={() => toggleDetails(tx.txid)}>
                                    {formatAmount(tx.amount)}
                                </td>
                                <td class="asset-cell" on:click={() => toggleDetails(tx.txid)}>
                                    {tx.asset || "HEMP"}
                                </td>
                                <td class="conf-cell" on:click={() => toggleDetails(tx.txid)}>{tx.confirmations}</td>
                                <td class="addr-cell mono">
                                    <button
                                        class="address-summary-btn"
                                        on:click={() => toggleDetails(tx.txid)}
                                        title={tx.address || ""}
                                    >
                                        {tx.address ? tx.address.substring(0, 12) + "..." : "-"}
                                    </button>
                                    {#if tx.address}
                                        <button
                                            class="copy-btn"
                                            on:click={() => openExplorer(tx.address)}
                                            title="Explore address"
                                            aria-label="Explore address"
                                        >
                                            &#x2315;
                                        </button>
                                        <button
                                            class="copy-btn"
                                            on:click={() => copyValue(tx.address, "Address")}
                                            title="Copy address"
                                            aria-label="Copy address"
                                        >
                                            <CopyIcon size={11} />
                                        </button>
                                    {/if}
                                </td>
                                <td class="txid-cell">
                                    <span class="mono txid-text">{tx.txid.substring(0, 16)}...</span>
                                    <button
                                        class="copy-btn"
                                        on:click={() => openExplorer(tx.txid)}
                                        title="Explore transaction"
                                        aria-label="Explore transaction"
                                    >
                                        &#x2315;
                                    </button>
                                    <button
                                        class="copy-btn"
                                        on:click={() => copyValue(tx.txid, "TXID")}
                                        title="Copy TXID"
                                        aria-label="Copy transaction ID"
                                    >
                                        <CopyIcon size={11} />
                                    </button>
                                </td>
                            </tr>
                            {#if expandedTxid === tx.txid}
                                <tr class="details-row">
                                    <td colspan="7">
                                        <div class="details-panel">
                                            <div class="detail-line"><span class="detail-key">TXID:</span><span class="detail-value mono">{tx.txid}</span></div>
                                            {#if tx.address}
                                                <div class="detail-line"><span class="detail-key">Address:</span><span class="detail-value mono">{tx.address}</span></div>
                                            {/if}
                                            {#if tx.asset}
                                                <div class="detail-line"><span class="detail-key">Asset:</span><span class="detail-value">{tx.asset}</span></div>
                                            {/if}
                                            {#if tx.fee}
                                                <div class="detail-line"><span class="detail-key">Fee:</span><span class="detail-value">{tx.fee} HEMP</span></div>
                                            {/if}
                                            <div class="detail-line"><span class="detail-key">Date:</span><span class="detail-value">{tx.date}</span></div>
                                            <div class="detail-line"><span class="detail-key">Confirmations:</span><span class="detail-value">{tx.confirmations}</span></div>
                                            <div class="detail-line"><span class="detail-key">Amount:</span><span class="detail-value">{formatAmount(tx.amount)} {tx.asset || "HEMP"}</span></div>
                                            <div class="detail-line"><span class="detail-key">Type:</span><span class="detail-value">{tx.type}</span></div>
                                            {#if tx.raw && Object.keys(tx.raw).length > 0}
                                                <div class="detail-line"><span class="detail-key">Raw:</span></div>
                                                <pre class="detail-json">{JSON.stringify(tx.raw, null, 2)}</pre>
                                            {/if}
                                        </div>
                                    </td>
                                </tr>
                            {/if}
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
    </div>

    <div class="history-footer">
        <span class="entry-count mono">
            {visibleItems.length > 0 ? `Showing ${visibleItems.length}` : ""}
            {total > 0 ? ` of ${total}${hasMore ? "+" : ""} loaded` : ""}
        </span>
        {#if hasMore}
            <button class="cyber-btn ghost" on:click={loadMore} disabled={loading}>
                {loading ? "LOADING..." : `LOAD MORE (${PAGE_SIZE})`}
            </button>
        {:else if items.length > 0}
            <span class="mono" style="color: #555; font-size: 0.7rem;">All transactions loaded</span>
        {/if}
    </div>
</div>

<style>
    .history-view {
        display: flex;
        flex-direction: column;
        height: 100%;
        gap: 0.5rem;
        overflow: hidden;
    }
    .history-controls {
        flex-shrink: 0;
    }
    .filter-row {
        display: flex;
        gap: 0.8rem;
        align-items: flex-end;
        flex-wrap: wrap;
    }
    .filter-group {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .filter-group label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1px;
        text-transform: uppercase;
    }
    .search-group {
        flex: 1;
        min-width: 160px;
    }
    .actions-right {
        margin-left: auto;
        flex-direction: row;
        gap: 0.5rem;
        align-self: flex-end;
    }
    .input-glass {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.4rem 0.6rem;
        font-size: 0.75rem;
        border-radius: 4px;
        outline: none;
        font-family: var(--font-mono);
    }
    .input-glass:focus {
        border-color: var(--color-primary);
    }
    select.input-glass {
        cursor: pointer;
        appearance: none;
        -webkit-appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath fill='%2300ff41' d='M5 7L1 3h8z'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 0.5rem center;
        padding-right: 1.6rem;
        min-width: 110px;
    }
    select.input-glass option {
        background: #0a0a0a;
        color: #fff;
    }
    input.input-glass {
        width: 100%;
        box-sizing: border-box;
    }
    .history-list {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }
    .history-table-wrap {
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
    }
    .history-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.78rem;
    }
    .history-table th {
        text-align: left;
        padding: 0.5rem 0.5rem;
        color: #555;
        font-size: 0.6rem;
        letter-spacing: 1px;
        text-transform: uppercase;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        position: sticky;
        top: 0;
        z-index: 1;
        white-space: nowrap;
    }
    .history-table td {
        padding: 0.4rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        color: #ccc;
        cursor: pointer;
        white-space: nowrap;
    }
    .history-row:hover {
        background: rgba(0, 255, 65, 0.03);
    }
    .history-row.expanded {
        background: rgba(0, 255, 65, 0.05);
    }
    .date-cell { font-size: 0.7rem; color: #888; }
    .type-cell { font-size: 0.7rem; font-weight: 600; letter-spacing: 0.5px; }
    .amount-cell { font-size: 0.75rem; font-weight: 600; }
    .amount-cell.negative { color: #ff6666; }
    .asset-cell { font-size: 0.7rem; color: #aaa; }
    .conf-cell { font-size: 0.7rem; color: #888; }
    .addr-cell {
        display: flex;
        align-items: center;
        gap: 4px;
        font-size: 0.7rem;
        color: #aaa;
    }
    .address-summary-btn {
        min-width: 0;
        padding: 0;
        overflow: hidden;
        border: 0;
        background: transparent;
        color: inherit;
        font: inherit;
        letter-spacing: 0;
        text-align: left;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .address-summary-btn:hover {
        background: transparent;
        color: var(--color-primary);
        box-shadow: none;
        transform: none;
    }
    .txid-cell {
        display: flex;
        align-items: center;
        gap: 4px;
    }
    .txid-text { font-size: 0.68rem; color: #666; }
    .copy-btn {
        background: none;
        border: 1px solid rgba(255,255,255,0.1);
        color: #888;
        cursor: pointer;
        font-size: 0.7rem;
        padding: 0 0.3rem;
        border-radius: 3px;
        flex-shrink: 0;
    }
    .copy-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .details-row td {
        padding: 0;
        border-bottom: 1px solid rgba(0, 255, 65, 0.08);
    }
    .details-panel {
        background: rgba(0, 0, 0, 0.5);
        padding: 0.8rem 1rem;
        font-size: 0.75rem;
        color: #aaa;
        border-left: 2px solid var(--color-primary);
    }
    .detail-line {
        display: flex;
        gap: 0.5rem;
        padding: 0.15rem 0;
    }
    .detail-key {
        color: #666;
        flex-shrink: 0;
        min-width: 80px;
    }
    .detail-value {
        word-break: break-all;
    }
    .detail-json {
        margin: 0.5rem 0 0 0;
        padding: 0.5rem;
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.7rem;
        color: #aaffaa;
        overflow-x: auto;
        max-height: 200px;
        overflow-y: auto;
    }
    .empty-state {
        text-align: center;
        color: #555;
        padding: 2rem;
        font-size: 0.85rem;
    }
    .history-footer {
        flex-shrink: 0;
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.3rem 0;
    }
    .entry-count {
        color: #555;
        font-size: 0.7rem;
    }
    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.4rem 0.8rem;
        font-size: 0.7rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        white-space: nowrap;
        font-family: var(--font-mono);
    }
    .cyber-btn:hover {
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
    .cyber-btn.ghost:hover {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }
</style>
