<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";

    export let editPackage = null;
    const dispatch = createEventDispatcher();

    let name = "";
    let description = "";
    let tagsStr = "";
    let body = "";
    let saving = false;
    let errorMsg = "";

    $: isEdit = !!editPackage;
    $: title = isEdit ? "EDIT PACKAGE" : "NEW PACKAGE";

    if (editPackage) {
        name = editPackage.name;
        description = editPackage.description || "";
        tagsStr = (editPackage.tags || []).join(", ");
        body = "...";
    }

    async function initBody() {
        if (!editPackage) return;
        const mdFile = (editPackage.files || []).find((f) => f.path === "content.md");
        if (!mdFile) { body = ""; return; }
        try {
            const result = await core.invoke("content_library_get_file", {
                packageId: editPackage.id,
                filePath: "content.md",
            });
            const bytes = Uint8Array.from(atob(result.content_base64), (c) => c.charCodeAt(0));
            body = new TextDecoder().decode(bytes);
        } catch (err) {
            console.warn("Failed to load content.md body:", err);
            body = "";
        }
    }

    onMount(() => {
        initBody();
    });

    function parseTags(str) {
        return str
            .split(",")
            .map((t) => t.trim().toLowerCase())
            .filter((t) => t.length > 0);
    }

    async function save() {
        saving = true;
        errorMsg = "";

        const trimmedName = name.trim();
        if (!trimmedName) {
            errorMsg = "Package name is required.";
            saving = false;
            return;
        }

        const input = {
            name: trimmedName,
            description: description.trim() || "",
            tags: parseTags(tagsStr),
            body: body || "",
            files: [],
        };

        try {
            if (isEdit) {
                await core.invoke("content_library_update", {
                    packageId: editPackage.id,
                    input,
                });
            } else {
                await core.invoke("content_library_create", { input });
            }
            dispatch("saved");
        } catch (err) {
            errorMsg = String(err);
        }
        saving = false;
    }

    function cancel() {
        dispatch("cancel");
    }
</script>

<div class="composer" in:fade={{ duration: 150 }}>
    <h3 class="composer-title">{title}</h3>

    {#if errorMsg}
        <div class="error-bar">{errorMsg}</div>
    {/if}

    <div class="form-group">
        <label class="form-label" for="package-name">NAME</label>
        <input id="package-name" class="form-input mono" type="text" bind:value={name} placeholder="Package name" />
    </div>

    <div class="form-group">
        <label class="form-label" for="package-description">DESCRIPTION</label>
        <input id="package-description" class="form-input mono" type="text" bind:value={description} placeholder="Short description" />
    </div>

    <div class="form-group">
        <label class="form-label" for="package-tags">TAGS (comma-separated)</label>
        <input id="package-tags" class="form-input mono" type="text" bind:value={tagsStr} placeholder="nft, metadata, art" />
    </div>

    <div class="form-group">
        <label class="form-label" for="package-body">CONTENT (markdown)</label>
        <textarea id="package-body" class="form-textarea mono" bind:value={body} placeholder="Write your package content in markdown..."></textarea>
    </div>

    <div class="form-actions">
        <button class="cyber-btn" on:click={save} disabled={saving}>
            {saving ? "SAVING..." : isEdit ? "SAVE CHANGES" : "CREATE PACKAGE"}
        </button>
        <button class="cyber-btn ghost" on:click={cancel} disabled={saving}>
            CANCEL
        </button>
    </div>
</div>

<style>
    .composer {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1.2rem 1.5rem;
        max-width: 720px;
        max-height: calc(100vh - 300px);
        overflow-y: auto;
    }
    .composer-title {
        font-size: 0.85rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0 0 1.2rem 0;
    }
    .error-bar {
        padding: 0.4rem 0.8rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.25);
        color: #ff6666;
        font-size: 0.75rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        font-family: var(--font-mono);
    }
    .form-group {
        margin-bottom: 1rem;
    }
    .form-label {
        display: block;
        font-size: 0.65rem;
        letter-spacing: 1px;
        color: #555;
        margin-bottom: 0.35rem;
    }
    .form-input {
        width: 100%;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.5rem 0.75rem;
        border-radius: 4px;
        font-size: 0.8rem;
        outline: none;
        box-sizing: border-box;
    }
    .form-input:focus {
        border-color: var(--color-primary);
    }
    .form-input::placeholder {
        color: #444;
    }
    .form-textarea {
        width: 100%;
        height: 200px;
        background: #000;
        border: 1px solid #333;
        color: #afa;
        padding: 0.6rem 0.75rem;
        border-radius: 4px;
        font-size: 0.8rem;
        outline: none;
        resize: vertical;
        box-sizing: border-box;
    }
    .form-textarea:focus {
        border-color: var(--color-primary);
    }
    .form-textarea::placeholder {
        color: #444;
    }
    .form-actions {
        display: flex;
        gap: 0.75rem;
        margin-top: 1.25rem;
    }
    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.5rem 1.2rem;
        letter-spacing: 1px;
        font-weight: bold;
        font-size: 0.7rem;
        cursor: pointer;
        text-transform: uppercase;
        transition: all 0.2s;
    }
    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
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
    .cyber-btn.ghost:hover:not(:disabled) {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }
</style>
