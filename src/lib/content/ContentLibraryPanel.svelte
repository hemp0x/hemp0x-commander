<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { contentLibrary, libraryLoading, activePanel, searchQuery, statusFilter, filteredPackages, sortByUpdatedDesc, ipfsHubSection } from "../stores/contentLibrary.js";
    import PackageCard from "./PackageCard.svelte";
    import PackageComposer from "./PackageComposer.svelte";
    import ContentRenderer from "./ContentRenderer.svelte";

    const dispatch = createEventDispatcher();
    let errorMsg = "";
    let detailPackage = null;
    let detailFull = null;
    let detailMarkdownBody = "";
    let detailMarkdownLoading = false;
    let detailCacheExists = false;
    let detailFetchState = "idle";
    let detailFetchResult = null;
    let detailFetchError = "";
    let detailConsentGiven = false;
    let detailAttachments = [];

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
        detailFull = null;
    }

    function showBrowse() {
        $activePanel = "browse";
        detailPackage = null;
        detailFull = null;
    }

    async function showDetail(pkg) {
        detailPackage = pkg;
        detailFull = null;
        detailMarkdownBody = "";
        detailAttachments = [];
        detailFetchState = "idle";
        detailFetchResult = null;
        detailFetchError = "";
        detailConsentGiven = false;
        $activePanel = "detail";

        try {
            detailFull = await core.invoke("content_library_get", { packageId: pkg.id });
            if (detailFull.files) {
                detailAttachments = detailFull.files.filter((f) => f.path !== "content.md");
                const mdFile = detailFull.files.find((f) => f.path === "content.md");
                if (mdFile) {
                    detailMarkdownLoading = true;
                    try {
                        const result = await core.invoke("content_library_get_file", {
                            packageId: pkg.id,
                            filePath: "content.md",
                        });
                        const bytes = Uint8Array.from(atob(result.content_base64), (c) => c.charCodeAt(0));
                        detailMarkdownBody = new TextDecoder().decode(bytes);
                    } catch {
                        detailMarkdownBody = "";
                    }
                    detailMarkdownLoading = false;
                }
            }
            if (detailFull.cid) {
                detailCacheExists = await core.invoke("content_library_has_cache", { cid: detailFull.cid });
            }
        } catch (err) {
            errorMsg = String(err);
        }
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

    function showCidViewer(cid) {
        $ipfsHubSection = "cid-viewer";
    }

    function bytesToBase64(bytes) {
        let binary = "";
        const chunkSize = 0x8000;
        for (let i = 0; i < bytes.length; i += chunkSize) {
            const chunk = bytes.subarray(i, i + chunkSize);
            binary += String.fromCharCode.apply(null, chunk);
        }
        return btoa(binary);
    }

    function textToBase64(text) {
        return bytesToBase64(new TextEncoder().encode(text || ""));
    }

    function formatSize(bytes) {
        if (!bytes) return "0 B";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    }

    function isImageMime(mime) {
        return mime && mime.startsWith("image/");
    }

    function isPreviewable(mime) {
        return mime && (
            mime.startsWith("image/") ||
            mime.startsWith("text/") ||
            mime === "application/json"
        );
    }

    async function copyFilename(path) {
        try {
            await navigator.clipboard.writeText(path);
        } catch {}
    }

    async function fetchDetailCid() {
        if (!detailFull || !detailFull.cid) return;
        if (!detailCacheExists && !detailConsentGiven) {
            detailFetchState = "consenting";
            return;
        }
        detailFetchError = "";
        detailFetchState = "fetching";
        try {
            if (detailCacheExists) {
                detailFetchResult = await core.invoke("content_library_get_cached", { cid: detailFull.cid });
                detailFetchState = "cached";
            } else {
                detailFetchResult = await core.invoke("content_library_fetch_cid", { cid: detailFull.cid, gatewayIndex: null });
                detailFetchState = "fetched";
                detailCacheExists = true;
            }
        } catch (err) {
            detailFetchState = "error";
            detailFetchError = String(err);
        }
    }

    function grantDetailConsent() {
        detailConsentGiven = true;
        fetchDetailCid();
    }

    function denyDetailConsent() {
        detailConsentGiven = false;
        detailFetchState = "idle";
    }

    function refreshDetailCid() {
        detailFetchState = "fetching";
        (async () => {
            try {
                detailFetchResult = await core.invoke("content_library_refresh_cached", { cid: detailFull.cid, gatewayIndex: null });
                detailFetchState = "fetched";
                detailCacheExists = true;
            } catch (err) {
                detailFetchState = "error";
                detailFetchError = String(err);
            }
        })();
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

                <div class="detail-name-row">
                    <h3 class="detail-name">{detailFull ? detailFull.name : detailPackage.name}</h3>
                    <span class="detail-status-badge" class:status-external={detailPackage.status === "external"}
                        class:status-published={detailPackage.status === "published"}
                        class:status-local={!detailPackage.status || detailPackage.status === "local"}>
                        {detailPackage.status === "external" ? "External CID" : detailPackage.status === "published" ? "Published" : "Local Only"}
                    </span>
                </div>

                {#if detailFull && detailFull.description}
                    <p class="detail-desc">{detailFull.description}</p>
                {/if}

                <div class="detail-meta">
                    <div class="detail-meta-item">
                        <span class="detail-meta-label">Version</span>
                        <span class="detail-meta-value">v{detailPackage.version}</span>
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
                    {#if detailFull && detailFull.cid}
                        <div class="detail-meta-item wide">
                            <span class="detail-meta-label">CID</span>
                            <span class="detail-meta-value mono cid-value">{detailFull.cid}</span>
                            <button class="copy-btn" on:click={async () => { try { await navigator.clipboard.writeText(detailFull.cid); } catch {} }}>
                                COPY
                            </button>
                            {#if detailPackage.status === "external"}
                                {#if !detailFetchResult && detailFetchState !== "fetching"}
                                    {#if detailCacheExists}
                                        <button class="cyber-btn small" on:click={fetchDetailCid}>
                                            OPEN CACHED
                                        </button>
                                    {:else}
                                        <button class="cyber-btn small" on:click={fetchDetailCid}>
                                            FETCH
                                        </button>
                                    {/if}
                                {:else if detailFetchState === "fetching"}
                                    <button class="cyber-btn small" disabled>LOADING...</button>
                                {:else if detailFetchResult}
                                    <button class="cyber-btn small ghost" on:click={refreshDetailCid}>
                                        REFRESH
                                    </button>
                                {/if}
                            {/if}
                        </div>
                    {/if}
                    {#if detailFull && detailFull.provider}
                        <div class="detail-meta-item">
                            <span class="detail-meta-label">Provider</span>
                            <span class="detail-meta-value">{detailFull.provider}</span>
                        </div>
                    {/if}
                    {#if detailFull && detailFull.published_at}
                        <div class="detail-meta-item">
                            <span class="detail-meta-label">Published</span>
                            <span class="detail-meta-value">{(detailFull.published_at || "").slice(0, 10)}</span>
                        </div>
                    {/if}
                </div>

                {#if detailFull && detailFull.tags && detailFull.tags.length > 0}
                    <div class="detail-tags">
                        {#each detailFull.tags as tag}
                            <span class="tag-chip">{tag}</span>
                        {/each}
                    </div>
                {/if}

                {#if detailFetchError}
                    <div class="error-bar" style="margin-top:0.5rem;">{detailFetchError}</div>
                {/if}

                {#if detailFetchState === "fetching"}
                    <div class="fetching-indicator">
                        Fetching from gateway...
                    </div>
                {/if}

                {#if detailFetchState === "consenting"}
                    <div class="gateway-consent-panel">
                        <div class="section-label">GATEWAY FETCH NOTICE</div>
                        <p>
                            Public gateway requests reveal this CID and your IP address to the gateway operator.
                            The content is cached locally after fetch.
                        </p>
                        <div class="consent-actions">
                            <button class="cyber-btn small" on:click={grantDetailConsent}>FETCH CONTENT</button>
                            <button class="cyber-btn small ghost" on:click={denyDetailConsent}>CANCEL</button>
                        </div>
                    </div>
                {/if}

                {#if detailFetchResult && (detailFetchState === "fetched" || detailFetchState === "cached")}
                    <div class="detail-fetched-section">
                        <div class="section-label">
                            FETCHED CONTENT
                            <span class="section-badge" style="color:{detailFetchState === 'cached' ? '#cca' : 'var(--color-primary)'};">
                                {detailFetchState === "cached" ? "CACHED" : "FRESH"}
                            </span>
                        </div>
                        <ContentRenderer
                            contentBase64={detailFetchResult.content_base64}
                            contentType={detailFetchResult.content_type}
                            sizeBytes={detailFetchResult.size_bytes}
                        />
                    </div>
                {/if}

                {#if detailMarkdownBody || detailMarkdownLoading}
                    <div class="detail-markdown-section">
                        <div class="section-label">PACKAGE CONTENT</div>
                        {#if detailMarkdownLoading}
                            <div class="fetching-indicator">Loading content...</div>
                        {:else}
                            <ContentRenderer
                                contentBase64={detailMarkdownBody ? textToBase64(detailMarkdownBody) : ""}
                                contentType="text/markdown"
                                sizeBytes={new TextEncoder().encode(detailMarkdownBody || "").length}
                            />
                        {/if}
                    </div>
                {/if}

                {#if detailAttachments.length > 0}
                    <div class="detail-attachments-section">
                        <div class="section-label">ATTACHMENTS</div>
                        <div class="attachment-list">
                            {#each detailAttachments as file}
                                <div class="attachment-row">
                                    <span class="attachment-icon">{isImageMime(file.mime) ? "img" : "doc"}</span>
                                    <span class="attachment-name mono">{file.path}</span>
                                    <span class="attachment-size">{formatSize(file.size_bytes)}</span>
                                    <span class="attachment-mime">{(file.mime || "").split("/")[1] || file.mime}</span>
                                    {#if isPreviewable(file.mime)}
                                        <button class="attachment-btn" on:click={async () => {
                                            try {
                                                const result = await core.invoke("content_library_get_file", {
                                                    packageId: detailFull.id,
                                                    filePath: file.path,
                                                });
                                                detailFetchResult = {
                                                    cid: "",
                                                    gateway_used: "",
                                                    content_type: result.mime,
                                                    size_bytes: result.size_bytes,
                                                    fetched_at: "",
                                                    local_path: "",
                                                    content_base64: result.content_base64,
                                                };
                                                detailFetchState = "cached";
                                            } catch (e) {
                                                detailFetchError = String(e);
                                                detailFetchState = "error";
                                            }
                                        }}>
                                            PREVIEW
                                        </button>
                                    {/if}
                                    <button class="attachment-btn" on:click={() => copyFilename(file.path)}>
                                        COPY
                                    </button>
                                </div>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if detailFull && detailFull.cid}
                    <div class="detail-section">
                        <div class="section-label">LINK STATUS</div>
                        <div class="link-status-row">
                            <span class="link-label">CID:</span>
                            <span class="mono link-value">{detailFull.cid}</span>
                            <span class="link-provider">({detailFull.provider || "manual"})</span>
                        </div>
                        {#if detailCacheExists}
                            <div class="link-cached">Cached locally</div>
                        {:else if detailPackage.status === "external"}
                            <div class="link-not-cached" style="color:#cca;">Not fetched yet. Click FETCH above.</div>
                        {/if}
                    </div>
                {/if}

                <div class="detail-section">
                    <div class="section-label">PACKAGE HISTORY</div>
                    <div class="history-placeholder">
                        Version history will be shown here in a future update.
                    </div>
                </div>

                <div class="detail-actions">
                    <button class="cyber-btn" on:click={() => { $activePanel = detailPackage.id; }}>
                        EDIT PACKAGE
                    </button>
                    {#if detailPackage.cid}
                        <button class="cyber-btn ghost" on:click={() => { $ipfsHubSection = "cid-viewer"; }}>
                            VIEW IN CID VIEWER
                        </button>
                    {/if}
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
        max-width: 800px;
    }
    .detail-header {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }
    .detail-name-row {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        margin-bottom: 0.5rem;
    }
    .detail-name {
        font-size: 1rem;
        color: #fff;
        margin: 0;
        letter-spacing: 0.5px;
    }
    .detail-status-badge {
        font-size: 0.55rem;
        padding: 2px 8px;
        border-radius: 4px;
        letter-spacing: 0.5px;
        font-weight: 600;
    }
    .status-local {
        background: rgba(255, 255, 255, 0.06);
        color: #777;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }
    .status-external {
        background: rgba(255, 165, 0, 0.08);
        color: #cca;
        border: 1px solid rgba(255, 165, 0, 0.2);
    }
    .status-published {
        background: rgba(0, 255, 65, 0.08);
        color: var(--color-primary);
        border: 1px solid rgba(0, 255, 65, 0.2);
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
    .cid-value {
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
        font-size: 0.6rem;
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
    .detail-section {
        margin-top: 1rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .section-label {
        font-size: 0.65rem;
        color: var(--color-primary);
        letter-spacing: 1.5px;
        margin-bottom: 0.5rem;
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .section-badge {
        font-size: 0.55rem;
        letter-spacing: 0.5px;
    }
    .detail-markdown-section {
        margin-top: 1rem;
    }
    .detail-fetched-section {
        margin-top: 1rem;
    }
    .gateway-consent-panel {
        margin-top: 0.75rem;
        padding: 0.75rem;
        background: rgba(255, 165, 0, 0.06);
        border: 1px solid rgba(255, 165, 0, 0.2);
        border-radius: 6px;
        color: #aaa;
        font-size: 0.65rem;
        line-height: 1.45;
    }
    .gateway-consent-panel p {
        margin: 0 0 0.6rem 0;
    }
    .consent-actions {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .detail-attachments-section {
        margin-top: 1rem;
    }
    .attachment-list {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .attachment-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.65rem;
    }
    .attachment-icon {
        font-size: 0.5rem;
        color: #555;
        width: 24px;
        text-align: center;
        flex-shrink: 0;
    }
    .attachment-name {
        color: #aaa;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }
    .attachment-size {
        color: #555;
        font-size: 0.55rem;
        flex-shrink: 0;
    }
    .attachment-mime {
        color: #444;
        font-size: 0.5rem;
        flex-shrink: 0;
    }
    .attachment-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
        font-size: 0.5rem;
        padding: 1px 5px;
        border-radius: 3px;
        cursor: pointer;
        flex-shrink: 0;
    }
    .attachment-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .link-status-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.6rem;
    }
    .link-label {
        color: #555;
    }
    .link-value {
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }
    .link-provider {
        color: #666;
    }
    .link-cached {
        margin-top: 0.35rem;
        color: var(--color-primary);
        font-size: 0.6rem;
    }
    .link-not-cached {
        margin-top: 0.35rem;
        font-size: 0.6rem;
    }
    .history-placeholder {
        padding: 0.5rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px dashed rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        font-size: 0.6rem;
        color: #555;
    }
    .fetching-indicator {
        font-size: 0.65rem;
        color: #aaa;
        padding: 0.5rem 0;
        text-align: center;
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
    .cyber-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
    }
    .cyber-btn.small {
        padding: 0.2rem 0.5rem;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
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
    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
</style>
