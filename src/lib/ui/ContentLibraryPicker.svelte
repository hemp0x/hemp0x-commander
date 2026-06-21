<script>
    import { createEventDispatcher } from "svelte";
    import { fly } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { contentLibrary, libraryLoading } from "../stores/contentLibrary.js";

    export let value = "";
    export let label = "IPFS Hash";
    export let id = "ipfs-picker";
    export let placeholder = "Qm...";
    export let disabled = false;

    let showDropdown = false;
    let pickerSearch = "";
    let pickerError = "";
    let pickerEl;
    const dispatch = createEventDispatcher();
    let collapsedFolders = {};

    $: pickerPackages = filterPackages($contentLibrary, pickerSearch);
    $: pickerGroups = groupByFolder(pickerPackages);

    function filterPackages(packages, query) {
        let list = [...packages];
        if (query.trim()) {
            const q = query.toLowerCase();
            list = list.filter((p) =>
                p.name.toLowerCase().includes(q) ||
                (p.description && p.description.toLowerCase().includes(q)) ||
                (p.tags && p.tags.some((t) => t.toLowerCase().includes(q))) ||
                (p.cid && p.cid.toLowerCase().includes(q)) ||
                (p.folder && p.folder.toLowerCase().includes(q))
            );
        }
        // Sort: published/linked first, then by updated desc
        return list.sort((a, b) => {
            const aHasCid = !!a.cid;
            const bHasCid = !!b.cid;
            if (aHasCid && !bHasCid) return -1;
            if (!aHasCid && bHasCid) return 1;
            return (b.updated_at || "").localeCompare(a.updated_at || "");
        });
    }

    function groupByFolder(packages) {
        const groups = new Map();
        for (const pkg of packages) {
            const folder = pkg.folder && pkg.folder.trim() ? pkg.folder.trim() : "Unsorted";
            if (!groups.has(folder)) groups.set(folder, []);
            groups.get(folder).push(pkg);
        }
        return Array.from(groups.entries()).map(([folder, rows]) => ({ folder, rows }));
    }

    function shortCid(cid) {
        if (!cid) return "";
        if (cid.length <= 20) return cid;
        return cid.slice(0, 8) + "..." + cid.slice(-8);
    }

    function statusLabel(pkg) {
        if (pkg.status === "published") return "Published";
        if (pkg.status === "external") return "External";
        return "Local";
    }

    function isSelectable(pkg) {
        return !!pkg.cid;
    }

    function selectPackage(pkg) {
        if (!pkg.cid) return;
        value = pkg.cid;
        showDropdown = false;
        pickerSearch = "";
    }

    function selectCustom() {
        showDropdown = false;
        pickerSearch = "";
    }

    async function toggleDropdown(e) {
        e.stopPropagation();
        showDropdown = !showDropdown;
        if (showDropdown) {
            pickerSearch = "";
            pickerError = "";
            await refreshPackages();
        }
    }

    function closeDropdown() {
        showDropdown = false;
        pickerSearch = "";
        pickerError = "";
    }

    function toggleFolder(folder) {
        collapsedFolders = { ...collapsedFolders, [folder]: !collapsedFolders[folder] };
    }

    function openLibrary() {
        dispatch("openLibrary");
        closeDropdown();
    }

    function handleKeydown(e) {
        if (e.key === "Escape") closeDropdown();
    }

    async function refreshPackages() {
        $libraryLoading = true;
        pickerError = "";
        try {
            $contentLibrary = await core.invoke("content_library_list");
        } catch (err) {
            pickerError = String(err);
        } finally {
            $libraryLoading = false;
        }
    }
</script>

<svelte:window on:click={showDropdown ? closeDropdown : undefined} on:keydown={handleKeydown} />

