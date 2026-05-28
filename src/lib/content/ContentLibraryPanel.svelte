<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { contentLibrary, libraryLoading, activePanel, searchQuery, statusFilter, filteredPackages, sortByUpdatedDesc } from "../stores/contentLibrary.js";
    import PackageCard from "./PackageCard.svelte";
    import PackageComposer from "./PackageComposer.svelte";

    const dispatch = createEventDispatcher();
    let errorMsg = "";
    let detailPackage = null;

    onMount(async () => {
        await refresh();
    });

    async function refresh() {
        $libraryLoading = true;
        errorMsg = "";
        try {
            $contentLibrary = await core.invoke("content_library_list");
        } catch (err) {
            errorMsg = String(err);
        }
        $libraryLoading = false;
    }

    function showCreate() {
        $activePanel = "create";
        detailPackage = null;
    }

    function showBrowse() {
        $activePanel = "browse";
        detailPackage = null;
    }

    function showDetail(pkg) {
        detailPackage = pkg;
        $activePanel = "detail";
    }

    function onView(e) {
        const id = e.detail || (e.target && e.target.pkg && e.target.pkg.id);
    }

    function onPackageSaved() {
        showBrowse();
        refresh();
    }

    function onPackageImported() {
        refresh();
    }

    function handleCardView(event) {
        const pkgId = event.target?.closest?.("[data-pkg-id]")?.dataset?.pkgId;
        if (pkgId) {
            const pkg = $contentLibrary.find((p) => p.id === pkgId);
            if (pkg) showDetail(pkg);
        }
    }
</script>

