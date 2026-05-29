<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import { contentLibrary, ipfsHubSection } from "../stores/contentLibrary.js";

    export let pkg;
    const dispatch = createEventDispatcher();
    let deleteConfirm = false;
    let deleting = false;
    let cidCopied = false;
    let duplicating = false;

    function formatDate(iso) {
        if (!iso) return "Unknown";
        return iso.slice(0, 10);
    }

    async function deletePackage() {
        if (deleting) return;
        deleting = true;
        try {
            await core.invoke("content_library_delete", { packageId: pkg.id });
            dispatch("refresh");
        } catch (err) {
            alert("Delete failed: " + err);
        }
        deleting = false;
        deleteConfirm = false;
    }

    async function dupPackage() {
        if (duplicating) return;
        duplicating = true;
        try {
            await core.invoke("content_library_duplicate", { packageId: pkg.id });
            dispatch("refresh");
        } catch (err) {
            alert("Duplicate failed: " + err);
        }
        duplicating = false;
    }

    function filePreview(files) {
        const md = files && files.find((f) => f.path === "content.md");
        if (md) return "Markdown document";
        if (files && files.length > 0) return `${files.length} file${files.length > 1 ? "s" : ""}`;
        return "Empty package";
    }

    async function copyCid() {
        if (!pkg.cid) return;
        cidCopied = true;
        try {
            await navigator.clipboard.writeText(pkg.cid);
        } catch {
            // Fallback
            const ta = document.createElement("textarea");
            ta.value = pkg.cid;
            document.body.appendChild(ta);
            ta.select();
            document.execCommand("copy");
            document.body.removeChild(ta);
        }
        setTimeout(() => (cidCopied = false), 1500);
    }
</script>

<div class="package-card glass-card" class:has-confirm={deleteConfirm}>
    <div class="card-name">{pkg.name}</div>

    {#if pkg.description}
        <div class="card-desc">{pkg.description}</div>
    {/if}

    <div class="card-meta">
        <span class="meta-item">
            <span class="meta-label">v{pkg.version}</span>
        </span>
        <span class="meta-item">
            <span class="meta-label">{pkg.file_count} file{pkg.file_count !== 1 ? "s" : ""}</span>
        </span>
        <span class="meta-item">
            <span class="meta-label">{formatDate(pkg.updated_at)}</span>
        </span>
    </div>

    {#if pkg.cid}
        <div class="card-cid">
            <span class="cid-label">CID: </span>
            <span class="cid-hash mono" title={pkg.cid}>
                {pkg.cid.length > 20 ? pkg.cid.slice(0, 10) + "..." + pkg.cid.slice(-10) : pkg.cid}
            </span>
            <button class="cid-copy-btn" on:click={copyCid} title="Copy CID">
                {cidCopied ? "copied" : "copy"}
            </button>
        </div>
    {/if}

    {#if pkg.tags && pkg.tags.length > 0}
        <div class="card-tags">
            {#each pkg.tags as tag}
                <span class="tag-chip">{tag}</span>
            {/each}
        </div>
    {/if}

    <div class="actions-wrap">
        <div class="card-actions">
            <button class="action-btn" on:click|stopPropagation={() => dispatch("edit")} title="Edit Package">EDIT</button>
            <button class="action-btn" on:click|stopPropagation={() => dispatch("view")} title="View Details">VIEW</button>
            <button class="action-btn" on:click|stopPropagation={dupPackage} disabled={duplicating} title="Duplicate Package">
                {duplicating ? "..." : "DUP"}
            </button>
            <button class="action-btn danger" on:click|stopPropagation={() => (deleteConfirm = true)} title="Delete Package">DEL</button>
        </div>
        {#if deleteConfirm}
            <div class="delete-overlay" transition:fade={{ duration: 120 }}>
                <span class="delete-overlay-text">Delete?</span>
                <button class="action-btn danger confirm" on:click|stopPropagation={deletePackage} disabled={deleting}>
                    {deleting ? "..." : "YES"}
                </button>
                <button class="action-btn" on:click|stopPropagation={() => (deleteConfirm = false)}>NO</button>
            </div>
        {/if}
    </div>
</div>

<style>
    .package-card {
        position: relative;
        background: rgba(0, 0, 0, 0.35);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 1rem;
        display: flex;
        flex-direction: column;
        transition: all 0.25s;
    }
    .package-card:hover {
        border-color: rgba(0, 255, 65, 0.3);
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
    }
    .package-card.has-confirm {
        border-color: rgba(255, 68, 68, 0.3);
    }
    .card-name {
        font-size: 0.9rem;
        font-weight: 700;
        color: #fff;
        margin-bottom: 0.3rem;
        letter-spacing: 0.5px;
        text-overflow: ellipsis;
        overflow: hidden;
        white-space: nowrap;
    }
    .card-desc {
        font-size: 0.7rem;
        color: #888;
        line-height: 1.4;
        margin-bottom: 0.6rem;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .card-meta {
        display: flex;
        gap: 0.75rem;
        margin-bottom: 0.5rem;
    }
    .meta-item {
        font-size: 0.6rem;
        color: #555;
    }
    .card-cid {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        margin-bottom: 0.5rem;
        padding: 0.3rem 0.4rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.55rem;
    }
    .cid-label {
        color: #555;
        flex-shrink: 0;
    }
    .cid-hash {
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .cid-copy-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: #666;
        font-size: 0.5rem;
        padding: 2px 6px;
        border-radius: 3px;
        cursor: pointer;
        flex-shrink: 0;
    }
    .cid-copy-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .card-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 0.3rem;
        margin-bottom: 0.75rem;
    }
    .tag-chip {
        font-size: 0.55rem;
        padding: 2px 6px;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.15);
        color: var(--color-primary);
        border-radius: 3px;
    }
    .card-actions {
        display: flex;
        gap: 0.4rem;
        margin-top: auto;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .action-btn {
        background: transparent;
        border: 1.5px solid rgba(255, 255, 255, 0.14);
        color: #aaa;
        padding: 0.3rem 0.6rem;
        font-size: 0.6rem;
        letter-spacing: 1px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .action-btn:hover {
        border-color: rgba(0, 255, 65, 0.45);
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.05);
    }
    .action-btn.danger {
        border-color: rgba(255, 68, 68, 0.25);
        color: #c66;
    }
    .action-btn.danger:hover {
        border-color: rgba(255, 68, 68, 0.55);
        color: #ff5555;
    }
    .action-btn.confirm {
        border-color: rgba(255, 68, 68, 0.45);
        color: #ff5555;
        background: rgba(255, 68, 68, 0.08);
    }
    .action-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .actions-wrap {
        position: relative;
        margin-top: auto;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .card-actions {
        display: flex;
        gap: 0.4rem;
    }
    .delete-overlay {
        position: absolute;
        inset: -0.25rem -0.25rem -0.25rem -0.25rem;
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: 0.3rem;
        background: rgba(30, 10, 10, 0.92);
        border: 1px solid rgba(255, 68, 68, 0.25);
        border-radius: 6px;
        padding: 0 0.5rem;
        backdrop-filter: blur(2px);
    }
    .delete-overlay-text {
        color: #c66;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
        margin-right: 0.2rem;
    }
</style>