<div class="content-picker" bind:this={pickerEl}>
    <label for={id}>{label}</label>

    <div class="picker-input-row">
        <input
            {id}
            type="text"
            class="cyber-input mono"
            bind:value
            {placeholder}
            {disabled}
            on:focus={() => (showDropdown = false)}
        />
        <button
            class="picker-trigger"
            type="button"
            on:click={toggleDropdown}
            title="Browse Content Library"
            {disabled}
        >
            <span class="picker-arrow" class:open={showDropdown}>▼</span>
        </button>
    </div>

    {#if showDropdown}
        <div class="picker-dropdown" transition:fly={{ y: -6, duration: 150 }}>
            <!-- Search inside dropdown -->
            <div class="dropdown-search">
                <input
                    type="text"
                    class="search-input mono"
                    bind:value={pickerSearch}
                    placeholder="Search name, CID, tags, folder..."
                />
            </div>
            <div class="dropdown-tools">
                <button class="library-link" type="button" on:click={openLibrary}>
                    Go to Content Library
                </button>
            </div>

            <div class="dropdown-divider"></div>

            <!-- Custom option -->
            <button
                class="dropdown-row custom"
                type="button"
                on:click={selectCustom}
            >
                <span class="row-name">Custom hash</span>
                <span class="row-cid mono">{shortCid(value) || "Type your own"}</span>
                <span class="row-status"></span>
            </button>

            <div class="dropdown-divider"></div>

            <!-- Header -->
            <div class="dropdown-header">
                <span class="header-col">NAME</span>
                <span class="header-col">CID</span>
                <span class="header-col right">STATUS</span>
            </div>

            <!-- Package rows -->
            {#if pickerError}
                <div class="dropdown-empty error">{pickerError}</div>
            {:else if $libraryLoading}
                <div class="dropdown-empty">Loading...</div>
            {:else if pickerPackages.length === 0}
                <div class="dropdown-empty">No packages found.</div>
            {:else}
                {#each pickerGroups as group (group.folder)}
                    <button class="folder-row" type="button" on:click={() => toggleFolder(group.folder)}>
                        <span class="folder-name">{collapsedFolders[group.folder] ? "▶" : "▼"} {group.folder}</span>
                        <span class="folder-count">{group.rows.length}</span>
                    </button>
                    {#if !collapsedFolders[group.folder]}
                        {#each group.rows as pkg (pkg.id)}
                            <button
                                class="dropdown-row"
                                type="button"
                                class:selected={pkg.cid === value}
                                class:disabled={!isSelectable(pkg)}
                                on:click={() => selectPackage(pkg)}
                                disabled={!isSelectable(pkg)}
                            >
                                <span class="row-name" title={pkg.name}>{pkg.name}</span>
                                <span class="row-cid mono" title={pkg.cid || ""}>{pkg.cid ? shortCid(pkg.cid) : "No CID yet"}</span>
                                <span class="row-status" class:published={pkg.status === "published"} class:external={pkg.status === "external"}>
                                    {statusLabel(pkg)}
                                </span>
                            </button>
                        {/each}
                    {/if}
                {/each}
            {/if}
        </div>
    {/if}
</div>

<style>
    .content-picker {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }

    label {
        color: #888;
        font-size: 0.65rem;
        letter-spacing: 0.5px;
        display: block;
        margin-bottom: 0.15rem;
    }

    .picker-input-row {
        display: flex;
        align-items: center;
        gap: 0.35rem;
    }

    .cyber-input {
        flex: 1;
        padding: 0.45rem 0.6rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        outline: none;
        transition: all 0.2s;
        box-sizing: border-box;
        min-width: 0;
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.03);
    }
    .cyber-input:focus {
        border-color: var(--color-primary);
    }
    .cyber-input::placeholder {
        color: #555;
    }

    .picker-trigger {
        flex-shrink: 0;
        width: 32px;
        height: 32px;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-bottom: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.55rem;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all 0.15s;
        box-sizing: border-box;
        box-shadow: inset 0 0 0 1px rgba(0, 255, 65, 0.08);
    }
    .picker-trigger:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
    }
    .picker-trigger:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .picker-arrow {
        transition: transform 0.2s;
        display: inline-block;
    }
    .picker-arrow.open {
        transform: rotate(180deg);
    }

    .picker-dropdown {
        position: absolute;
        top: calc(100% + 4px);
        left: 0;
        right: 0;
        z-index: 500;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.7);
        max-height: 280px;
        overflow-y: auto;
        padding: 0.4rem 0.55rem 0.55rem;
        scrollbar-gutter: stable;
    }
    .picker-dropdown::-webkit-scrollbar {
        width: 8px;
    }
    .picker-dropdown::-webkit-scrollbar-track {
        background: transparent;
    }
    .picker-dropdown::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.12);
        border-radius: 4px;
    }
    .picker-dropdown::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.3);
    }

    .dropdown-search {
        padding: 0.25rem 0.3rem;
    }
    .dropdown-tools {
        padding: 0.15rem 0.3rem 0.25rem;
    }
    .library-link {
        width: 100%;
        text-align: left;
        border: 1px solid rgba(0, 255, 65, 0.25);
        background: rgba(0, 255, 65, 0.08);
        color: var(--color-primary);
        border-radius: 6px;
        padding: 0.35rem 0.45rem;
        font-size: 0.65rem;
        cursor: pointer;
    }
    .library-link:hover {
        background: rgba(0, 255, 65, 0.16);
    }
    .search-input {
        width: 100%;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.7rem;
        outline: none;
        box-sizing: border-box;
    }
    .search-input:focus {
        border-color: var(--color-primary);
    }
    .search-input::placeholder {
        color: #555;
    }

    .dropdown-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.15rem 0;
    }

    .dropdown-header {
        display: grid;
        grid-template-columns: 1.2fr 1fr 80px;
        gap: 0.4rem;
        padding: 0.25rem 0.5rem;
        font-size: 0.55rem;
        color: #555;
        letter-spacing: 0.5px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .header-col {
        font-weight: 600;
    }
    .header-col.right {
        text-align: right;
    }

    .dropdown-row {
        display: grid;
        grid-template-columns: 1.2fr 1fr 80px;
        gap: 0.4rem;
        align-items: center;
        padding: 0.4rem 0.5rem;
        background: none;
        border: none;
        border-radius: 4px;
        color: #ccc;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        transition: all 0.1s;
        width: 100%;
    }
    .folder-row {
        display: flex;
        width: 100%;
        justify-content: space-between;
        align-items: center;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        margin: 0.2rem 0;
        color: #aaa;
        font-size: 0.62rem;
        padding: 0.22rem 0.45rem;
        cursor: pointer;
    }
    .folder-name {
        text-transform: uppercase;
        letter-spacing: 0.4px;
    }
    .folder-count {
        color: #666;
        font-family: var(--font-mono);
    }
    .dropdown-row:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.06);
    }
    .dropdown-row.selected {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
    }
    .dropdown-row.disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .dropdown-row.custom {
        color: #888;
        font-style: italic;
    }
    .dropdown-row.custom .row-name {
        color: #aaa;
    }

    .row-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: #ddd;
    }
    .row-cid {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: #888;
        font-size: 0.65rem;
    }
    .row-status {
        text-align: right;
        font-size: 0.6rem;
        color: #666;
    }
    .row-status.published {
        color: var(--color-primary);
    }
    .row-status.external {
        color: #ffaa00;
    }

    .dropdown-empty {
        padding: 0.6rem 0.5rem;
        text-align: center;
        color: #555;
        font-size: 0.7rem;
    }
    .dropdown-empty.error {
        color: #ff5555;
    }
</style>
