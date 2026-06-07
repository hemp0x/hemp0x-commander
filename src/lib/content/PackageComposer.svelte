<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";

    /**
     * @typedef {Object} ExistingFile
     * @property {string} path
     * @property {string} [mime]
     * @property {number} size_bytes
     */

    /**
     * @typedef {Object} PendingFile
     * @property {string} path
     * @property {string} mime
     * @property {number} size_bytes
     * @property {string} content_base64
     */

    /**
     * @typedef {{ path: string, mime: string, content_base64: string }} FileInput
     */

    /**
     * @typedef {Object} EditPackageData
     * @property {string} id
     * @property {string} name
     * @property {string} [description]
     * @property {string[]} [tags]
     * @property {ExistingFile[]} [files]
     * @property {string} [folder]
     * @property {string} [cid]
     * @property {string} [status]
     * @property {string} [version]
     * @property {string} [updated_at]
     * @property {string} [created_at]
     * @property {number} [file_count]
     * @property {string} [provider]
     * @property {string} [published_at]
     */

    /**
     * @typedef {Object} SavePayload
     * @property {string} name
     * @property {string} description
     * @property {string[]} tags
     * @property {string} body
     * @property {FileInput[]} [files]
     * @property {string} [folder]
     */

    /** @type {EditPackageData | null} */
    export let editPackage = null;
    /** @type {string | null} */
    export let createFolder = null;
    const dispatch = createEventDispatcher();

    let name = "";
    let description = "";
    let tagsStr = "";
    let body = "";
    let saving = false;
    let errorMsg = "";
    let showPreview = false;
    /** @type {PendingFile[]} */
    let pendingFiles = [];
    /** @type {ExistingFile[]} */
    let existingFiles = [];
    /** @type {string[]} */
    let removingFiles = [];

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
        if (editPackage.files) {
            existingFiles = editPackage.files.filter((f) => f.path !== "content.md");
        }
    }

    onMount(() => {
        initBody();
    });

    /** @param {string} str */
    function parseTags(str) {
        return str
            .split(",")
            .map((t) => t.trim().toLowerCase())
            .filter((t) => t.length > 0);
    }

    /** @param {number} bytes */
    function formatSize(bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    }

    /** @param {number} idx */
    function removePendingFile(idx) {
        pendingFiles.splice(idx, 1);
        pendingFiles = pendingFiles;
    }

    /** @param {number} idx */
    function removeExistingFile(idx) {
        const file = existingFiles[idx];
        removingFiles.push(file.path);
        existingFiles.splice(idx, 1);
        existingFiles = existingFiles;
    }

    /** @param {{ mime?: string }} file */
    function isImageFile(file) {
        const mime = file.mime || "";
        return mime.startsWith("image/");
    }

    /** @param {DragEvent} e */
    function handleDrop(e) {
        e.preventDefault();
        if (!e.dataTransfer || !e.dataTransfer.files) return;
        processFiles(Array.from(e.dataTransfer.files));
    }

    /** @param {DragEvent} e */
    function handleDragOver(e) {
        e.preventDefault();
    }

    function handleFileSelect() {
        const input = document.createElement("input");
        input.type = "file";
        input.multiple = true;
        input.onchange = () => {
            if (input.files) processFiles(Array.from(input.files));
        };
        input.click();
    }

    /** @param {File[]} files */
    async function processFiles(files) {
        for (const file of files) {
            try {
                const data = await file.arrayBuffer();
                const bytes = new Uint8Array(data);
                const base64 = bytesToBase64(bytes);
                const mime = file.type || guessMimeFromName(file.name);
                pendingFiles = [
                    ...pendingFiles,
                    {
                        path: file.name,
                        mime,
                        size_bytes: bytes.length,
                        content_base64: base64,
                    },
                ];
            } catch (err) {
                console.warn("Failed to read file:", file.name, err);
            }
        }
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

    /** @param {string} name */
    function guessMimeFromName(name) {
        const ext = (name.split(".").pop() || "").toLowerCase();
        /** @type {Record<string, string>} */
        const map = {
            md: "text/markdown",
            json: "application/json",
            txt: "text/plain",
            png: "image/png",
            jpg: "image/jpeg",
            jpeg: "image/jpeg",
            gif: "image/gif",
            webp: "image/webp",
            svg: "image/svg+xml",
            pdf: "application/pdf",
            html: "text/html",
            css: "text/css",
            js: "application/javascript",
        };
        return map[ext] || "application/octet-stream";
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

        const fileInputs = pendingFiles.map((f) => ({
            path: f.path,
            mime: f.mime,
            content_base64: f.content_base64,
        }));

        // Include existing files not marked for removal
        if (editPackage) {
            for (const f of existingFiles) {
                if (!removingFiles.includes(f.path)) {
                    try {
                        const result = await core.invoke("content_library_get_file", {
                            packageId: editPackage.id,
                            filePath: f.path,
                        });
                        fileInputs.push({
                            path: f.path,
                            mime: f.mime || result.mime,
                            content_base64: result.content_base64,
                        });
                    } catch (err) {
                        console.warn("Failed to read existing file:", f.path, err);
                    }
                }
            }
        }

        const input = {
            name: trimmedName,
            description: description.trim() || "",
            tags: parseTags(tagsStr),
            body: body || "",
            files: fileInputs.length > 0 ? fileInputs : undefined,
            folder: isEdit ? undefined : (createFolder || undefined),
        };

        try {
            if (editPackage) {
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

    /** @param {string} syntax */
    function insertMarkdown(syntax) {
        const textarea = /** @type {HTMLTextAreaElement | null} */ (document.getElementById("package-body"));
        if (!textarea) return;
        const start = textarea.selectionStart;
        const end = textarea.selectionEnd;
        const selected = body.substring(start, end);

        let replacement = "";
        let cursorOffset = 0;
        switch (syntax) {
            case "heading":
                replacement = "## " + (selected || "Heading");
                cursorOffset = selected ? replacement.length : 3;
                break;
            case "bold":
                replacement = "**" + (selected || "bold text") + "**";
                cursorOffset = replacement.length - 2;
                break;
            case "italic":
                replacement = "*" + (selected || "italic text") + "*";
                cursorOffset = replacement.length - 1;
                break;
            case "link":
                replacement = "[" + (selected || "link text") + "](url)";
                cursorOffset = replacement.length - (selected ? 1 : 4);
                break;
            case "image":
                replacement = "![" + (selected || "alt text") + "](url)";
                cursorOffset = replacement.length - (selected ? 1 : 4);
                break;
            case "code":
                replacement = "```\n" + (selected || "code") + "\n```";
                cursorOffset = selected ? replacement.length - 3 : 4;
                break;
            case "ulist":
                replacement = "- " + (selected || "item");
                cursorOffset = replacement.length;
                break;
            case "json":
                replacement = "```json\n" + (selected || "{\n  \"key\": \"value\"\n}") + "\n```";
                cursorOffset = selected ? replacement.length - 3 : 7;
                break;
            default:
                replacement = selected;
                cursorOffset = replacement.length;
        }

        body = body.substring(0, start) + replacement + body.substring(end);
        setTimeout(() => {
            textarea.focus();
            textarea.selectionStart = start + cursorOffset;
            textarea.selectionEnd = start + cursorOffset;
        }, 0);
    }

    /** @param {string} md */
    function renderPreview(md) {
        if (!md) return "";
        let html = md
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;");
        html = html.replace(/^### (.+)$/gm, "<h3>$1</h3>");
        html = html.replace(/^## (.+)$/gm, "<h2>$1</h2>");
        html = html.replace(/^# (.+)$/gm, "<h1>$1</h1>");
        html = html.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
        html = html.replace(/\*(.+?)\*/g, "<em>$1</em>");
        html = html.replace(/`([^`]+)`/g, "<code>$1</code>");
        html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
        html = html.replace(/^!\[([^\]]*)\]\(([^)]+)\)/gm, '<div class="md-image-ref">[image: $1] ($2)</div>');
        html = html.replace(/^- (.+)$/gm, "<li>$1</li>");
        html = html.replace(/(<li>.*<\/li>\n?)+/g, "<ul>$&</ul>");
        html = html.replace(/\n\n/g, "</p><p>");
        html = "<p>" + html + "</p>";
        html = html.replace(/<p><\/p>/g, "");
        html = html.replace(/<p><h([123])>/g, "<h$1>");
        html = html.replace(/<\/h([123])><\/p>/g, "</h$1>");
        html = html.replace(/<p><ul>/g, "<ul>");
        html = html.replace(/<\/ul><\/p>/g, "</ul>");
        return html;
    }
</script>

<div class="composer" in:fade={{ duration: 150 }}>
    <h3 class="composer-title">{title}</h3>

    {#if errorMsg}
        <div class="error-bar">{errorMsg}</div>
    {/if}

    <div class="composer-grid">
        <div class="form-col">
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
                <div class="label-row">
                    <label class="form-label" for="package-body">CONTENT (markdown)</label>
                    <button class="toggle-preview-btn" on:click={() => (showPreview = !showPreview)}>
                        {showPreview ? "EDIT" : "PREVIEW"}
                    </button>
                </div>
                <div class="md-toolbar">
                    <button class="md-btn" on:click={() => insertMarkdown("heading")} title="Heading">H</button>
                    <button class="md-btn" on:click={() => insertMarkdown("bold")} title="Bold">B</button>
                    <button class="md-btn" on:click={() => insertMarkdown("italic")} title="Italic">I</button>
                    <button class="md-btn" on:click={() => insertMarkdown("link")} title="Link">&gt;</button>
                    <button class="md-btn" on:click={() => insertMarkdown("image")} title="Image">img</button>
                      <button class="md-btn" on:click={() => insertMarkdown("code")} title="Code block">{"{ }"}</button>
                    <button class="md-btn" on:click={() => insertMarkdown("ulist")} title="Unordered list">-</button>
                    <button class="md-btn" on:click={() => insertMarkdown("json")} title="JSON template">{"{}"}</button>
                </div>
                {#if showPreview}
                    <div class="md-preview">
                        {@html renderPreview(body)}
                    </div>
                {:else}
                    <textarea id="package-body" class="form-textarea mono" bind:value={body} placeholder="Write your package content in markdown..."></textarea>
                {/if}
            </div>

            <!-- File Attachments -->
            <div class="form-group">
                <div class="form-label">FILES / ATTACHMENTS</div>
                <div
                    class="drop-zone"
                    role="group"
                    aria-label="File attachment drop zone"
                    on:dragover={handleDragOver}
                    on:drop={handleDrop}
                >
                    <div class="drop-message">
                        Drop files here or use the button to select
                    </div>
                    <button class="cyber-btn ghost small" on:click={handleFileSelect}>
                        ADD FILES
                    </button>
                </div>

                {#if existingFiles.length > 0}
                    <div class="file-section-label">Existing files:</div>
                    <div class="file-list">
                        {#each existingFiles as file, idx}
                            <div class="file-row">
                                <span class="file-icon">{isImageFile(file) ? "img" : "doc"}</span>
                                <span class="file-name mono">{file.path}</span>
                                <span class="file-size mono">{formatSize(file.size_bytes)}</span>
                                <span class="file-mime">{(file.mime || "").split("/")[1] || file.mime}</span>
                                <button class="file-remove" on:click={() => removeExistingFile(idx)} title="Remove">x</button>
                            </div>
                        {/each}
                    </div>
                {/if}

                {#if pendingFiles.length > 0}
                    <div class="file-section-label">New files:</div>
                    <div class="file-list">
                        {#each pendingFiles as file, idx}
                            <div class="file-row new">
                                <span class="file-icon">{isImageFile(file) ? "img" : "doc"}</span>
                                <span class="file-name mono">{file.path}</span>
                                <span class="file-size mono">{formatSize(file.size_bytes)}</span>
                                <span class="file-mime">{(file.mime || "").split("/")[1] || file.mime}</span>
                                <button class="file-remove" on:click={() => removePendingFile(idx)} title="Remove">x</button>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        </div>
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
        border-top: 2px solid rgba(0, 255, 65, 0.3);
        border-radius: 8px;
        padding: 0;
        max-width: 100%;
        display: flex;
        flex-direction: column;
    }
    .composer-title {
        font-size: 0.85rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0;
        padding: 1rem 1.5rem 0.6rem;
        flex-shrink: 0;
        border-bottom: 1px solid rgba(0, 255, 65, 0.06);
    }
    .error-bar {
        padding: 0.4rem 0.8rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.25);
        color: #ff6666;
        font-size: 0.75rem;
        border-radius: 4px;
        margin: 0 1.5rem 0.6rem;
        font-family: var(--font-mono);
        flex-shrink: 0;
    }
    .composer-grid {
        display: flex;
        gap: 1.5rem;
        padding: 0 1.5rem;
    }
    .form-col {
        flex: 1;
        min-width: 0;
    }
    .form-group {
        margin-bottom: 1rem;
    }
    .label-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.35rem;
    }
    .form-label {
        font-size: 0.65rem;
        letter-spacing: 1px;
        color: #555;
    }
    .toggle-preview-btn {
        font-size: 0.6rem;
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #888;
        padding: 2px 8px;
        border-radius: 3px;
        cursor: pointer;
    }
    .toggle-preview-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .md-toolbar {
        display: flex;
        gap: 2px;
        margin-bottom: 0.35rem;
        flex-wrap: wrap;
    }
    .md-btn {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: #666;
        font-size: 0.6rem;
        padding: 3px 8px;
        border-radius: 3px;
        cursor: pointer;
        font-weight: 600;
        transition: all 0.15s;
    }
    .md-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.05);
    }
    .md-preview {
        background: #000;
        border: 1px solid #333;
        border-radius: 4px;
        padding: 0.6rem 0.8rem;
        min-height: 200px;
        max-height: 400px;
        overflow-y: auto;
        color: #ccc;
        font-size: 0.78rem;
        line-height: 1.6;
    }
    .md-preview :global(h1) { font-size: 1.2rem; color: var(--color-primary); margin: 0.5rem 0; }
    .md-preview :global(h2) { font-size: 1rem; color: var(--color-primary); margin: 0.4rem 0; }
    .md-preview :global(h3) { font-size: 0.85rem; color: var(--color-primary); margin: 0.3rem 0; }
    .md-preview :global(strong) { color: #fff; }
    .md-preview :global(em) { color: #aaa; }
    .md-preview :global(code) {
        background: rgba(0, 255, 65, 0.1);
        padding: 2px 5px;
        border-radius: 3px;
        font-size: 0.72rem;
        color: var(--color-primary);
    }
    .md-preview :global(a) { color: var(--color-primary); }
    .md-preview :global(li) { margin: 0.2rem 0; padding-left: 0.5rem; }
    .md-preview :global(.md-image-ref) {
        color: #555;
        font-style: italic;
        padding: 0.3rem 0;
        border: 1px dashed rgba(255, 255, 255, 0.08);
        margin: 0.3rem 0;
        border-radius: 3px;
        font-size: 0.7rem;
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
    .drop-zone {
        border: 1px dashed rgba(255, 255, 255, 0.12);
        border-radius: 6px;
        padding: 1.25rem 1rem;
        text-align: center;
        background: rgba(0, 0, 0, 0.2);
        margin-bottom: 0.5rem;
        transition: all 0.2s;
    }
    .drop-zone:hover {
        border-color: rgba(0, 255, 65, 0.35);
        background: rgba(0, 255, 65, 0.03);
    }
    .drop-message {
        font-size: 0.65rem;
        color: #666;
        margin-bottom: 0.6rem;
        letter-spacing: 0.5px;
    }
    .file-section-label {
        font-size: 0.55rem;
        color: #555;
        letter-spacing: 1px;
        margin: 0.5rem 0 0.3rem 0;
    }
    .file-list {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .file-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.65rem;
    }
    .file-row.new {
        border-color: rgba(0, 255, 65, 0.12);
    }
    .file-icon {
        font-size: 0.5rem;
        color: #555;
        width: 24px;
        text-align: center;
        flex-shrink: 0;
    }
    .file-name {
        color: #aaa;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }
    .file-size {
        color: #555;
        font-size: 0.55rem;
        flex-shrink: 0;
    }
    .file-mime {
        color: #444;
        font-size: 0.5rem;
        flex-shrink: 0;
    }
    .file-remove {
        background: transparent;
        border: 1px solid rgba(255, 68, 68, 0.2);
        color: #c66;
        font-size: 0.55rem;
        padding: 1px 5px;
        border-radius: 3px;
        cursor: pointer;
        flex-shrink: 0;
    }
    .file-remove:hover {
        border-color: #ff5555;
        color: #ff5555;
    }
    .form-actions {
        display: flex;
        gap: 0.75rem;
        padding: 0.75rem 1.5rem;
        border-top: 1px solid rgba(0, 255, 65, 0.08);
        background: rgba(0, 0, 0, 0.2);
        flex-shrink: 0;
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
    .cyber-btn.small {
        padding: 0.3rem 0.8rem;
        font-size: 0.6rem;
    }
</style>
