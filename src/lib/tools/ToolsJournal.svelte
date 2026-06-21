<script>
    import { onMount } from "svelte";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { save, open } from "@tauri-apps/plugin-dialog";
    import { systemStatus } from "../../stores.js";
    import { addToolNotification } from "../stores/notifications.js";
    import HelpHitbox from "../ui/HelpHitbox.svelte";
    import CommanderLoader from "../ui/CommanderLoader.svelte";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info", notify = true) {
        dispatch("toast", { msg, type, notify });
    }

    let entries = [];
    let loading = false;
    let filterStatus = "ALL";
    let filterOperation = "ALL";
    let operationTypes = [];
    let expandedEntry = null;
    let deleteConfirmId = null;
    let statusMessage = null;
    let statusMessageType = "info";
    let archiveConfirm = false;
    let restoreConfirmFilename = null;
    let archives = [];
    let editMode = false;
    let selectedIds = new Set();
    let bulkDeleteConfirm = false;

    let exportLoading = false;
    let importLoading = false;
    let archiveLoading = false;
    let restoreLoading = false;
    let restoreLoadingFilename = null;

    $: filtersActive = filterStatus !== "ALL" || filterOperation !== "ALL";
    $: allVisibleIds = filteredEntries.map((e) => e.id);
    $: allVisibleSelected = allVisibleIds.length > 0 && allVisibleIds.every((id) => selectedIds.has(id));
    $: selectionCount = [...selectedIds].filter((id) => allVisibleIds.includes(id)).length;

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

    function setStatus(msg, type = "info") {
        statusMessage = msg;
        statusMessageType = type;
    }

    function dismissStatus() {
        statusMessage = null;
    }

    async function loadEntries() {
        if (!tauriReady) return;
        loading = true;
        dismissStatus();
        try {
            entries = await core.invoke("list_tx_journal_entries");
            const types = new Set(entries.map((e) => e.operation_type));
            operationTypes = [...types].sort();
        } catch (err) {
            setStatus("Failed to load journal: " + err, "error");
        }
        loading = false;
    }

    function toggleEditMode() {
        editMode = !editMode;
        if (!editMode) {
            selectedIds.clear();
            bulkDeleteConfirm = false;
            deleteConfirmId = null;
        }
    }

    function toggleSelectEntry(id) {
        if (selectedIds.has(id)) {
            selectedIds.delete(id);
        } else {
            selectedIds.add(id);
        }
        selectedIds = selectedIds;
    }

    function toggleSelectAll() {
        if (allVisibleSelected) {
            for (const id of allVisibleIds) {
                selectedIds.delete(id);
            }
        } else {
            for (const id of allVisibleIds) {
                selectedIds.add(id);
            }
        }
        selectedIds = selectedIds;
    }

    function selectAll() {
        for (const id of allVisibleIds) {
            selectedIds.add(id);
        }
        selectedIds = selectedIds;
    }

    function deselectAll() {
        for (const id of allVisibleIds) {
            selectedIds.delete(id);
        }
        selectedIds = selectedIds;
    }

    async function bulkDelete() {
        if (!tauriReady || selectionCount === 0) return;
        bulkDeleteConfirm = false;
        const idsToDelete = [...selectedIds].filter((id) => allVisibleIds.includes(id));
        try {
            const result = await core.invoke("delete_tx_journal_entries", { ids: idsToDelete });
            setStatus(result, "success");
            addToolNotification("Journal entries deleted", `${idsToDelete.length} entries removed`, "info");
            for (const id of idsToDelete) {
                selectedIds.delete(id);
            }
            selectedIds = selectedIds;
            await loadEntries();
        } catch (err) {
            setStatus("Bulk delete failed: " + err, "error");
        }
    }

    async function exportJournal() {
        if (!tauriReady || exportLoading) return;
        exportLoading = true;
        dismissStatus();
        try {
            const ts = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
            const filePath = await save({
                title: "Export Transaction Journal",
                defaultPath: `hemp0x_tx_journal_${ts}.json`,
                filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (!filePath) {
                exportLoading = false;
                return;
            }
            await core.invoke("export_tx_journal", { path: filePath });
            setStatus("Journal exported to: " + filePath, "success");
        } catch (err) {
            setStatus("Export failed: " + err, "error");
        }
        exportLoading = false;
    }

    async function importMergeJournal() {
        if (!tauriReady || importLoading) return;
        importLoading = true;
        dismissStatus();
        try {
            const selected = await open({
                title: "Import Transaction Journal",
                filters: [{ name: "JSON", extensions: ["json"] }],
                multiple: false,
            });
            if (!selected) {
                importLoading = false;
                return;
            }
            const filePath = typeof selected === "string" ? selected : selected.path;
            const result = await core.invoke("import_merge_tx_journal", { path: filePath });
            setStatus(result, "success");
            await loadEntries();
        } catch (err) {
            setStatus("Import failed: " + err, "error");
        }
        importLoading = false;
    }

    async function archiveJournal() {
        if (!tauriReady || archiveLoading) return;
        archiveConfirm = false;
        archiveLoading = true;
        dismissStatus();
        try {
            const result = await core.invoke("archive_tx_journal");
            setStatus(result, "success");
            addToolNotification("Journal archived and reset", "Archived entries saved to journal_archives/", "info");
            await loadEntries();
            await loadArchives();
        } catch (err) {
            setStatus("Archive failed: " + err, "error");
        }
        archiveLoading = false;
    }

    async function loadArchives() {
        if (!tauriReady) return;
        try {
            archives = await core.invoke("list_tx_journal_archives");
        } catch {
            archives = [];
        }
    }

    async function restoreArchive(filename) {
        if (!tauriReady || restoreLoading) return;
        restoreConfirmFilename = null;
        restoreLoading = true;
        restoreLoadingFilename = filename;
        dismissStatus();
        try {
            const result = await core.invoke("restore_tx_journal_archive", { filename });
            setStatus(result, "success");
            addToolNotification("Journal restored", "Previous journal backed up before restore", "info");
            await loadEntries();
            await loadArchives();
        } catch (err) {
            setStatus("Restore failed: " + err, "error");
        }
        restoreLoading = false;
        restoreLoadingFilename = null;
    }

    async function showJournalPath() {
        if (!tauriReady) return;
        try {
            const path = await core.invoke("get_tx_journal_path");
            setStatus("Journal path: " + path, "info");
        } catch (err) {
            setStatus("Failed to get journal path", "error");
        }
    }

    async function deleteEntry(id) {
        if (!tauriReady) return;
        try {
            const entry = entries.find((e) => e.id === id);
            await core.invoke("delete_tx_journal_entry", { id });
            entries = entries.filter((e) => e.id !== id);
            deleteConfirmId = null;
            selectedIds.delete(id);
            selectedIds = selectedIds;
            setStatus("Entry deleted", "success");
            if (entry) {
                addToolNotification(
                    "Journal entry deleted",
                    entry.summary ? entry.summary.substring(0, 100) : "",
                    "info",
                );
            }
        } catch (err) {
            setStatus("Delete failed: " + err, "error");
        }
    }

    async function deleteArchive(filename) {
        if (!tauriReady) return;
        try {
            const result = await core.invoke("delete_tx_journal_archive", { filename });
            setStatus(result, "info");
            await loadArchives();
        } catch (err) {
            setStatus("Delete archive failed: " + err, "error");
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

    function hasContextData(entry) {
        return entry.network || entry.core_wallet_name || entry.vault_display_name
            || entry.vault_fingerprint || entry.wallet_record_id || entry.alignment_id;
    }

    function formatSize(bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1048576) return (bytes / 1024).toFixed(1) + " KB";
        return (bytes / 1048576).toFixed(1) + " MB";
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

    onMount(() => {
        loadEntries();
    });
</script>

<div class="journal-view">
    <div class="journal-controls">
        <div class="journal-title-row">
            <HelpHitbox title="Transaction Journal">
                <p>The journal is a local activity log of previews, broadcasts, asset operations, and operator actions.</p>
                <p>Deleting entries only removes local records in Commander. It does not cancel or alter on-chain transactions.</p>
                <p>Use IMPORT to merge entries from an exported journal file (duplicates by ID are skipped).</p>
                <p>Journal entries can include non-secret context: network, wallet name, vault display name.</p>
                <p>Toggle EDIT to select and delete multiple entries at once.</p>
                <p>The journal is stored locally in the Commander app data folder as tx_journal.json.</p>
            </HelpHitbox>
        </div>

        {#if statusMessage}
            <div class="status-banner-compact {statusMessageType}">
                <span class="status-text">{statusMessage}</span>
                <button class="dismiss" on:click={dismissStatus} aria-label="Dismiss">&times;</button>
            </div>
        {/if}

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
                {#if entries.length > 0 && !loading}
                    <button class="cyber-btn ghost" on:click={toggleEditMode} class:active={editMode}>
                        {editMode ? "DONE" : "EDIT"}
                    </button>
                {/if}
                <button class="cyber-btn ghost" on:click={exportJournal} disabled={exportLoading || importLoading || archiveLoading || restoreLoading}>
                    {exportLoading ? "EXPORTING..." : "EXPORT"}
                </button>
                <button class="cyber-btn ghost" on:click={importMergeJournal} disabled={exportLoading || importLoading || archiveLoading || restoreLoading}>
                    {importLoading ? "IMPORTING..." : "IMPORT"}
                </button>
                <button class="cyber-btn" on:click={loadEntries} disabled={loading || exportLoading || importLoading || archiveLoading || restoreLoading}>
                    {loading ? "LOADING..." : "REFRESH"}
                </button>
            </div>
        </div>

        {#if editMode && entries.length > 0 && !loading}
            <div class="edit-toolbar">
                <span class="selection-info">
                    {selectionCount} of {allVisibleIds.length} selected
                </span>
                <button class="text-btn" on:click={selectAll}>SELECT ALL</button>
                <button class="text-btn" on:click={deselectAll}>DESELECT ALL</button>
                {#if !bulkDeleteConfirm}
                    <button class="text-btn danger" on:click={() => (bulkDeleteConfirm = true)} disabled={selectionCount === 0}>
                        DELETE SELECTED
                    </button>
                {:else}
                    <span class="confirm-action-row">
                        <button class="text-btn danger" on:click={bulkDelete}>CONFIRM DELETE</button>
                        <button class="text-btn" on:click={() => (bulkDeleteConfirm = false)}>CANCEL</button>
                    </span>
                {/if}
            </div>
        {/if}
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
                            {#if editMode}
                                <th class="check-col">
                                    <input
                                        type="checkbox"
                                        class="journal-check"
                                        checked={allVisibleSelected}
                                        on:change={toggleSelectAll}
                                        aria-label="Select all visible journal entries"
                                    />
                                </th>
                            {/if}
                            <th>STATUS</th>
                            <th>TYPE</th>
                            <th>SUMMARY</th>
                            <th>TXID</th>
                            <th>CTX</th>
                            <th>UPDATED</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each filteredEntries as entry}
                            <tr class="journal-row" class:expanded={expandedEntry === entry.id} class:selected={selectedIds.has(entry.id)}>
                                {#if editMode}
                                    <td class="check-col">
                                        <input
                                            type="checkbox"
                                            class="journal-check"
                                            checked={selectedIds.has(entry.id)}
                                            on:change={() => toggleSelectEntry(entry.id)}
                                            aria-label={`Select journal entry ${entry.id}`}
                                        />
                                    </td>
                                {/if}
                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                <!-- svelte-ignore a11y_no_static_element_interactions -->
                                <td class="status-cell" style={statusColors[entry.status] || ""} on:click={() => toggleDetails(entry.id)}>
                                    {entry.status}
                                </td>
                                <td on:click={() => toggleDetails(entry.id)}>{entry.operation_type}</td>
                                <td class="summary-cell" on:click={() => toggleDetails(entry.id)}>{truncate(entry.summary, 40)}</td>
                                <td class="txid-cell mono" on:click={() => toggleDetails(entry.id)}>{truncate(entry.txid, 16)}</td>
                                <td class="ctx-cell" on:click={() => toggleDetails(entry.id)}>
                                    {#if hasContextData(entry)}
                                        <span class="ctx-badge" title="Journal context data available">&bull;</span>
                                    {:else}
                                        <span class="ctx-none">-</span>
                                    {/if}
                                </td>
                                <td class="time-cell" on:click={() => toggleDetails(entry.id)}>{formatTime(entry.updated_at)}</td>
                            </tr>
                            {#if expandedEntry === entry.id}
                                <tr class="details-row">
                                    <td colspan={editMode ? 7 : 6}>
                                        <div class="details-panel">
                                            <div class="detail-line"><span class="detail-key">ID:</span><span class="detail-value mono">{entry.id}</span></div>
                                            <div class="detail-line"><span class="detail-key">Created:</span><span class="detail-value">{formatTime(entry.created_at)}</span></div>
                                            <div class="detail-line"><span class="detail-key">Summary:</span><span class="detail-value">{entry.summary}</span></div>
                                            {#if entry.txid}
                                                <div class="detail-line"><span class="detail-key">TXID:</span><span class="detail-value mono">{entry.txid}</span></div>
                                            {/if}
                                            {#if hasContextData(entry)}
                                                <div class="detail-section-title">Context</div>
                                                {#if entry.network}
                                                    <div class="detail-line"><span class="detail-key">Network:</span><span class="detail-value">{entry.network}</span></div>
                                                {/if}
                                                {#if entry.core_wallet_name}
                                                    <div class="detail-line"><span class="detail-key">Wallet:</span><span class="detail-value">{entry.core_wallet_name}</span></div>
                                                {/if}
                                                {#if entry.vault_display_name}
                                                    <div class="detail-line"><span class="detail-key">Vault:</span><span class="detail-value">{entry.vault_display_name}</span></div>
                                                {/if}
                                                {#if entry.vault_fingerprint}
                                                    <div class="detail-line"><span class="detail-key">Vault FP:</span><span class="detail-value mono">{truncate(entry.vault_fingerprint, 24)}</span></div>
                                                {/if}
                                                {#if entry.wallet_record_id}
                                                    <div class="detail-line"><span class="detail-key">Record ID:</span><span class="detail-value mono">{entry.wallet_record_id}</span></div>
                                                {/if}
                                                {#if entry.alignment_id}
                                                    <div class="detail-line"><span class="detail-key">Align ID:</span><span class="detail-value mono">{truncate(entry.alignment_id, 16)}</span></div>
                                                {/if}
                                            {/if}
                                            {#if entry.details && typeof entry.details === "object" && Object.keys(entry.details).length > 0}
                                                <div class="detail-section-title">Details</div>
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

{#if exportLoading || importLoading || archiveLoading || restoreLoading}
    <div class="loader-overlay">
        <div class="loader-panel">
            <CommanderLoader compact={true} label="" detail="" />
            <h3>
                {#if exportLoading}EXPORTING JOURNAL
                {:else if importLoading}IMPORTING JOURNAL
                {:else if archiveLoading}ARCHIVING JOURNAL
                {:else if restoreLoading}RESTORING JOURNAL
                {/if}
            </h3>
            <p>Please wait while the operation completes.</p>
        </div>
    </div>
{/if}

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
    .journal-title-row {
        display: flex;
        justify-content: flex-end;
        margin-bottom: 0.3rem;
    }
    .status-text {
        flex: 1;
        word-break: break-word;
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
    .filter-group-label {
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
    .edit-toolbar {
        display: flex;
        align-items: center;
        gap: 0.8rem;
        padding: 0.5rem 0;
        margin-top: 0.3rem;
        border-top: 1px solid rgba(255, 255, 255, 0.04);
    }
    .selection-info {
        font-size: 0.7rem;
        color: var(--color-primary);
        font-family: var(--font-mono);
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
    .journal-table th.check-col {
        width: 28px;
        text-align: center;
        padding: 0.5rem 0.2rem;
    }
    .journal-table td {
        padding: 0.45rem 0.6rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        color: #ccc;
        cursor: pointer;
        white-space: nowrap;
    }
    .journal-table td.check-col {
        width: 28px;
        text-align: center;
        padding: 0.45rem 0.2rem;
    }
    .journal-row:hover {
        background: rgba(0, 255, 65, 0.03);
    }
    .journal-row.expanded {
        background: rgba(0, 255, 65, 0.05);
    }
    .journal-row.selected {
        background: rgba(0, 255, 65, 0.06);
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
    .ctx-cell {
        text-align: center;
        width: 30px;
    }
    .ctx-badge {
        color: var(--color-primary);
        font-weight: bold;
        font-size: 1.1rem;
    }
    .ctx-none {
        color: #444;
    }
    .journal-check {
        width: 16px;
        height: 16px;
        accent-color: var(--color-primary);
        cursor: pointer;
        vertical-align: middle;
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
    .detail-section-title {
        color: var(--color-primary);
        font-size: 0.65rem;
        letter-spacing: 1px;
        text-transform: uppercase;
        margin-top: 0.5rem;
        margin-bottom: 0.2rem;
        padding-top: 0.3rem;
        border-top: 1px solid rgba(0, 255, 65, 0.1);
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
    .archives-section {
        flex-shrink: 0;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        padding-top: 0.5rem;
        max-height: 160px;
        overflow-y: auto;
    }
    .archives-header {
        margin-bottom: 0.3rem;
    }
    .archives-list {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .archive-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.25rem 0.4rem;
        font-size: 0.68rem;
        background: rgba(0, 0, 0, 0.2);
        border-radius: 3px;
    }
    .archive-row:hover {
        background: rgba(0, 0, 0, 0.35);
    }
    .archive-name {
        color: #aaa;
        flex-shrink: 1;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .archive-meta {
        color: #555;
        flex: 1;
        white-space: nowrap;
        font-size: 0.65rem;
    }
    .archive-actions {
        display: flex;
        gap: 0.5rem;
        flex-shrink: 0;
    }
    .text-btn {
        background: none;
        border: none;
        color: var(--color-primary);
        cursor: pointer;
        font-size: 0.7rem;
        padding: 0.15rem 0.4rem;
        font-family: var(--font-mono);
        letter-spacing: 0.5px;
    }
    .text-btn:hover {
        text-decoration: underline;
    }
    .text-btn:disabled {
        color: #444;
        cursor: not-allowed;
        text-decoration: none;
    }
    .text-btn.danger {
        color: #ff5555;
    }
    .text-btn.danger:disabled {
        color: #552222;
    }
    .confirm-delete-row {
        display: flex;
        gap: 0.3rem;
        justify-content: center;
    }
    .confirm-action-row {
        display: flex;
        gap: 0.3rem;
        align-items: center;
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
    .cyber-btn.active {
        border-color: var(--color-primary);
        color: #000;
        background: var(--color-primary);
    }
</style>
