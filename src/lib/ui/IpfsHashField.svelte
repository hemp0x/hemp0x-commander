<script>
    import { core } from "@tauri-apps/api";
    import { fly, fade } from "svelte/transition";
    import { contentLibrary, libraryLoading } from "../stores/contentLibrary.js";

    export let value = "";
    export let id = "ipfs-hash";
    export let disabled = false;

    let showPicker = false;
    let pickerSearch = "";
    let pickerError = "";

    $: pickerPackages = filterPackages($contentLibrary, pickerSearch);

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

    function shortCid(cid) {
        if (!cid) return "";
        if (cid.length <= 20) return cid;
        return cid.slice(0, 8) + "..." + cid.slice(-8);
    }

    function selectPackage(pkg) {
        if (!pkg.cid) return;
        value = pkg.cid;
        showPicker = false;
        pickerSearch = "";
    }

    async function copyCid(cid) {
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

    async function openPicker() {
        showPicker = true;
        pickerSearch = "";
        pickerError = "";
        await refreshPackages();
    }

    function closePicker() {
        showPicker = false;
        pickerSearch = "";
    }

    function statusLabel(pkg) {
        if (pkg.status === "published") return "Published";
        if (pkg.status === "external") return "External CID";
        return "Local Only";
    }

    function isSelectable(pkg) {
        return !!pkg.cid;
    }

    function formatDate(iso) {
        if (!iso) return "";
        return iso.slice(0, 10);
    }
</script>

<div class="ipfs-hash-field">
    <div class="ipfs-input-row">
        <input
            {id}
            type="text"
            class="glass-input mono"
            placeholder="Qm..."
            bind:value
            {disabled}
        />
        <button
            class="library-btn"
            type="button"
            on:click={openPicker}
            {disabled}
            title="Browse Content Library"
        >
            LIBRARY
        </button>
    </div>
</div>

{#if showPicker}
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
        class="picker-overlay"
        transition:fade={{ duration: 150 }}
        on:click={closePicker}
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === "Escape" && closePicker()}
    >
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="picker-modal" on:click|stopPropagation transition:fly={{ y: 20 }}>
            <div class="picker-header">
                <span class="picker-title">SELECT CONTENT PACKAGE</span>
                <button class="picker-close" on:click={closePicker} type="button">×</button>
            </div>

            <div class="picker-search-row">
                <input
                    type="text"
                    class="picker-search-input mono"
                    placeholder="Search name, tag, folder, CID..."
                    bind:value={pickerSearch}
                />
            </div>

            <div class="picker-list">
                {#if pickerError}
                    <div class="picker-empty error">{pickerError}</div>
                {:else if $libraryLoading}
                    <div class="picker-empty">Loading Content Library...</div>
                {:else if pickerPackages.length === 0}
                    <div class="picker-empty">No packages match your search.</div>
                {:else}
                    {#each pickerPackages as pkg (pkg.id)}
                        <div class="picker-row" class:disabled={!isSelectable(pkg)}>
                            <div class="picker-info">
                                <div class="picker-name-row">
                                    <span class="picker-name">{pkg.name}</span>
                                    <span class="picker-status" class:published={pkg.status === "published"} class:external={pkg.status === "external"}>
                                        {statusLabel(pkg)}
                                    </span>
                                </div>
                                <div class="picker-meta">
                                    {#if pkg.cid}
                                        <span class="picker-cid mono" title={pkg.cid}>{shortCid(pkg.cid)}</span>
                                    {:else}
                                        <span class="picker-cid missing">No CID</span>
                                    {/if}
                                    {#if pkg.folder}
                                        <span class="picker-folder">{pkg.folder}</span>
                                    {/if}
                                    {#if pkg.tags && pkg.tags.length > 0}
                                        <span class="picker-tags">{pkg.tags.slice(0, 3).join(", ")}{pkg.tags.length > 3 ? " +" + (pkg.tags.length - 3) : ""}</span>
                                    {/if}
                                </div>
                                <div class="picker-dates">
                                    <span>Updated {formatDate(pkg.updated_at)}</span>
                                    {#if pkg.published_at}
                                        <span>Published {formatDate(pkg.published_at)}</span>
                                    {/if}
                                </div>
                            </div>
                            <div class="picker-actions">
                                {#if pkg.cid}
                                    <button
                                        class="picker-btn select"
                                        type="button"
                                        on:click={() => selectPackage(pkg)}
                                    >
                                        SELECT
                                    </button>
                                    <button
                                        class="picker-btn copy"
                                        type="button"
                                        on:click={() => copyCid(pkg.cid)}
                                    >
                                        COPY CID
                                    </button>
                                {:else}
                                    <span class="picker-notice">Publish or link a CID first</span>
                                {/if}
                            </div>
                        </div>
                    {/each}
                {/if}
            </div>
        </div>
    </div>
{/if}

<style>
    .ipfs-hash-field {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        width: 100%;
    }

    .ipfs-input-row {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .ipfs-input-row .glass-input {
        flex: 1;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.7rem 1rem;
        font-size: 0.85rem;
        border-radius: 8px;
        outline: none;
        transition: all 0.2s;
        backdrop-filter: blur(5px);
    }

    .ipfs-input-row .glass-input:focus {
        border-color: var(--color-primary);
        box-shadow:
            0 0 20px rgba(0, 255, 65, 0.15),
            inset 0 0 20px rgba(0, 255, 65, 0.03);
    }

    .library-btn {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
        padding: 0.7rem 1rem;
        font-size: 0.65rem;
        font-weight: 700;
        letter-spacing: 1px;
        border-radius: 8px;
        cursor: pointer;
        transition: all 0.2s;
        white-space: nowrap;
    }

    .library-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.3);
    }

    .library-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    /* Picker Overlay */
    .picker-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10000;
        padding: 1rem;
    }

    .picker-modal {
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        width: 100%;
        max-width: 600px;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        box-shadow:
            0 0 80px rgba(0, 0, 0, 0.8),
            0 0 40px rgba(0, 255, 65, 0.1);
    }

    .picker-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.8rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }

    .picker-title {
        color: var(--color-primary);
        font-size: 0.75rem;
        font-weight: 600;
        letter-spacing: 1.5px;
    }

    .picker-close {
        background: transparent;
        border: none;
        color: #555;
        font-size: 1.2rem;
        cursor: pointer;
        width: 28px;
        height: 28px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 6px;
        transition: all 0.15s;
    }

    .picker-close:hover {
        color: #fff;
        background: rgba(255, 255, 255, 0.1);
    }

    .picker-search-row {
        padding: 0.6rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }

    .picker-search-input {
        width: 100%;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.5rem 0.75rem;
        font-size: 0.75rem;
        border-radius: 6px;
        outline: none;
        transition: all 0.2s;
    }

    .picker-search-input:focus {
        border-color: var(--color-primary);
    }

    .picker-list {
        flex: 1;
        overflow-y: auto;
        padding: 0.5rem;
    }

    .picker-empty {
        text-align: center;
        padding: 2rem;
        color: #555;
        font-size: 0.7rem;
    }

    .picker-empty.error {
        color: #ff6b6b;
    }

    .picker-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.75rem;
        padding: 0.6rem 0.75rem;
        border-radius: 6px;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.04);
        margin-bottom: 0.35rem;
        transition: all 0.15s;
    }

    .picker-row:not(.disabled):hover {
        border-color: rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.03);
    }

    .picker-row.disabled {
        opacity: 0.6;
    }

    .picker-info {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }

    .picker-name-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .picker-name {
        font-size: 0.75rem;
        font-weight: 600;
        color: #fff;
        letter-spacing: 0.5px;
    }

    .picker-status {
        font-size: 0.5rem;
        padding: 2px 6px;
        border-radius: 4px;
        background: rgba(255, 255, 255, 0.06);
        color: #777;
        letter-spacing: 0.5px;
        text-transform: uppercase;
    }

    .picker-status.published {
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }

    .picker-status.external {
        background: rgba(255, 170, 0, 0.1);
        color: #ffaa00;
    }

    .picker-meta {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .picker-cid {
        font-size: 0.55rem;
        color: #888;
    }

    .picker-cid.missing {
        color: #555;
    }

    .picker-folder {
        font-size: 0.55rem;
        color: #666;
    }

    .picker-tags {
        font-size: 0.5rem;
        color: #555;
    }

    .picker-dates {
        display: flex;
        gap: 0.5rem;
        font-size: 0.5rem;
        color: #444;
    }

    .picker-actions {
        display: flex;
        gap: 0.35rem;
        flex-shrink: 0;
    }

    .picker-btn {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.35rem 0.6rem;
        color: #aaa;
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        white-space: nowrap;
    }

    .picker-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }

    .picker-btn.select {
        background: rgba(0, 255, 65, 0.08);
        border-color: rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
    }

    .picker-btn.select:hover {
        background: var(--color-primary);
        color: #000;
    }

    .picker-notice {
        font-size: 0.5rem;
        color: #555;
        white-space: nowrap;
    }

    @media (max-width: 680px) {
        .ipfs-input-row,
        .picker-row,
        .picker-actions {
            flex-direction: column;
            align-items: stretch;
        }

        .library-btn,
        .picker-btn {
            width: 100%;
        }
    }
</style>