<div class="content-library">
    <header class="library-header">
        <div class="header-left">
            <span class="header-title">CONTENT LIBRARY</span>
            <span class="header-count mono">{$filteredPackages.length} packages</span>
        </div>
        <div class="header-actions">
            {#if $activePanel !== "browse"}
                <button class="header-btn" on:click={showBrowse}>
                    BACK TO LIBRARY
                </button>
            {/if}
            {#if $activePanel === "browse"}
                <button class="header-btn create-btn" on:click={showCreate}>
                    + NEW PACKAGE
                </button>
            {/if}
            <button class="header-btn refresh-btn" on:click={refresh} disabled={$libraryLoading}>
                {$libraryLoading ? "LOADING..." : "REFRESH"}
            </button>
        </div>
    </header>

    {#if $activePanel === "browse"}
        <div class="filter-bar">
            <div class="search-group">
                <input
                    class="search-input mono"
                    type="text"
                    placeholder="Search by name, description, tag, CID..."
                    bind:value={$searchQuery}
                />
            </div>
            <div class="filter-btns">
                <button class="filter-btn" class:active={$statusFilter === "all"} on:click={() => ($statusFilter = "all")}>
                    ALL
                </button>
                <button class="filter-btn" class:active={$statusFilter === "local"} on:click={() => ($statusFilter = "local")}>
                    LOCAL
                </button>
                <button class="filter-btn" class:active={$statusFilter === "external"} on:click={() => ($statusFilter = "external")}>
                    EXTERNAL
                </button>
                <button class="filter-btn" class:active={$statusFilter === "published"} on:click={() => ($statusFilter = "published")}>
                    PUBLISHED
                </button>
            </div>
        </div>
    {/if}

    <div class="library-body">
        {#if errorMsg}
            <div class="error-bar">{errorMsg}</div>
        {/if}

        {#if $activePanel === "browse"}
            {#if $libraryLoading}
                <div class="empty-state">Loading packages...</div>
            {:else if $filteredPackages.length === 0}
                <div class="empty-state">
                    <div class="empty-icon">◈</div>
                    <div class="empty-text">
                        {#if $searchQuery || $statusFilter !== "all"}
                            No packages match your filters
                        {:else}
                            No packages in library
                        {/if}
                    </div>
                    {#if !$searchQuery && $statusFilter === "all"}
                        <button class="cyber-btn" on:click={showCreate}>CREATE YOUR FIRST PACKAGE</button>
                    {:else}
                        <button class="cyber-btn ghost" on:click={() => { $searchQuery = ""; $statusFilter = "all"; }}>
                            CLEAR FILTERS
                        </button>
                    {/if}
                </div>
            {:else}
                <div class="package-grid">
                    {#each $filteredPackages as pkg (pkg.id)}
                        <div in:fly={{ y: 10, duration: 200 }}>
                            <PackageCard {pkg} on:refresh={refresh} on:edit={() => { $activePanel = pkg.id; }} on:view={() => showDetail(pkg)} />
                        </div>
                    {/each}
                </div>
            {/if}
        {:else if $activePanel === "create"}
            <PackageComposer on:saved={onPackageSaved} on:cancel={showBrowse} />
        {:else if $activePanel === "detail" && detailPackage}
            <div class="detail-panel" in:fly={{ y: 10, duration: 150 }}>
                <div class="detail-header">
                    <button class="header-btn" on:click={showBrowse}>BACK</button>
                    <button class="header-btn" on:click={() => { $activePanel = detailPackage.id; }}>
                        EDIT
                    </button>
                </div>
                <h3 class="detail-name">{detailPackage.name}</h3>
                {#if detailPackage.description}
                    <p class="detail-desc">{detailPackage.description}</p>
                {/if}
                <div class="detail-meta">
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Version</span>
                        <span class="detail-meta-value">v{detailPackage.version}</span>
                    </div>
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Status</span>
                        <span class="detail-meta-value status-{detailPackage.status || 'local'}">
                            {detailPackage.status === "external" ? "External CID" : detailPackage.status === "published" ? "Published" : "Local Only"}
                        </span>
                    </div>
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Files</span>
                        <span class="detail-meta-value">{detailPackage.file_count}</span>
                    </div>
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Created</span>
                        <span class="detail-meta-value">{(detailPackage.created_at || "").slice(0, 10)}</span>
                    </div>
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Updated</span>
                        <span class="detail-meta-value">{(detailPackage.updated_at || "").slice(0, 10)}</span>
                    </div>
                    {#if detailPackage.cid}
                        <div class="detail-meta-item wide">
                            <span class="detail-meta-label">CID</span>
                            <span class="detail-meta-value mono cid-value">{detailPackage.cid}</span>
                            <button class="copy-btn" on:click={async () => { try { await navigator.clipboard.writeText(detailPackage.cid); } catch {} }}>
                                COPY
                            </button>
                        </div>
                    {/if}
                    {#if detailPackage.provider}
                        <div class="detail-meta-item">
                            <span class="detail-meta-label">Provider</span>
                            <span class="detail-meta-value">{detailPackage.provider}</span>
                        </div>
                    {/if}
                </div>
                {#if detailPackage.tags && detailPackage.tags.length > 0}
                    <div class="detail-tags">
                        {#each detailPackage.tags as tag}
                            <span class="tag-chip">{tag}</span>
                        {/each}
                    </div>
                {/if}
                <div class="detail-actions">
                    <button class="cyber-btn" on:click={() => { $activePanel = detailPackage.id; }}>
                        EDIT PACKAGE
                    </button>
                    <button class="cyber-btn ghost" on:click={showBrowse}>
                        BACK TO LIBRARY
                    </button>
                </div>
            </div>
        {:else}
            {#each $contentLibrary as pkg (pkg.id)}
                {#if $activePanel === pkg.id}
                    <PackageComposer editPackage={pkg} on:saved={onPackageSaved} on:cancel={showBrowse} />
                {/if}
            {/each}
        {/if}
    </div>
</div>

<style>
    .content-library {
        flex: 1;
        display: flex;
        flex-direction: column;
        min-height: 0;
    }
    .library-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0 0 0.75rem 0;
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }
    .header-left {
        display: flex;
        align-items: baseline;
        gap: 0.75rem;
    }
    .header-title {
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 2px;
    }
    .header-count {
        font-size: 0.7rem;
        color: #555;
    }
    .header-actions {
        display: flex;
        gap: 0.5rem;
    }
    .header-btn {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        padding: 0.4rem 0.8rem;
        font-size: 0.65rem;
        font-weight: 600;
        letter-spacing: 1px;
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .header-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.2);
    }
    .header-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .filter-bar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.8rem;
        padding: 0.6rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        flex-shrink: 0;
        flex-wrap: wrap;
    }
    .search-group {
        flex: 1;
        min-width: 180px;
    }
    .search-input {
        width: 100%;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.35rem 0.6rem;
        border-radius: 4px;
        font-size: 0.7rem;
        outline: none;
        box-sizing: border-box;
    }
    .search-input:focus {
        border-color: var(--color-primary);
    }
    .search-input::placeholder {
        color: #444;
    }
    .filter-btns {
        display: flex;
        gap: 0.3rem;
    }
    .filter-btn {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: #555;
        padding: 0.3rem 0.6rem;
        font-size: 0.55rem;
        letter-spacing: 1px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .filter-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .filter-btn.active {
        background: rgba(0, 255, 65, 0.08);
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .library-body {
        flex: 1;
        overflow-y: auto;
        padding: 1rem 0;
    }
    .package-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
        gap: 1rem;
    }
    .empty-state {
        text-align: center;
        padding: 3rem 2rem;
        color: #555;
    }
    .empty-icon {
        font-size: 2.5rem;
        margin-bottom: 0.75rem;
        opacity: 0.3;
    }
    .empty-text {
        font-size: 0.9rem;
        margin-bottom: 1.5rem;
    }
    .error-bar {
        padding: 0.5rem 1rem;
        background: rgba(255, 68, 68, 0.1);
        border: 1px solid rgba(255, 68, 68, 0.3);
        color: #ff6666;
        font-size: 0.75rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        font-family: var(--font-mono);
    }
    .detail-panel {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1.2rem;
        max-width: 700px;
    }
    .detail-header {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }
    .detail-name {
        font-size: 1rem;
        color: #fff;
        margin: 0 0 0.5rem 0;
        letter-spacing: 0.5px;
    }
    .detail-desc {
        font-size: 0.8rem;
        color: #888;
        margin: 0 0 1rem 0;
        line-height: 1.4;
    }
    .detail-meta {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
        margin-bottom: 0.8rem;
    }
    .detail-meta-item {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.35rem 0.6rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.65rem;
    }
    .detail-meta-item.wide {
        flex-basis: 100%;
    }
    .detail-meta-label {
        color: #555;
        letter-spacing: 0.5px;
    }
    .detail-meta-value {
        color: #aaa;
    }
    .detail-meta-value.status-external {
        color: #cca;
    }
    .detail-meta-value.status-published {
        color: var(--color-primary);
    }
    .cid-value {
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }
    .copy-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
        font-size: 0.5rem;
        padding: 2px 6px;
        border-radius: 3px;
        cursor: pointer;
    }
    .copy-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .detail-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 0.3rem;
        margin-bottom: 1rem;
    }
    .tag-chip {
        font-size: 0.55rem;
        padding: 2px 6px;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.15);
        color: var(--color-primary);
        border-radius: 3px;
    }
    .detail-actions {
        display: flex;
        gap: 0.5rem;
        padding-top: 0.8rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
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
    }
    .cyber-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
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
