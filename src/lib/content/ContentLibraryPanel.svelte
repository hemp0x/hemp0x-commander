<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { contentLibrary, libraryLoading, activePanel, searchQuery, statusFilter, currentFolder, filteredPackages, folderGroups, ipfsHubSection, packageSortMode } from "../stores/contentLibrary.js";
    import PackageCard from "./PackageCard.svelte";
    import PackageComposer from "./PackageComposer.svelte";
    import ContentRenderer from "./ContentRenderer.svelte";

    const dispatch = createEventDispatcher();
    let errorMsg = "";
    /** @type {any} */
    let detailPackage = null;
    /** @type {any} */
    let detailFull = null;
    let detailMarkdownBody = "";
    let detailMarkdownLoading = false;
    let detailCacheExists = false;
    let detailFetchState = "idle";
    /** @type {any} */
    let detailFetchResult = null;
    let detailFetchError = "";
    let detailConsentGiven = false;
    /** @type {any[]} */
    let detailAttachments = [];
    let showPublishPanel = false;
    let publishCid = "";
    let publishProvider = "manual";
    let publishError = "";
    let publishLinking = false;
    let publishLoading = false;
    /** @type {any} */
    let publishResult = null;
    /** @type {any} */
    let attachmentPreviewFile = null;
    let viewMode = "grid";
    /** @type {string[]} */
    let backendFolders = [];
    /** @type {string | null} */
    let newPackageFolder = null;
    let deleteFolderConfirm = false;
    let deleteFolderDeleting = false;
    let creatingFolder = false;
    let createFolderName = "";
    let showCreateFolder = false;

    // Multi-select
    let selectMode = false;
    let selectedIds = new Set();
    let bulkDeleting = false;
    let bulkMoving = false;

    // Move popup
    let showMovePopup = false;
    let moveSearch = "";
    let moveTargetFolder = "";

    // Duplicate
    /** @type {string | null} */
    let duplicatingId = null;

    // Delete confirmation in list view
    /** @type {string | null} */
    let deleteConfirmId = null;
    /** @type {string | null} */
    let deleteDeletingId = null;

    // Bulk delete confirmation
    let bulkDeleteConfirm = false;

    // Sort mode cycling
    const SORT_MODES = [
        { key: "alpha-asc", label: "A → Z" },
        { key: "alpha-desc", label: "Z → A" },
        { key: "updated-newest", label: "NEWEST" },
        { key: "updated-oldest", label: "OLDEST" },
    ];

    function cycleSortMode() {
        const idx = SORT_MODES.findIndex((m) => m.key === $packageSortMode);
        const next = SORT_MODES[(idx + 1) % SORT_MODES.length];
        $packageSortMode = next.key;
    }

    $: sortedFolders = (() => {
        const all = [...backendFolders];
        if ($packageSortMode === "alpha-asc") return all.sort((a, b) => a.localeCompare(b));
        if ($packageSortMode === "alpha-desc") return all.sort((a, b) => b.localeCompare(a));
        if ($packageSortMode === "updated-newest") {
            return all.sort((a, b) => {
                const da = folderLastUpdated(a) || "";
                const db = folderLastUpdated(b) || "";
                return db.localeCompare(da);
            });
        }
        if ($packageSortMode === "updated-oldest") {
            return all.sort((a, b) => {
                const da = folderLastUpdated(a) || "";
                const db = folderLastUpdated(b) || "";
                return da.localeCompare(db);
            });
        }
        return all;
    })();

    $: $currentFolder, (async () => { backendFolders = await core.invoke("content_library_list_folders"); })();

    onMount(async () => {
        await refresh();
        try { backendFolders = await core.invoke("content_library_list_folders"); } catch {}
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
        newPackageFolder = $currentFolder;
        $activePanel = "create";
        detailPackage = null;
        detailFull = null;
    }

    function showBrowse() {
        $activePanel = "browse";
        detailPackage = null;
        detailFull = null;
        newPackageFolder = null;
        selectMode = false;
        selectedIds = new Set();
        bulkDeleteConfirm = false;
    }

    /**
     * @param {string | null} folder
     */
    function enterFolder(folder) {
        $currentFolder = folder;
        selectMode = false;
        selectedIds = new Set();
        bulkDeleteConfirm = false;
    }

    function goToRoot() {
        $currentFolder = null;
        selectMode = false;
        selectedIds = new Set();
        bulkDeleteConfirm = false;
    }

    async function createNewFolder() {
        const name = createFolderName.trim();
        if (!name) return;
        try {
            await core.invoke("content_library_create_folder", { name });
            backendFolders = await core.invoke("content_library_list_folders");
            showCreateFolder = false;
            createFolderName = "";
            await refresh();
        } catch (err) {
            errorMsg = String(err);
        }
    }

    async function deleteCurrentFolder() {
        if ($currentFolder === null || $currentFolder === "") return;
        deleteFolderDeleting = true;
        try {
            await core.invoke("content_library_delete_folder", { name: $currentFolder });
            goToRoot();
            await refresh();
            backendFolders = await core.invoke("content_library_list_folders");
        } catch (err) {
            errorMsg = String(err);
        }
        deleteFolderDeleting = false;
        deleteFolderConfirm = false;
    }

    /**
     * @param {string} folderName
     */
    function folderPackageCount(folderName) {
        return $contentLibrary.filter((p) => {
            if (!folderName) return !p.folder || !p.folder.trim();
            return p.folder && p.folder.trim() === folderName;
        }).length;
    }

    /**
     * @param {string} folderName
     */
    function folderLastUpdated(folderName) {
        const pkgs = $contentLibrary.filter((p) => {
            if (!folderName) return !p.folder || !p.folder.trim();
            return p.folder && p.folder.trim() === folderName;
        });
        if (pkgs.length === 0) return null;
        return pkgs.sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0].updated_at;
    }

    /**
     * @param {{ id: string }} pkg
     */
    function toggleSelect(pkg) {
        if (!selectMode) return;
        const next = new Set(selectedIds);
        if (next.has(pkg.id)) {
            next.delete(pkg.id);
        } else {
            next.add(pkg.id);
        }
        selectedIds = next;
    }

    /**
     * @param {{ id: string }} pkg
     */
    function isSelected(pkg) {
        return selectMode && selectedIds.has(pkg.id);
    }

    async function bulkDelete() {
        if (selectedIds.size === 0) return;
        bulkDeleting = true;
        for (const id of selectedIds) {
            try {
                await core.invoke("content_library_delete", { packageId: id });
            } catch (err) {
                console.warn("Delete failed:", id, err);
            }
        }
        selectedIds = new Set();
        selectMode = false;
        await refresh();
        bulkDeleting = false;
    }

    async function bulkMove() {
        if (selectedIds.size === 0 || !moveTargetFolder) return;
        bulkMoving = true;
        try {
            await core.invoke("content_library_move_packages", {
                packageIds: Array.from(selectedIds),
                folder: moveTargetFolder,
            });
            selectedIds = new Set();
            selectMode = false;
            showMovePopup = false;
            await refresh();
        } catch (err) {
            errorMsg = String(err);
        }
        bulkMoving = false;
    }

    /**
     * @param {{ id: string }} pkg
     */
    async function duplicatePackage(pkg) {
        duplicatingId = pkg.id;
        try {
            await core.invoke("content_library_duplicate", { packageId: pkg.id });
            await refresh();
        } catch (err) {
            errorMsg = String(err);
        }
        duplicatingId = null;
    }

    function filteredMoveFolders() {
        const q = moveSearch.trim().toLowerCase();
        const all = ["", ...backendFolders];
        if (!q) return all;
        return all.filter((f) => (f || "unsorted").toLowerCase().includes(q));
    }

    /**
     * @param {{ id: string, cid?: string }} pkg
     */
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
                detailAttachments = detailFull.files.filter(/** @param {{ path: string }} f */ (f) => f.path !== "content.md");
                const mdFile = detailFull.files.find(/** @param {{ path: string }} f */ (f) => f.path === "content.md");
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

    /**
     * @param {any} e
     */
    function onView(e) {
        const id = e.detail || (e.target && e.target.pkg && e.target.pkg.id);
    }

    function onPackageSaved() {
        showBrowse();
        refresh();
        newPackageFolder = null;
    }

    function onPackageImported() {
        refresh();
    }

    /** @param {CustomEvent<any> & { target: EventTarget | null }} event */
    function handleCardView(event) {
        const target = event.target instanceof Element ? event.target : null;
        const pkgEl = target?.closest?.("[data-pkg-id]");
        const pkgId = pkgEl instanceof HTMLElement ? pkgEl.dataset.pkgId : null;
        if (pkgId) {
            const pkg = $contentLibrary.find((p) => p.id === pkgId);
            if (pkg) showDetail(pkg);
        }
    }

    /**
     * @param {string} cid
     */
    function showCidViewer(cid) {
        $ipfsHubSection = "cid-viewer";
    }

    /** @param {Uint8Array} bytes */
    function bytesToBase64(bytes) {
        let binary = "";
        const chunkSize = 0x8000;
        for (let i = 0; i < bytes.length; i += chunkSize) {
            const chunk = bytes.subarray(i, i + chunkSize);
            binary += String.fromCharCode.apply(null, Array.from(chunk));
        }
        return btoa(binary);
    }

    /** @param {string} text */
    function textToBase64(text) {
        return bytesToBase64(new TextEncoder().encode(text || ""));
    }

    /** @param {number} bytes */
    function formatSize(bytes) {
        if (!bytes) return "0 B";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    }

    /** @param {string} mime */
    function isImageMime(mime) {
        return mime && mime.startsWith("image/");
    }

    /** @param {string} mime */
    function isPreviewable(mime) {
        return mime && (
            mime.startsWith("image/") ||
            mime.startsWith("text/") ||
            mime === "application/json"
        );
    }

    /** @param {string} path */
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

    function togglePublishPanel() {
        showPublishPanel = !showPublishPanel;
        publishCid = detailFull && detailFull.cid ? detailFull.cid : "";
        publishProvider = (detailFull && detailFull.provider) ? detailFull.provider : "manual";
        publishError = "";
        publishLoading = false;
        publishResult = null;
    }

    async function doLinkCid() {
        publishError = "";
        publishLinking = true;
        try {
            const result = await core.invoke("content_library_link_cid", {
                input: {
                    package_id: detailFull.id,
                    cid: publishCid.trim(),
                    provider: publishProvider,
                },
            });
            detailFull = result;
            detailPackage = {
                ...detailPackage,
                cid: result.cid,
                status: result.status,
                version: result.version,
                updated_at: result.updated_at,
            };
            showPublishPanel = false;
        } catch (err) {
            publishError = String(err);
        }
        publishLinking = false;
    }

    async function doPublish() {
        publishError = "";
        publishLoading = true;
        publishResult = null;
        try {
            const result = await core.invoke("content_library_publish_package", {
                packageId: detailFull.id,
                provider: publishProvider,
            });
            publishResult = result;
            publishCid = result.cid;
            detailFull = {
                ...detailFull,
                cid: result.cid,
                status: result.status,
                version: result.version,
                provider: result.provider,
                published_at: result.published_at,
            };
            detailPackage = {
                ...detailPackage,
                cid: result.cid,
                status: result.status,
                version: result.version,
                updated_at: result.published_at,
            };
        } catch (err) {
            publishError = String(err);
        }
        publishLoading = false;
    }

    async function copyPublishCid() {
        const cid = (publishResult && publishResult.cid) || publishCid || (detailFull && detailFull.cid) || "";
        if (!cid) return;
        try {
            await navigator.clipboard.writeText(cid);
        } catch {
            const ta = document.createElement("textarea");
            ta.value = cid;
            document.body.appendChild(ta);
            ta.select();
            document.execCommand("copy");
            document.body.removeChild(ta);
        }
    }

    /** @param {string} text */
    function copyToClipboard(text) {
        navigator.clipboard.writeText(text).catch(() => {});
    }

    function clearAttachmentPreview() {
        attachmentPreviewFile = null;
        detailFetchResult = null;
        detailFetchState = "idle";
        detailFetchError = "";
    }

    async function openPackageFolder() {
        if (!detailPackage) return;
        try {
            await core.invoke("content_library_open_package_folder", { packageId: detailPackage.id });
        } catch (err) {
            errorMsg = String(err);
        }
    }
