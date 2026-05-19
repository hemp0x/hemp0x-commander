<script>
    import { onMount } from "svelte";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { save } from "@tauri-apps/plugin-dialog";
    import { systemStatus } from "../../stores.js";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    let entries = [];
    let loading = false;
    let filterStatus = "ALL";
    let filterOperation = "ALL";
    let operationTypes = [];
    let expandedEntry = null;
    let deleteConfirmId = null;

    $: filteredEntries = (() => {
        let result = entries;
        if (filterStatus !== "ALL") {
            result = result.filter((e) => e.status === filterStatus);
        }
        if (filterOperation !== "ALL") {
            result = result.filter((e) => e.operation_type === filterOperation);
        }
        return result.sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    })();

    async function loadEntries() {
        if (!tauriReady) return;
        loading = true;
        try {
            entries = await core.invoke("list_tx_journal_entries");
            const types = new Set(entries.map((e) => e.operation_type));
            operationTypes = [...types].sort();
        } catch (err) {
            showToast("Failed to load journal: " + err, "error");
        }
        loading = false;
    }

    async function exportJournal() {
        if (!tauriReady) return;
        try {
            const ts = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
            const filePath = await save({
                title: "Export Transaction Journal",
                defaultPath: `hemp0x_tx_journal_${ts}.json`,
                filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (!filePath) return;
            await core.invoke("export_tx_journal", { path: filePath });
            showToast("Journal exported to: " + filePath, "success");
        } catch (err) {
            showToast("Export failed: " + err, "error");
        }
    }

    async function showJournalPath() {
        if (!tauriReady) return;
        try {
            const path = await core.invoke("get_tx_journal_path");
            showToast("Journal path: " + path, "info");
        } catch (err) {
            showToast("Failed to get journal path", "error");
        }
    }

    async function deleteEntry(id) {
        if (!tauriReady) return;
        try {
            await core.invoke("delete_tx_journal_entry", { id });
            entries = entries.filter((e) => e.id !== id);
            deleteConfirmId = null;
            showToast("Entry deleted", "success");
        } catch (err) {
            showToast("Delete failed: " + err, "error");
        }
    }

    function toggleDetails(id) {
        expandedEntry = expandedEntry === id ? null : id;
    }

    function formatTime(iso) {
        try {
            const d = new Date(iso);
            return d.toLocaleString();
        } catch {
            return iso;
        }
    }

    function truncate(value, max) {
        if (!value) return "-";
        return value.length > max ? value.substring(0, max) + "..." : value;
    }

    const statusColors = {
        Previewed: "color: var(--color-primary);",
        Broadcasted: "color: #00ffc8;",
        Confirmed: "color: #00ff41;",
        Failed: "color: #ff5555;",
        Abandoned: "color: #888;",
        Draft: "color: #666;",
        Signed: "color: #ffaa00;",
    };

    onMount(loadEntries);
</script>

<div class="journal-view">
    <div class="journal-controls">
        <div class="filter-row">
            <div class="filter-group">
                <label for="journal-filter-status">STATUS</label>
                <select id="journal-filter-status" class="input-glass" bind:value={filterStatus}>
                    <option value="ALL">ALL</option>
                    <option value="Previewed">Previewed</option>
                    <option value="Broadcasted">Broadcasted</option>
                    <option value="Confirmed">Confirmed</option>
                    <option value="Failed">Failed</option>
                    <option value="Abandoned">Abandoned</option>
                    <option value="Draft">Draft</option>
                    <option value="Signed">Signed</option>
                </select>
            </div>
            <div class="filter-group">
                <label for="journal-filter-type">TYPE</label>
                <select id="journal-filter-type" class="input-glass" bind:value={filterOperation}>
                    <option value="ALL">ALL</option>
                    {#each operationTypes as op}
                        <option value={op}>{op}</option>
                    {/each}
                </select>
            </div>
            <div class="filter-group actions-right">
                <button class="cyber-btn ghost" on:click={showJournalPath}>PATH</button>
                <button class="cyber-btn ghost" on:click={exportJournal}>EXPORT</button>
                <button class="cyber-btn" on:click={loadEntries} disabled={loading}>
                    {loading ? "LOADING..." : "REFRESH"}
                </button>
            </div>
        </div>
    </div>

    <div class="journal-list">
        {#if loading}
            <div class="empty-state">Loading journal entries...</div>
        {:else if filteredEntries.length === 0}
            <div class="empty-state">
                {entries.length === 0 ? "No journal entries yet. Send or preview a transaction first." : "No entries match current filters."}
            </div>
        {:else}
            <div class="journal-table-wrap">
                <table class="journal-table">
                    <thead>
                        <tr>
                            <th>STATUS</th>
                            <th>TYPE</th>
                            <th>SUMMARY</th>
                            <th>TXID</th>
                            <th>UPDATED</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each filteredEntries as entry}
                            <tr class="journal-row" class:expanded={expandedEntry === entry.id}>
                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                <!-- svelte-ignore a11y_no_static_element_interactions -->
                                <td class="status-cell" style={statusColors[entry.status] || ""} on:click={() => toggleDetails(entry.id)}>
                                    {entry.status}
                                </td>
                                <td on:click={() => toggleDetails(entry.id)}>{entry.operation_type}</td>
                                <td class="summary-cell" on:click={() => toggleDetails(entry.id)}>{truncate(entry.summary, 40)}</td>
                                <td class="txid-cell mono" on:click={() => toggleDetails(entry.id)}>{truncate(entry.txid, 16)}</td>
                                <td class="time-cell" on:click={() => toggleDetails(entry.id)}>{formatTime(entry.updated_at)}</td>
                                <td class="action-cell">
                                    {#if deleteConfirmId === entry.id}
                                        <span class="confirm-delete-row">
                                            <button class="text-btn danger" on:click={() => deleteEntry(entry.id)}>YES</button>
                                            <button class="text-btn" on:click={() => (deleteConfirmId = null)}>NO</button>
                                        </span>
                                    {:else}
                                        <button class="text-btn danger" on:click={() => (deleteConfirmId = entry.id)} title="Delete entry">DEL</button>
                                    {/if}
                                </td>
                            </tr>
                            {#if expandedEntry === entry.id}
                                <tr class="details-row">
                                    <td colspan="6">
                                        <div class="details-panel">
                                            <div class="detail-line"><span class="detail-key">ID:</span><span class="detail-value mono">{entry.id}</span></div>
                                            <div class="detail-line"><span class="detail-key">Created:</span><span class="detail-value">{formatTime(entry.created_at)}</span></div>
                                            <div class="detail-line"><span class="detail-key">Summary:</span><span class="detail-value">{entry.summary}</span></div>
                                            {#if entry.txid}
                                                <div class="detail-line"><span class="detail-key">TXID:</span><span class="detail-value mono">{entry.txid}</span></div>
                                            {/if}
                                            {#if entry.details && typeof entry.details === "object" && Object.keys(entry.details).length > 0}
                                                <div class="detail-line"><span class="detail-key">Details:</span></div>
                                                <pre class="detail-json">{JSON.stringify(entry.details, null, 2)}</pre>
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

    {#if entries.length > 0}
        <div class="journal-footer">
            <span class="entry-count mono">{filteredEntries.length} of {entries.length} entries</span>
        </div>
    {/if}
</div>

<style>
    .journal-view {
        display: flex;
        flex-direction: column;
        height: 100%;
        gap: 0.5rem;
        overflow: hidden;
    }
    .journal-controls {
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
        min-width: 110px;
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
    }
    select.input-glass option {
        background: #0a0a0a;
        color: #fff;
    }
    .journal-list {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }
    .journal-table-wrap {
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
    }
    .journal-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.78rem;
    }
    .journal-table th {
        text-align: left;
        padding: 0.5rem 0.6rem;
        color: #555;
        font-size: 0.65rem;
        letter-spacing: 1px;
        text-transform: uppercase;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        position: sticky;
        top: 0;
        z-index: 1;
    }
    .journal-table td {
        padding: 0.45rem 0.6rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        color: #ccc;
        cursor: pointer;
        white-space: nowrap;
    }
    .journal-row:hover {
        background: rgba(0, 255, 65, 0.03);
    }
    .journal-row.expanded {
        background: rgba(0, 255, 65, 0.05);
    }
    .status-cell {
        font-weight: 600;
        font-size: 0.7rem;
        letter-spacing: 0.5px;
    }
    .summary-cell {
        max-width: 200px;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .txid-cell, .time-cell {
        font-size: 0.7rem;
        color: #888;
    }
    .action-cell {
        text-align: center;
        width: 50px;
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
        min-width: 60px;
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
    .journal-footer {
        flex-shrink: 0;
        text-align: right;
        padding: 0.3rem 0;
    }
    .entry-count {
        color: #555;
        font-size: 0.65rem;
    }
    .text-btn {
        background: none;
        border: none;
        color: var(--color-primary);
        cursor: pointer;
        font-size: 0.7rem;
        padding: 0.15rem 0.4rem;
        font-family: var(--font-mono);
    }
    .text-btn:hover {
        text-decoration: underline;
    }
    .text-btn.danger {
        color: #ff5555;
    }
    .confirm-delete-row {
        display: flex;
        gap: 0.3rem;
        justify-content: center;
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
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
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
