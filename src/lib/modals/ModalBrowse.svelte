<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import eyeOpen from "../../assets/eye-open.png";
    import eyeClosed from "../../assets/eye-closed.png";

    const dispatch = createEventDispatcher();

    export let isOpen = false;
    export let inline = false;

    let searchInput = "";
    let activePattern = "";
    let typeFilter = "ALL";

    /**
     * @typedef {{
     *   name: string,
     *   balance: number,
     *   type: string,
     *   units?: number,
     *   reissuable?: boolean,
     *   has_ipfs?: boolean,
     *   raw: any
     * }} BrowseAsset
     */

    /** @type {BrowseAsset[]} */
    let allResults = [];
    /** @type {BrowseAsset[]} */
    let displayResults = [];
    let loading = false;
    let error = "";
    let searched = false;
    let currentLimit = 100;
    let hasMore = false;
    let lastFetchTime = "";
    let cacheUsed = false;

    const PAGE_SIZE = 100;
    const MAX_LIMIT = 2000;

    const HIDDEN_LIST_KEY = "hemp0x_browse_hidden_list";
    const BROWSE_CACHE_KEY = "hemp0x_browse_cache";
    const BLOCK_TAGS_KEY = "hemp0x_browse_block_tags";

    /** @type {Set<string>} */
    let hiddenAssets = new Set();
    let showHidden = false;
    let settingsOpen = false;

    /** @type {string[]} */
    let blockTags = ["#SPAM"];
    let blockTagsInput = "#SPAM";
    let tagFilterActive = false;
    let tagCheckPending = false;
    /** @type {Set<string>} */
    let tagBlockedNames = new Set();

    let sortKey = "";
    let sortDir = "asc";

    const TYPE_FILTERS = ["ALL", "TOKEN", "SUB", "NFT", "QUALIFIER", "RESTRICTED", "OWNER"];

    function classifyAssetType(/** @type {string} */ name) {
        if (name.endsWith("!")) return "OWNER";
        if (name.startsWith("#")) return "QUALIFIER";
        if (name.startsWith("$")) return "RESTRICTED";
        if (name.includes("/")) return "SUB";
        if (name.includes("#")) return "NFT";
        return "TOKEN";
    }

    function typeLabel(/** @type {string} */ type) {
        const labels = { TOKEN: "TOKEN", SUB: "SUB", NFT: "NFT", QUALIFIER: "QUAL", RESTRICTED: "RESTR", OWNER: "OWNER" };
        return labels[type] || type;
    }

    function typeColor(/** @type {string} */ type) {
        const colors = { TOKEN: "#00ff41", SUB: "#00ccff", NFT: "#ffaa00", QUALIFIER: "#cc77ff", RESTRICTED: "#ff5555", OWNER: "#888888" };
        return colors[type] || "#888";
    }

    function formatBalance(/** @type {number} */ bal) {
        if (bal === 0) return "0";
        if (bal === Math.floor(bal)) return bal.toLocaleString();
        return bal.toFixed(8).replace(/0+$/, "").replace(/\.$/, "");
    }

    function loadHiddenAssets() {
        try {
            const raw = localStorage.getItem(HIDDEN_LIST_KEY);
            if (raw) hiddenAssets = new Set(JSON.parse(raw));
        } catch {}
    }
    function saveHiddenAssets() {
        try { localStorage.setItem(HIDDEN_LIST_KEY, JSON.stringify([...hiddenAssets])); } catch {}
    }

    function loadBlockTags() {
        try {
            const raw = localStorage.getItem(BLOCK_TAGS_KEY);
            if (raw) {
                blockTags = JSON.parse(raw);
                blockTagsInput = blockTags.join(", ");
            }
        } catch {}
    }
    function saveBlockTags() {
        blockTags = blockTagsInput.split(/[,\n]+/).map((t) => t.trim()).filter(Boolean);
        try { localStorage.setItem(BLOCK_TAGS_KEY, JSON.stringify(blockTags)); } catch {}
    }

    function toggleHide(/** @type {string} */ name) {
        if (hiddenAssets.has(name)) hiddenAssets.delete(name);
        else hiddenAssets.add(name);
        hiddenAssets = hiddenAssets;
        saveHiddenAssets();
        applyFilters();
    }

    function cycleSort(/** @type {string} */ key) {
        if (sortKey === key) {
            sortDir = sortDir === "asc" ? "desc" : "asc";
        } else {
            sortKey = key;
            sortDir = key === "balance" ? "desc" : "asc";
        }
        applySort();
    }

    function applySort() {
        if (!sortKey) return;
        const dir = sortDir === "asc" ? 1 : -1;
        displayResults = [...displayResults].sort((a, b) => {
            if (sortKey === "name") return dir * a.name.localeCompare(b.name);
            if (sortKey === "type") {
                const cmp = a.type.localeCompare(b.type);
                return cmp !== 0 ? dir * cmp : dir * a.name.localeCompare(b.name);
            }
            if (sortKey === "balance") return dir * (a.balance - b.balance);
            return 0;
        });
    }

    function sortIndicator(/** @type {string} */ key) {
        if (sortKey !== key) return "";
        return sortDir === "asc" ? " ▲" : " ▼";
    }

    function applyFilters() {
        let results = allResults;
        if (typeFilter !== "ALL") {
            results = results.filter((a) => a.type === typeFilter);
        }
        if (!showHidden) {
            results = results.filter((a) => !hiddenAssets.has(a.name));
        }
        if (tagFilterActive && tagBlockedNames.size > 0) {
            results = results.filter((a) => !tagBlockedNames.has(a.name));
        }
        displayResults = results;
        applySort();
        hasMore = allResults.length >= currentLimit && allResults.length < MAX_LIMIT;
    }

    function saveCache() {
        try {
            localStorage.setItem(BROWSE_CACHE_KEY, JSON.stringify({
                pattern: activePattern,
                results: allResults,
                limit: currentLimit,
                time: new Date().toLocaleTimeString(),
            }));
        } catch {}
    }

    function loadCache() {
        try {
            const raw = localStorage.getItem(BROWSE_CACHE_KEY);
            if (raw) {
                const cache = JSON.parse(raw);
                if (cache.results && cache.results.length > 0) {
                    allResults = cache.results;
                    activePattern = cache.pattern || "*";
                    searchInput = activePattern === "*" ? "" : activePattern;
                    currentLimit = cache.limit || 100;
                    lastFetchTime = cache.time || "";
                    searched = true;
                    cacheUsed = true;
                    applyFilters();
                    return true;
                }
            }
        } catch {}
        return false;
    }

    async function doSearch() {
        if (loading) return;
        loading = true;
        error = "";
        searched = true;
        cacheUsed = false;
        currentLimit = PAGE_SIZE;
        const pattern = searchInput.trim() || "*";
        activePattern = pattern;
        try {
            const raw = await core.invoke("list_network_assets", { pattern, verbose: true, limit: currentLimit });
            allResults = parseNetworkAssets(raw);
            lastFetchTime = new Date().toLocaleTimeString();
            saveCache();
            applyFilters();
            dispatch("loaded", { count: allResults.length });
        } catch (err) {
            error = String(err);
            allResults = [];
            displayResults = [];
        } finally {
            loading = false;
        }
    }

    async function loadMore() {
        if (loading || !hasMore) return;
        loading = true;
        currentLimit = Math.min(currentLimit + PAGE_SIZE, MAX_LIMIT);
        try {
            const raw = await core.invoke("list_network_assets", { pattern: activePattern, verbose: true, limit: currentLimit });
            allResults = parseNetworkAssets(raw);
            lastFetchTime = new Date().toLocaleTimeString();
            saveCache();
            applyFilters();
            dispatch("loaded", { count: allResults.length });
        } catch (err) {
            error = String(err);
        } finally {
            loading = false;
        }
    }

    $: canRefreshResults = searched || !!activePattern || !!lastFetchTime || allResults.length > 0;

    async function refreshResults() {
        if (loading) return;
        loading = true;
        error = "";
        cacheUsed = false;
        try {
            const raw = await core.invoke("list_network_assets", { pattern: activePattern || "*", verbose: true, limit: currentLimit });
            allResults = parseNetworkAssets(raw);
            lastFetchTime = new Date().toLocaleTimeString();
            saveCache();
            applyFilters();
            dispatch("loaded", { count: allResults.length });
        } catch (err) {
            error = String(err);
        } finally {
            loading = false;
        }
    }

    async function runTagBlockCheck() {
        if (tagCheckPending || blockTags.length === 0 || allResults.length === 0) return;
        tagCheckPending = true;
        tagBlockedNames = new Set();
        try {
            const assetNames = allResults
                .filter((a) => !a.name.endsWith("!") && !a.name.startsWith("#"))
                .map((a) => a.name);
            if (assetNames.length > 0) {
                const blocked = await core.invoke("h0xc_filter_tagged_channels", {
                    channelNames: assetNames.slice(0, 200),
                    tagNames: blockTags,
                });
                tagBlockedNames = new Set(Array.isArray(blocked) ? blocked : []);
            }
        } catch {}
        tagCheckPending = false;
        applyFilters();
    }

    /**
     * @param {any} raw
     * @returns {BrowseAsset[]}
     */
    function parseNetworkAssets(raw) {
        let obj = null;
        if (typeof raw === "string") {
            try { obj = JSON.parse(raw); } catch { return []; }
        } else if (typeof raw === "object" && raw !== null) {
            obj = raw;
        }
        if (!obj || typeof obj !== "object" || Array.isArray(obj)) return [];
        const results = [];
        for (const [name, value] of Object.entries(obj)) {
            if (!name || typeof name !== "string") continue;
            let balance = 0;
            let units, reissuable, has_ipfs;
            if (typeof value === "number") {
                balance = value;
            } else if (typeof value === "object" && value !== null) {
                const v = /** @type {any} */ (value);
                balance = typeof v.balance === "number" ? v.balance : typeof v.amount === "number" ? v.amount : 0;
                if (typeof v.units === "number") units = v.units;
                if (typeof v.reissuable === "boolean") reissuable = v.reissuable;
                if (typeof v.has_ipfs === "boolean") has_ipfs = v.has_ipfs;
            } else if (typeof value === "string") {
                balance = parseFloat(value) || 0;
            }
            results.push({ name, balance, type: classifyAssetType(name), units, reissuable, has_ipfs, raw: value });
        }
        results.sort((a, b) => a.name.localeCompare(b.name));
        return results;
    }

    function handleKeydown(/** @type {KeyboardEvent} */ e) {
        if (e.key === "Enter") doSearch();
    }

    function setFilter(/** @type {string} */ filter) {
        typeFilter = filter;
        applyFilters();
    }

    function viewAsset(/** @type {BrowseAsset} */ asset) {
        dispatch("viewAsset", {
            name: asset.name,
            balance: 0,
            type: asset.type,
            units: asset.units,
            isQualifier: asset.name.startsWith("#"),
            isRestricted: asset.name.startsWith("$"),
        });
    }

    function close() {
        dispatch("close");
    }

    loadHiddenAssets();
    loadBlockTags();
    if (!loadCache()) {
        searched = false;
    }