</script>

<div class="content-library">
    <header class="library-header">
        <div class="header-left">
            <span class="header-title">CONTENT LIBRARY</span>
            {#if $activePanel === "browse" && $currentFolder !== null}
                <span class="header-count mono">{$filteredPackages.length} packages</span>
            {/if}
        </div>
        <div class="header-actions">
            {#if $activePanel === "browse"}
                {#if $currentFolder === null}
                    <button class="header-btn" on:click={() => { showCreateFolder = true; }}>
                        + NEW FOLDER
                    </button>
                {/if}
                <button class="header-btn create-btn" on:click={showCreate}>
                    + NEW PACKAGE
                </button>
                {#if $currentFolder !== null && $currentFolder !== ""}
                    {#if !deleteFolderConfirm}
                        <button class="header-btn danger" on:click={() => (deleteFolderConfirm = true)}>
                            DELETE FOLDER
                        </button>
                    {:else}
                        <div class="confirm-bar inline" in:fly={{ y: 3, duration: 120 }}>
                            <span class="confirm-text">This will delete the folder including all packages in the folder. Are you sure?</span>
                            <button class="header-btn danger tiny" on:click={deleteCurrentFolder} disabled={deleteFolderDeleting}>
                                {deleteFolderDeleting ? "..." : "YES"}
                            </button>
                            <button class="header-btn tiny" on:click={() => (deleteFolderConfirm = false)}>NO</button>
                        </div>
                    {/if}
                {/if}
            {/if}
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
                <button class="filter-btn" class:active={$statusFilter === "published"} on:click={() => ($statusFilter = $statusFilter === "published" ? "all" : "published")}>
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
            {:else}
                {#if $currentFolder === null}
                    <!-- ROOT EXPLORER VIEW -->
                    {#if showCreateFolder}
                        <div class="create-folder-bar" in:fly={{ y: 5, duration: 150 }}>
                            <input
                                class="form-input mono"
                                type="text"
                                placeholder="Folder name"
                                bind:value={createFolderName}
                                on:keydown={(e) => e.key === 'Enter' && createNewFolder()}
                            />
                            <button class="cyber-btn small" on:click={createNewFolder}>CREATE</button>
                            <button class="cyber-btn small ghost" on:click={() => { showCreateFolder = false; createFolderName = ""; }}>CANCEL</button>
                        </div>
                    {/if}
                    <div class="explorer-toolbar">
                        <div class="toolbar-left"></div>
                        <div class="toolbar-right">
                            <button class="view-btn sort-btn-inline" on:click={cycleSortMode} title="Sort">
                                SORT: {SORT_MODES.find(m => m.key === $packageSortMode)?.label || "NEWEST"}
                            </button>
                            <button class="view-btn" class:active={viewMode === "grid"} on:click={() => (viewMode = "grid")} title="Grid view">
                                GRID
                            </button>
                            <button class="view-btn" class:active={viewMode === "list"} on:click={() => (viewMode = "list")} title="List view">
                                LIST
                            </button>
                        </div>
                    </div>
                    {#if $contentLibrary.length === 0 && backendFolders.length === 0}
                        <div class="empty-state">
                            <div class="empty-icon">◈</div>
                            <div class="empty-text">No packages in library</div>
                            <button class="cyber-btn" on:click={showCreate}>CREATE YOUR FIRST PACKAGE</button>
                        </div>
                    {:else if viewMode === "grid"}
                        <div class="folder-grid">
                            <!-- Unsorted card -->
                            {#if folderPackageCount("") > 0 || $searchQuery || $statusFilter !== "all"}
                                <div class="folder-card" role="button" tabindex="0" on:click={() => enterFolder("")} on:keydown={(e) => e.key === 'Enter' && enterFolder("")} in:fly={{ y: 8, duration: 150 }}>
                                    <span class="folder-card-icon">◈</span>
                                    <span class="folder-card-name">Unsorted</span>
                                    <span class="folder-card-count mono">{folderPackageCount("")} packages</span>
                                    <span class="folder-card-date">{folderLastUpdated("") ? folderLastUpdated("").slice(0, 10) : ""}</span>
                                </div>
                            {/if}
                            {#each sortedFolders as folderName}
                                <div class="folder-card" role="button" tabindex="0" on:click={() => enterFolder(folderName)} on:keydown={(e) => e.key === 'Enter' && enterFolder(folderName)} in:fly={{ y: 8, duration: 150 }}>
                                    <span class="folder-card-icon">📁</span>
                                    <span class="folder-card-name">{folderName}</span>
                                    <span class="folder-card-count mono">{folderPackageCount(folderName)} packages</span>
                                    <span class="folder-card-date">{folderLastUpdated(folderName) ? folderLastUpdated(folderName).slice(0, 10) : ""}</span>
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="folder-list">
                            <!-- Unsorted row -->
                            {#if folderPackageCount("") > 0 || $searchQuery || $statusFilter !== "all"}
                                <div class="folder-row" role="button" tabindex="0" on:click={() => enterFolder("")} on:keydown={(e) => e.key === 'Enter' && enterFolder("")} in:fly={{ y: 8, duration: 150 }}>
                                    <span class="row-icon">◈</span>
                                    <span class="row-name">Unsorted</span>
                                    <span class="row-count mono">{folderPackageCount("")} packages</span>
                                    <span class="row-date">{folderLastUpdated("") ? folderLastUpdated("").slice(0, 10) : ""}</span>
                                    <span class="row-arrow">→</span>
                                </div>
                            {/if}
                            {#each sortedFolders as folderName}
                                <div class="folder-row" role="button" tabindex="0" on:click={() => enterFolder(folderName)} on:keydown={(e) => e.key === 'Enter' && enterFolder(folderName)} in:fly={{ y: 8, duration: 150 }}>
                                    <span class="row-icon">📁</span>
                                    <span class="row-name">{folderName}</span>
                                    <span class="row-count mono">{folderPackageCount(folderName)} packages</span>
                                    <span class="row-date">{folderLastUpdated(folderName) ? folderLastUpdated(folderName).slice(0, 10) : ""}</span>
                                    <span class="row-arrow">→</span>
                                </div>
                            {/each}
                        </div>
                    {/if}
                {:else}
                    <!-- INSIDE FOLDER VIEW -->
                    <div class="folder-breadcrumb">
                        <div class="breadcrumb-left">
                            <button class="breadcrumb-btn" on:click={goToRoot}>
                                <span class="breadcrumb-arrow">←</span> CONTENT LIBRARY
                            </button>
                            <span class="breadcrumb-sep">/</span>
                            <span class="breadcrumb-current">{$currentFolder === "" ? "Unsorted" : $currentFolder}</span>
                        </div>
                        <div class="breadcrumb-right">
                            {#if !selectMode}
                                <button class="view-btn select-btn" on:click={() => { selectMode = true; selectedIds = new Set(); }}>
                                    SELECT
                                </button>
                            {:else}
                                <button class="view-btn select-btn" on:click={() => { selectMode = false; selectedIds = new Set(); }}>
                                    CANCEL
                                </button>
                            {/if}
                            <button class="view-btn sort-btn-inline" on:click={cycleSortMode} title="Sort">
                                SORT: {SORT_MODES.find(m => m.key === $packageSortMode)?.label || "NEWEST"}
                            </button>
                            <button class="view-btn" class:active={viewMode === "grid"} on:click={() => (viewMode = "grid")} title="Grid view">
                                GRID
                            </button>
                            <button class="view-btn" class:active={viewMode === "list"} on:click={() => (viewMode = "list")} title="List view">
                                LIST
                            </button>
                        </div>
                    </div>
                    {#if selectMode && selectedIds.size > 0}
                        <div class="bulk-bar" in:fly={{ y: 5, duration: 150 }}>
                            <span class="bulk-count">{selectedIds.size} selected</span>
                            <div class="bulk-actions">
                                <button class="cyber-btn small" on:click={() => { showMovePopup = true; moveSearch = ""; moveTargetFolder = ""; }}>MOVE</button>
                                {#if !bulkDeleteConfirm}
                                    <button class="cyber-btn small danger" on:click={() => { bulkDeleteConfirm = true; }}>DELETE</button>
                                {:else}
                                    <button class="cyber-btn small danger confirm" on:click={bulkDelete} disabled={bulkDeleting}>{bulkDeleting ? "..." : "YES"}</button>
                                    <button class="cyber-btn small ghost" on:click={() => { bulkDeleteConfirm = false; }}>NO</button>
                                {/if}
                                <button class="cyber-btn small ghost" on:click={() => { selectMode = false; selectedIds = new Set(); bulkDeleteConfirm = false; }}>CANCEL</button>
                            </div>
                        </div>
                    {/if}
                    {#if $filteredPackages.length === 0}
                        <div class="empty-state">
                            <div class="empty-icon">◈</div>
                            <div class="empty-text">No packages in this folder</div>
                            <button class="cyber-btn" on:click={showCreate}>CREATE PACKAGE</button>
                        </div>
                    {:else if viewMode === "grid"}
                        <div class="package-grid">
                            {#each $filteredPackages as pkg (pkg.id)}
                                <div class="grid-item" class:selected={isSelected(pkg)} in:fly={{ y: 8, duration: 150 }} on:click={() => { if (selectMode) { toggleSelect(pkg); } else { showDetail(pkg); } }} role="button" tabindex="0" on:keydown={(e) => { if (e.key === 'Enter') { if (selectMode) toggleSelect(pkg); else showDetail(pkg); } }}>
                                    {#if selectMode}
                                        <div class="select-overlay">
                                            <span class="select-check">{isSelected(pkg) ? "◉" : "○"}</span>
                                        </div>
                                    {/if}
                                    <PackageCard {pkg} on:refresh={refresh} on:edit={() => { $activePanel = pkg.id; }} on:view={() => showDetail(pkg)} on:duplicate={() => duplicatePackage(pkg)} />
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="package-list">
                            {#each $filteredPackages as pkg (pkg.id)}
                                <div class="list-row" class:selected={isSelected(pkg)} role="button" tabindex="0" in:fly={{ y: 5, duration: 150 }} on:click={() => { if (selectMode) { toggleSelect(pkg); } else { showDetail(pkg); } }} on:keydown={(e) => { if (e.key === 'Enter') { if (selectMode) toggleSelect(pkg); else showDetail(pkg); } }}>
                                    {#if selectMode}
                                        <span class="list-check">{isSelected(pkg) ? "◉" : "○"}</span>
                                    {:else}
                                        <span class="list-icon">◈</span>
                                    {/if}
                                    <span class="list-name">{pkg.name}</span>
                                    <span class="list-meta mono">v{pkg.version} · {pkg.file_count} file{pkg.file_count !== 1 ? "s" : ""} · {(pkg.updated_at || "").slice(0, 10)}</span>
                                    {#if pkg.cid}
                                        <span class="list-cid mono" title={pkg.cid}>
                                            {pkg.cid.length > 20 ? pkg.cid.slice(0, 8) + "..." + pkg.cid.slice(-8) : pkg.cid}
                                        </span>
                                        <button class="action-btn tiny" on:click|stopPropagation={async () => { try { await navigator.clipboard.writeText(pkg.cid); } catch {} }} title="Copy CID">COPY CID</button>
                                    {/if}
                                    {#if !selectMode}
                                        <div class="list-actions">
                                            <button class="action-btn tiny" on:click|stopPropagation={() => { $activePanel = pkg.id; }}>EDIT</button>
                                            <button class="action-btn tiny" on:click|stopPropagation={() => showDetail(pkg)}>VIEW</button>
                                            <button class="action-btn tiny" on:click|stopPropagation={() => duplicatePackage(pkg)} disabled={duplicatingId === pkg.id}>
                                                {duplicatingId === pkg.id ? "..." : "DUP"}
                                            </button>
                                            {#if deleteConfirmId === pkg.id}
                                                <button class="action-btn tiny danger confirm" on:click|stopPropagation={async () => { deleteDeletingId = pkg.id; try { await core.invoke("content_library_delete", { packageId: pkg.id }); deleteConfirmId = null; await refresh(); } catch (err) { errorMsg = String(err); } deleteDeletingId = null; }} disabled={deleteDeletingId === pkg.id}>
                                                    {deleteDeletingId === pkg.id ? "..." : "YES"}
                                                </button>
                                                <button class="action-btn tiny" on:click|stopPropagation={() => { deleteConfirmId = null; }}>NO</button>
                                            {:else}
                                                <button class="action-btn tiny danger" on:click|stopPropagation={() => { deleteConfirmId = pkg.id; }}>DEL</button>
                                            {/if}
                                        </div>
                                    {/if}
                                </div>
                            {/each}
                        </div>
                    {/if}
                {/if}
            {/if}
        {:else if $activePanel === "create"}
            <PackageComposer createFolder={newPackageFolder} on:saved={onPackageSaved} on:cancel={showBrowse} />
        {:else if $activePanel === "detail" && detailPackage}
            <div class="detail-panel" in:fly={{ y: 10, duration: 150 }}>
                <div class="detail-header">
                    <button class="header-btn back-btn" on:click={showBrowse}>← BACK TO LIBRARY</button>
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
                    <div class="detail-meta-item wide folder-meta">
                        <span class="detail-meta-label">Folder</span>
                        <span class="detail-meta-value">{detailFull && detailFull.folder ? detailFull.folder : "Unsorted"}</span>
                        <button class="inline-btn" on:click={openPackageFolder}>OPEN SYSTEM FOLDER</button>
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
                            {#if attachmentPreviewFile}
                                Previewing attachment: <span class="mono">{attachmentPreviewFile.path}</span>
                                <button class="clear-preview-btn" on:click={clearAttachmentPreview}>CLEAR</button>
                            {:else}
                                FETCHED CONTENT
                            {/if}
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
                                                attachmentPreviewFile = file;
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

                <div class="detail-section">
                    <div class="section-label">ACTIONS</div>
                    <div class="detail-actions-minimal">
                        <button class="cyber-btn" on:click={() => { $activePanel = detailPackage.id; }}>
                            EDIT PACKAGE
                        </button>
                        <button class="cyber-btn" on:click={togglePublishPanel}>
                            PUBLISH / LINK
                        </button>
                        <button class="cyber-btn ghost" on:click={openPackageFolder}>
                            OPEN SYSTEM FOLDER
                        </button>
                        {#if detailPackage.cid}
                            <button class="cyber-btn ghost" on:click={() => { $ipfsHubSection = "cid-viewer"; }}>
                                VIEW IN CID VIEWER
                            </button>
                        {/if}
                    </div>
                </div>

                {#if showPublishPanel}
                    <div class="publish-panel" in:fly={{ y: 10, duration: 150 }}>
                        <div class="section-label">PUBLISH / LINK PACKAGE</div>
                        <div class="publish-desc">
                            Publish this package to IPFS or link a CID you already created. Only publish content you have the right to share.
                        </div>
                        {#if detailFull && detailFull.cid}
                            <div class="publish-info">
                                Current CID: <span class="mono">{detailFull.cid}</span>
                                {#if detailFull.provider}
                                    <span class="link-provider">({detailFull.provider})</span>
                                {/if}
                            </div>
                        {/if}
                        <div class="publish-form">
                            <div class="form-group">
                                <label class="form-label" for="publish-provider">PROVIDER</label>
                                <select id="publish-provider" class="form-input mono" bind:value={publishProvider}>
                                    <option value="manual">Manual CID Link</option>
                                    <option value="pinata">Pinata</option>
                                    <option value="filebase">Filebase</option>
                                    <option value="installed_kubo">Installed Kubo</option>
                                </select>
                            </div>
                            {#if publishProvider === "manual"}
                                <div class="form-group">
                                    <label class="form-label" for="publish-cid">CID</label>
                                    <input id="publish-cid" class="form-input mono" type="text" bind:value={publishCid} placeholder="Qm... or bafy..." />
                                </div>
                            {:else}
                                <div class="publish-note">
                                    Publishing uploads the full package folder through {publishProvider === "installed_kubo" ? "your local Kubo node" : publishProvider}. Configure provider credentials in IPFS Settings first.
                                </div>
                            {/if}
                            {#if publishError}
                                <div class="error-bar" style="margin-bottom:0.5rem;">{publishError}</div>
                            {/if}
                            {#if publishResult}
                                <div class="publish-success">
                                    <span>Published CID</span>
                                    <code>{publishResult.cid}</code>
                                </div>
                            {/if}
                            <div class="publish-actions">
                                {#if publishProvider === "manual"}
                                    <button class="cyber-btn small" on:click={doLinkCid} disabled={publishLinking || !publishCid.trim()}>
                                        {publishLinking ? "LINKING..." : "LINK CID"}
                                    </button>
                                {:else}
                                    <button class="cyber-btn small" on:click={doPublish} disabled={publishLoading}>
                                        {publishLoading ? "PUBLISHING..." : "PUBLISH PACKAGE"}
                                    </button>
                                {/if}
                                {#if (publishResult && publishResult.cid) || publishCid || (detailFull && detailFull.cid)}
                                    <button class="cyber-btn small ghost" on:click={copyPublishCid}>
                                        COPY CID
                                    </button>
                                    <button class="cyber-btn small ghost" on:click={() => { $ipfsHubSection = "cid-viewer"; }}>
                                        CID VIEWER
                                    </button>
                                {/if}
                                <button class="cyber-btn small ghost" on:click={() => { showPublishPanel = false; publishError = ""; }}>
                                    CLOSE
                                </button>
                            </div>
                        </div>
                        <div class="publish-note">
                            Published IPFS content may be public and difficult to remove.
                        </div>
                    </div>
                {/if}
            </div>
        {:else}
            {#each $contentLibrary as pkg (pkg.id)}
                {#if $activePanel === pkg.id}
                    <PackageComposer editPackage={pkg} on:saved={onPackageSaved} on:cancel={showBrowse} />
                {/if}
            {/each}
        {/if}

        {#if showMovePopup}
            <div class="move-popup-backdrop" role="button" tabindex="0" aria-label="Close move popup" on:click={() => { showMovePopup = false; }} on:keydown={(e) => e.key === 'Escape' && (showMovePopup = false)}>
                <div class="move-popup" role="dialog" aria-modal="true" tabindex="-1" on:click|stopPropagation on:keydown={(e) => e.key === 'Escape' && (showMovePopup = false)}>
                    <div class="move-popup-header">MOVE SELECTED PACKAGES</div>
                    <div class="move-popup-search">
                        <input
                            class="form-input mono"
                            type="text"
                            placeholder="Search folders..."
                            bind:value={moveSearch}
                        />
                    </div>
                    <div class="move-popup-list">
                        {#each filteredMoveFolders() as folderName}
                            <div class="move-popup-row" class:active={moveTargetFolder === folderName} on:click={() => { moveTargetFolder = folderName; }} role="button" tabindex="0" on:keydown={(e) => e.key === 'Enter' && (moveTargetFolder = folderName)}>
                                <span class="move-popup-icon">▸</span>
                                <span class="move-popup-label">{folderName === "" ? "Unsorted" : folderName}</span>
                            </div>
                        {/each}
                    </div>
                    <div class="move-popup-actions">
                        <button class="cyber-btn small" on:click={bulkMove} disabled={bulkMoving || !moveTargetFolder}>{bulkMoving ? "MOVING..." : "MOVE"}</button>
                        <button class="cyber-btn small ghost" on:click={() => { showMovePopup = false; }}>CANCEL</button>
                    </div>
                </div>
            </div>
        {/if}
    </div>
</div>

<style>
    .content-library {
        display: flex;
        flex-direction: column;
        min-height: 0;
    }
    .library-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0 0.75rem 0;
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
        padding: 1rem 0;
    }
    .explorer-toolbar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.75rem;
        flex-wrap: wrap;
        gap: 0.5rem;
    }
    .toolbar-left {
        display: flex;
        gap: 0.4rem;
        flex-wrap: wrap;
    }
    .toolbar-right {
        display: flex;
        gap: 0.2rem;
    }
    .view-btn {
        background: rgba(255, 255, 255, 0.02);
        border: 1px solid rgba(255, 255, 255, 0.06);
        color: #555;
        padding: 0.25rem 0.55rem;
        font-size: 0.55rem;
        letter-spacing: 1px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .view-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .view-btn.active {
        background: rgba(0, 255, 65, 0.06);
        border-color: rgba(0, 255, 65, 0.4);
        color: var(--color-primary);
    }
    .select-btn {
        padding: 0.25rem 0.5rem;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
    }
    .sort-btn-inline {
        padding: 0.25rem 0.5rem;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
    }
    .breadcrumb-right {
        display: flex;
        gap: 0.2rem;
        align-items: center;
    }
    .confirm-bar.inline {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        padding: 0.15rem 0.4rem;
        background: rgba(255, 68, 68, 0.06);
        border: 1px solid rgba(255, 68, 68, 0.2);
        border-radius: 4px;
    }
    .header-btn.tiny {
        padding: 0.2rem 0.5rem;
        font-size: 0.55rem;
    }
    .create-folder-bar {
        display: flex;
        gap: 0.4rem;
        align-items: center;
        margin-bottom: 0.6rem;
        flex-wrap: wrap;
    }
    .create-folder-bar .form-input {
        flex: 1;
        min-width: 160px;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.35rem 0.6rem;
        border-radius: 4px;
        font-size: 0.7rem;
        outline: none;
        box-sizing: border-box;
    }
    .create-folder-bar .form-input:focus {
        border-color: var(--color-primary);
    }
    .confirm-bar {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .confirm-text {
        font-size: 0.6rem;
        color: #c66;
        margin-right: 0.3rem;
    }
    .header-btn.danger {
        background: rgba(255, 68, 68, 0.08);
        border-color: rgba(255, 68, 68, 0.3);
        color: #c66;
    }
    .header-btn.danger:hover {
        background: rgba(255, 68, 68, 0.15);
        border-color: #ff5555;
        color: #ff5555;
        box-shadow: 0 0 10px rgba(255, 68, 68, 0.15);
    }
    .folder-list {
        display: flex;
        flex-direction: column;
        gap: 0.35rem;
    }
    .folder-row {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 0.55rem 0.8rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .folder-row:hover {
        border-color: rgba(0, 255, 65, 0.25);
        background: rgba(0, 255, 65, 0.04);
    }
    .row-icon {
        font-size: 0.7rem;
        color: var(--color-primary);
        opacity: 0.8;
        width: 20px;
        text-align: center;
    }
    .row-name {
        font-size: 0.78rem;
        font-weight: 600;
        color: #ccc;
        letter-spacing: 0.3px;
        flex: 1;
        min-width: 0;
    }
    .row-count {
        font-size: 0.6rem;
        color: #555;
        flex-shrink: 0;
    }
    .row-date {
        font-size: 0.55rem;
        color: #444;
        flex-shrink: 0;
    }
    .row-arrow {
        font-size: 0.7rem;
        color: #444;
        flex-shrink: 0;
    }
    .folder-row:hover .row-arrow {
        color: var(--color-primary);
    }
    .folder-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
        gap: 0.75rem;
    }
    .folder-card {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.3rem;
        padding: 1rem 0.8rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        text-align: center;
    }
    .folder-card:hover {
        border-color: rgba(0, 255, 65, 0.25);
        background: rgba(0, 255, 65, 0.04);
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
    }
    .folder-card-icon {
        font-size: 1.4rem;
        color: var(--color-primary);
        opacity: 0.8;
        margin-bottom: 0.2rem;
    }
    .folder-card-name {
        font-size: 0.85rem;
        font-weight: 600;
        color: #ccc;
        letter-spacing: 0.3px;
        word-break: break-word;
    }
    .folder-card-count {
        font-size: 0.6rem;
        color: #555;
    }
    .folder-card-date {
        font-size: 0.55rem;
        color: #444;
    }
    .folder-breadcrumb {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        margin-bottom: 0.6rem;
        padding-bottom: 0.4rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        justify-content: space-between;
        flex-wrap: wrap;
    }
    .breadcrumb-left {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .breadcrumb-btn {
        background: transparent;
        border: none;
        color: #888;
        font-size: 0.65rem;
        letter-spacing: 1px;
        cursor: pointer;
        padding: 0;
        transition: color 0.15s;
        display: flex;
        align-items: center;
        gap: 0.3rem;
    }
    .breadcrumb-btn:hover {
        color: var(--color-primary);
    }
    .breadcrumb-arrow {
        font-size: 0.75rem;
        color: inherit;
    }
    .breadcrumb-sep {
        color: #444;
        font-size: 0.65rem;
    }
    .breadcrumb-current {
        color: #ccc;
        font-size: 0.7rem;
        font-weight: 600;
        letter-spacing: 0.5px;
    }
    .package-list {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .list-row {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 0.5rem 0.7rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .list-row:hover {
        border-color: rgba(0, 255, 65, 0.25);
        background: rgba(0, 255, 65, 0.04);
    }
    .list-name {
        font-size: 0.8rem;
        color: #ccc;
        font-weight: 600;
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .list-meta {
        font-size: 0.6rem;
        color: #555;
        flex-shrink: 0;
    }
    .list-cid {
        font-size: 0.6rem;
        color: #777;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        padding: 0.15rem 0.4rem;
        border-radius: 4px;
        flex-shrink: 0;
        max-width: 140px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .package-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
        grid-auto-rows: 1fr;
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
        border-top: 2px solid rgba(0, 255, 65, 0.3);
        border-radius: 8px;
        padding: 1.2rem;
        max-width: 100%;
    }
    .detail-header {
        display: flex;
        align-items: center;
        margin-bottom: 1rem;
        padding-bottom: 0.6rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .back-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
        font-size: 0.6rem;
        letter-spacing: 1px;
        padding: 0.3rem 0.7rem;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .back-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .detail-actions-minimal {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .folder-meta .inline-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
        font-size: 0.5rem;
        padding: 2px 8px;
        border-radius: 3px;
        cursor: pointer;
        margin-left: auto;
        transition: all 0.15s;
    }
    .folder-meta .inline-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
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
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
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
        border-radius: 6px;
        font-size: 0.65rem;
    }
    .detail-meta-item.wide {
        grid-column: 1 / -1;
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
    .detail-section {
        margin-top: 1rem;
        padding: 0.85rem 1rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-left: 2px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
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
        padding: 0.75rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 6px;
    }
    .detail-fetched-section {
        margin-top: 1rem;
        padding: 0.75rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 6px;
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
        padding: 0.75rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 6px;
    }
    .attachment-list {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .attachment-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.35rem 0.6rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        font-size: 0.65rem;
        transition: border-color 0.15s;
    }
    .attachment-row:hover {
        border-color: rgba(0, 255, 65, 0.15);
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
    .publish-panel {
        margin-top: 1rem;
        padding: 0.8rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.15);
        border-radius: 6px;
    }
    .publish-info {
        font-size: 0.65rem;
        color: var(--color-primary);
    }
    .publish-desc {
        font-size: 0.65rem;
        color: #aaa;
        margin-bottom: 0.75rem;
        line-height: 1.4;
    }
    .publish-form {
        margin-bottom: 0.5rem;
    }
    .publish-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.5rem;
    }
    .publish-note {
        margin-top: 0.6rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        font-size: 0.55rem;
        color: #555;
        line-height: 1.4;
    }
    .form-group {
        margin-bottom: 0.5rem;
    }
    .form-label {
        display: block;
        font-size: 0.6rem;
        letter-spacing: 1px;
        color: #555;
        margin-bottom: 0.25rem;
    }
    .form-input {
        width: 100%;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.4rem 0.6rem;
        border-radius: 4px;
        font-size: 0.7rem;
        outline: none;
        box-sizing: border-box;
    }
    .form-input:focus {
        border-color: var(--color-primary);
    }
    select.form-input {
        cursor: pointer;
        background: #020604;
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.28);
    }
    select.form-input option {
        background: #101010;
        color: #d8d8d8;
    }
    .clear-preview-btn {
        background: transparent;
        border: 1px solid rgba(255, 68, 68, 0.2);
        color: #c66;
        font-size: 0.5rem;
        padding: 1px 5px;
        border-radius: 3px;
        cursor: pointer;
    }
    .clear-preview-btn:hover {
        border-color: #ff5555;
        color: #ff5555;
    }
    .grid-item {
        position: relative;
        cursor: pointer;
        display: flex;
    }
    .grid-item :global(.package-card) {
        height: 100%;
        width: 100%;
    }
    .grid-item.selected {
        opacity: 0.85;
    }
    .grid-item.selected :global(.package-card) {
        border-color: rgba(0, 255, 65, 0.5);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.15);
    }
    .select-overlay {
        position: absolute;
        top: 0.5rem;
        right: 0.5rem;
        z-index: 2;
        pointer-events: none;
    }
    .select-check {
        font-size: 0.85rem;
        color: var(--color-primary);
        text-shadow: 0 0 4px rgba(0, 0, 0, 0.8);
    }
    .list-check {
        font-size: 0.75rem;
        color: var(--color-primary);
        width: 20px;
        text-align: center;
        flex-shrink: 0;
    }
    .list-icon {
        font-size: 0.6rem;
        color: #555;
        width: 20px;
        text-align: center;
        flex-shrink: 0;
    }
    .list-row.selected {
        border-color: rgba(0, 255, 65, 0.4);
        background: rgba(0, 255, 65, 0.06);
    }
    .list-actions {
        display: flex;
        gap: 0.25rem;
        flex-shrink: 0;
    }
    .action-btn.tiny {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
        padding: 2px 6px;
        font-size: 0.5rem;
        letter-spacing: 0.5px;
        border-radius: 3px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .action-btn.tiny:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .action-btn.tiny.danger {
        border-color: rgba(255, 68, 68, 0.15);
        color: #c66;
    }
    .action-btn.tiny.danger:hover {
        border-color: rgba(255, 68, 68, 0.4);
        color: #ff5555;
    }
    .action-btn.tiny.confirm {
        border-color: rgba(255, 68, 68, 0.4);
        color: #ff5555;
        background: rgba(255, 68, 68, 0.08);
    }
    .bulk-bar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.4rem 0.6rem;
        margin-bottom: 0.6rem;
        background: rgba(0, 255, 65, 0.04);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 6px;
    }
    .bulk-count {
        font-size: 0.65rem;
        color: var(--color-primary);
        font-weight: 600;
        letter-spacing: 0.5px;
    }
    .bulk-actions {
        display: flex;
        gap: 0.4rem;
    }
    .move-popup-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.6);
        z-index: 100;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .move-popup {
        background: rgba(10, 10, 10, 0.95);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        padding: 1rem;
        width: 320px;
        max-width: 90vw;
    }
    .move-popup-header {
        font-size: 0.7rem;
        color: var(--color-primary);
        letter-spacing: 1.5px;
        margin-bottom: 0.6rem;
        font-weight: 600;
    }
    .move-popup-search {
        margin-bottom: 0.5rem;
    }
    .move-popup-search .form-input {
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
    .move-popup-search .form-input:focus {
        border-color: var(--color-primary);
    }
    .move-popup-list {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
        max-height: 220px;
        overflow-y: auto;
        margin-bottom: 0.6rem;
    }
    .move-popup-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.4rem 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .move-popup-row:hover {
        border-color: rgba(0, 255, 65, 0.25);
        background: rgba(0, 255, 65, 0.04);
    }
    .move-popup-row.active {
        border-color: rgba(0, 255, 65, 0.4);
        background: rgba(0, 255, 65, 0.08);
    }
    .move-popup-icon {
        font-size: 0.6rem;
        color: var(--color-primary);
        opacity: 0.7;
    }
    .move-popup-label {
        font-size: 0.7rem;
        color: #ccc;
    }
    .move-popup-actions {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
    }
    .cyber-btn.danger {
        background: rgba(255, 68, 68, 0.08);
        border-color: rgba(255, 68, 68, 0.3);
        color: #c66;
    }
    .cyber-btn.danger:hover {
        background: rgba(255, 68, 68, 0.15);
        border-color: #ff5555;
        color: #ff5555;
        box-shadow: 0 0 10px rgba(255, 68, 68, 0.15);
    }
    .publish-success {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
        padding: 0.55rem 0.7rem;
        margin-bottom: 0.5rem;
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.65rem;
    }
    .publish-success code {
        color: #d8d8d8;
        word-break: break-all;
        font-size: 0.62rem;
    }
</style>