</script>

{#snippet panelContent()}
    <div class="modal-header">
        <div class="header-left">
            <h3>Browse Network Assets</h3>
            {#if lastFetchTime}
                <span class="header-time">{cacheUsed ? "cached" : "updated"} {lastFetchTime}</span>
            {/if}
        </div>
        <div class="header-right">
            {#if canRefreshResults}
                <button class="icon-btn" on:click={refreshResults} disabled={loading} title="Refresh from network">↻</button>
            {/if}
            <button class="icon-btn" on:click={() => (settingsOpen = !settingsOpen)} class:active={settingsOpen} title="Browse settings">⚙</button>
            <button class="close-btn" on:click={close}>&times;</button>
        </div>
    </div>

    <div class="modal-body">
        {#if settingsOpen}
            <div class="settings-popup" transition:fade={{ duration: 120 }}>
                <div class="settings-row">
                    <span class="settings-label">BLOCK TAGS</span>
                    <input
                        class="settings-input"
                        type="text"
                        bind:value={blockTagsInput}
                        on:blur={saveBlockTags}
                        on:keydown={(e) => e.key === "Enter" && (e.target?.blur())}
                        placeholder="#SPAM, #BAN"
                    />
                    <button
                        class="filter-chip"
                        class:active={tagFilterActive}
                        on:click={() => { tagFilterActive = !tagFilterActive; if (tagFilterActive) runTagBlockCheck(); else { tagBlockedNames = new Set(); applyFilters(); } }}
                        disabled={blockTags.length === 0}
                    >{tagCheckPending ? "..." : "TAG FILTER"}</button>
                </div>
                <div class="settings-toggles">
                    <label class="toggle-row">
                        <input type="checkbox" checked={showHidden} on:change={() => { showHidden = !showHidden; applyFilters(); }} />
                        <span class="toggle-label">SHOW HIDDEN ({hiddenAssets.size})</span>
                    </label>
                </div>
            </div>
        {/if}

        <div class="panel-body">
            <div class="search-row">
                <input class="search-input" type="text" bind:value={searchInput} on:keydown={handleKeydown}
                    placeholder="Asset name pattern (e.g. HEMP, #*, ASSET/SUB)" disabled={loading} />
                <button class="cyber-btn search-btn" on:click={doSearch} disabled={loading}>
                    {loading && !allResults.length ? "..." : "SEARCH"}
                </button>
            </div>

            <div class="filter-row">
                {#each TYPE_FILTERS as filter}
                    <button class="filter-chip" class:active={typeFilter === filter}
                        on:click={() => setFilter(filter)} disabled={loading && !allResults.length}>{filter}</button>
                {/each}
            </div>

            {#if error}
                <div class="error-state">
                    <span class="error-icon">⚠</span>
                    <span class="error-text">{error}</span>
                </div>
            {/if}

            {#if searched && !loading && allResults.length === 0 && !error}
                <div class="empty-state">
                    <div class="empty-icon">∅</div>
                    <div class="empty-text">No assets found</div>
                    <div class="empty-hint">Try a different search pattern or filter</div>
                </div>
            {:else if displayResults.length > 0}
                <div class="results-info">
                    <span class="results-count">{displayResults.length} asset{displayResults.length !== 1 ? "s" : ""}</span>
                    {#if typeFilter !== "ALL"}<span class="results-filter">type: {typeFilter}</span>{/if}
                    {#if tagFilterActive && tagBlockedNames.size > 0}<span class="results-filter">{tagBlockedNames.size} tag-blocked</span>{/if}
                </div>

                <div class="asset-table">
                    <div class="table-header">
                        <button class="col-header col-name" on:click={() => cycleSort("name")}>NAME{sortIndicator("name")}</button>
                        <button class="col-header col-type" on:click={() => cycleSort("type")}>TYPE{sortIndicator("type")}</button>
                        <button class="col-header col-balance" on:click={() => cycleSort("balance")}>SUPPLY{sortIndicator("balance")}</button>
                        <span class="col-actions"></span>
                    </div>
                    {#each displayResults as asset (asset.name)}
                        <div class="table-row" class:hidden-row={hiddenAssets.has(asset.name)}>
                            <button class="col-name" on:click={() => viewAsset(asset)} title="View {asset.name}">
                                {#if asset.name.length > 40}{asset.name.slice(0, 18)}…{asset.name.slice(-18)}{:else}{asset.name}{/if}
                            </button>
                            <span class="col-type">
                                <span class="type-badge" style="color: {typeColor(asset.type)}; border-color: {typeColor(asset.type)}33;">{typeLabel(asset.type)}</span>
                            </span>
                            <span class="col-balance mono">{formatBalance(asset.balance)}</span>
                            {#if settingsOpen}
                                <span class="col-actions">
                                    <button class="hide-toggle" class:hidden={hiddenAssets.has(asset.name)}
                                        on:click|stopPropagation={() => toggleHide(asset.name)}
                                        title={hiddenAssets.has(asset.name) ? "Unhide" : "Hide"}>
                                        <img src={hiddenAssets.has(asset.name) ? eyeClosed : eyeOpen} alt="hide" class="hide-eye" />
                                    </button>
                                </span>
                            {:else}
                                <span class="col-actions"></span>
                            {/if}
                        </div>
                    {/each}
                </div>

                {#if hasMore}
                    <div class="load-more-row">
                        <button class="cyber-btn small" on:click={loadMore} disabled={loading}>
                            {loading ? "Loading..." : `Load More (${allResults.length} of ~${currentLimit})`}
                        </button>
                    </div>
                {/if}
            {:else if !searched && !loading}
                <div class="empty-state">
                    <div class="empty-icon">🔍</div>
                    <div class="empty-text">Search the Hemp0x network</div>
                    <div class="empty-hint">Enter a pattern or leave blank for all assets</div>
                </div>
            {/if}

            {#if loading && allResults.length > 0}
                <div class="loading-bar">Loading...</div>
            {/if}
        </div>
    </div>
{/snippet}

{#if isOpen}
    {#if inline}
        <div class="browse-panel" in:fade={{ duration: 150 }}>
            {@render panelContent()}
        </div>
    {:else}
        <div class="modal-backdrop" role="button" tabindex="0" on:click={close}
            on:keydown={(e) => e.key === "Escape" && close()} transition:fade={{ duration: 200 }}>
            <div class="modal glass-panel" role="dialog" aria-modal="true" tabindex="-1"
                on:click|stopPropagation on:keydown={() => {}} transition:fly={{ y: 20, duration: 200 }}>
                {@render panelContent()}
            </div>
        </div>
    {/if}
{/if}

<style>
    .modal-backdrop {
        position: fixed; inset: 0; background: rgba(0, 0, 0, 0.85);
        display: flex; align-items: center; justify-content: center;
        padding: 0.75rem; z-index: 200000; backdrop-filter: blur(5px); box-sizing: border-box;
    }
    .modal {
        width: min(640px, 92vw); max-width: 92vw; max-height: min(44rem, calc(100dvh - 2rem));
        border: 1px solid rgba(0, 255, 65, 0.2); box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
        border-radius: 8px; overflow: hidden; display: flex; flex-direction: column;
        background: rgba(2, 4, 3, 0.98);
    }
    .modal-header {
        display: flex; justify-content: space-between; align-items: center;
        padding: 0.5rem 1rem 0.65rem; background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1); flex-shrink: 0;
    }
    .header-left { display: flex; align-items: center; gap: 0.6rem; }
    .header-right { display: flex; align-items: center; gap: 0.3rem; }
    .header-time { font-size: 0.55rem; color: #555; font-family: var(--font-mono); }
    .modal-header h3 {
        margin: 0; color: var(--color-primary);
        font-size: 0.9rem; letter-spacing: 1px;
    }
    .close-btn {
        background: none; border: none; color: #888; font-size: 1.3rem;
        cursor: pointer; transition: all 0.15s; padding: 0.15rem 0.4rem; line-height: 1;
    }
    .close-btn:hover { color: #fff; }
    .icon-btn {
        background: none; border: 1px solid rgba(255, 255, 255, 0.08); color: #888;
        font-size: 0.8rem; cursor: pointer; transition: all 0.15s;
        padding: 0.15rem 0.4rem; line-height: 1; border-radius: 4px;
    }
    .icon-btn:hover:not(:disabled) { border-color: var(--color-primary); color: var(--color-primary); }
    .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }
    .icon-btn.active { border-color: var(--color-primary); color: var(--color-primary); background: rgba(0, 255, 65, 0.08); }

    .modal-body {
        padding: 0.6rem 0.9rem; overflow-y: auto; overflow-x: hidden;
        flex: 1 1 0%; scrollbar-width: thin; scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .modal-body::-webkit-scrollbar { width: 8px; }
    .modal-body::-webkit-scrollbar-track { background: transparent; }
    .modal-body::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.35); border-radius: 4px; }
    .modal-body::-webkit-scrollbar-thumb:hover { background: rgba(0, 255, 65, 0.55); }

    .panel-body { display: flex; flex-direction: column; gap: 0.4rem; padding-bottom: 1.2rem; }

    .settings-popup {
        background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px; padding: 0.5rem 0.7rem; margin-bottom: 0.4rem;
        display: flex; flex-direction: column; gap: 0.4rem;
    }
    .settings-row { display: flex; align-items: center; gap: 0.4rem; }
    .settings-label {
        font-size: 0.5rem; color: #555; font-weight: 600;
        letter-spacing: 0.5px; white-space: nowrap;
    }
    .settings-input {
        flex: 1; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 3px; padding: 0.2rem 0.4rem; color: #aaa;
        font-family: var(--font-mono); font-size: 0.6rem; outline: none;
    }
    .settings-input:focus { border-color: rgba(255, 85, 85, 0.4); }
    .settings-toggles { display: flex; gap: 1rem; }
    .toggle-row {
        display: flex; align-items: center; gap: 0.4rem; cursor: pointer;
        font-size: 0.55rem; color: #777;
    }
    .toggle-row input[type="checkbox"] {
        width: 12px; height: 12px; accent-color: var(--color-primary); cursor: pointer;
    }
    .toggle-label { font-weight: 600; letter-spacing: 0.5px; }

    .search-row { display: flex; gap: 0.4rem; align-items: stretch; }
    .search-input {
        flex: 1; background: rgba(0, 0, 0, 0.4); border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 4px; padding: 0.45rem 0.7rem; color: #ddd;
        font-family: var(--font-mono); font-size: 0.75rem; outline: none; transition: border-color 0.15s;
    }
    .search-input:focus { border-color: var(--color-primary); }
    .search-input::placeholder { color: #555; }
    .search-btn { flex-shrink: 0; padding: 0.45rem 0.9rem; font-size: 0.7rem; }

    .filter-row { display: flex; gap: 0.3rem; flex-wrap: wrap; }
    .filter-chip {
        background: rgba(255, 255, 255, 0.03); border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px; color: #666; padding: 0.25rem 0.55rem;
        font-size: 0.6rem; font-weight: 600; letter-spacing: 0.5px; cursor: pointer; transition: all 0.15s;
    }
    .filter-chip:hover { border-color: rgba(0, 255, 65, 0.3); color: #aaa; }
    .filter-chip.active { background: rgba(0, 255, 65, 0.1); border-color: var(--color-primary); color: var(--color-primary); }

    .error-state {
        display: flex; align-items: flex-start; gap: 0.5rem; padding: 0.6rem 0.8rem;
        background: rgba(255, 50, 50, 0.08); border: 1px solid rgba(255, 50, 50, 0.2); border-radius: 4px;
    }
    .error-icon { color: #ff5555; flex-shrink: 0; }
    .error-text { color: #ff8888; font-size: 0.72rem; word-break: break-word; }

    .empty-state {
        text-align: center; padding: 3rem 2rem; color: #444;
        display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 0.5rem;
    }
    .empty-icon { font-size: 2rem; opacity: 0.4; }
    .empty-text { font-size: 0.85rem; color: #555; letter-spacing: 1px; }
    .empty-hint { font-size: 0.7rem; color: #444; }

    .results-info { display: flex; align-items: center; gap: 0.6rem; font-size: 0.65rem; color: #555; }
    .results-count { color: var(--color-primary); font-weight: 600; }
    .results-filter { color: #666; }

    .asset-table {
        display: flex; flex-direction: column; border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px; overflow: hidden;
    }
    .table-header {
        display: grid; grid-template-columns: 1fr 70px 110px 32px;
        gap: 0.4rem; padding: 0.35rem 0.7rem; background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .col-header {
        background: none; border: none; padding: 0; margin: 0;
        font-size: 0.55rem; font-weight: 600; color: #555; letter-spacing: 1px;
        text-transform: uppercase; cursor: pointer; transition: color 0.15s;
        text-align: left; font-family: var(--font-ui);
    }
    .col-header:hover { color: var(--color-primary); }
    .col-header.col-balance { text-align: right; }
    .table-row {
        display: grid; grid-template-columns: 1fr 70px 110px 32px;
        gap: 0.4rem; padding: 0.35rem 0.7rem; background: transparent; border: none;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03); color: #ccc;
        font-size: 0.72rem; text-align: left; transition: background 0.1s; width: 100%;
    }
    .table-row:last-child { border-bottom: none; }
    .table-row:hover { background: rgba(0, 255, 65, 0.04); }
    .table-row.hidden-row { opacity: 0.45; }
    .col-name {
        overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
        font-family: var(--font-mono); font-size: 0.68rem; background: none;
        border: none; color: #ccc; cursor: pointer; padding: 0; text-align: left; min-width: 0;
    }
    .col-name:hover { color: var(--color-primary); }
    .col-type { display: flex; align-items: center; }
    .type-badge {
        font-size: 0.5rem; font-weight: 600; letter-spacing: 0.5px;
        padding: 0.1rem 0.35rem; border: 1px solid; border-radius: 3px; white-space: nowrap;
    }
    .col-balance { text-align: right; font-size: 0.68rem; color: #999; display: flex; align-items: center; justify-content: flex-end; }
    .col-actions { display: flex; align-items: center; justify-content: center; }
    .hide-toggle {
        background: none; border: 1px solid rgba(255, 255, 255, 0.06); color: #555; cursor: pointer;
        padding: 0.15rem; line-height: 1; transition: all 0.15s; border-radius: 4px;
    }
    .hide-toggle:hover { border-color: rgba(255, 170, 0, 0.3); }
    .hide-toggle.hidden { border-color: rgba(255, 170, 0, 0.3); }
    .hide-eye { width: 16px; height: 16px; display: block; filter: brightness(0.6); }
    .hide-toggle.hidden .hide-eye { filter: brightness(0.8) sepia(1) hue-rotate(10deg) saturate(3); }
    .mono { font-family: var(--font-mono); }

    .load-more-row { display: flex; justify-content: center; padding: 0.5rem 0; }
    .loading-bar {
        text-align: center; padding: 0.4rem; color: var(--color-primary);
        font-size: 0.7rem; opacity: 0.7; background: rgba(0, 255, 65, 0.03); border-radius: 4px;
    }
    .browse-panel { flex: 1; min-height: 0; display: flex; flex-direction: column; }
</style>
