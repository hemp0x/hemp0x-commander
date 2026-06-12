<script>
    import { onDestroy } from "svelte";
    import { core } from "@tauri-apps/api";

    export let contentBase64 = "";
    export let contentType = "text/plain";
    export let sizeBytes = 0;
    export let cid = "";

    let dataUrl = "";
    let textContent = "";
    let jsonContent = null;
    let hasError = false;
    let errorMsg = "";
    let decoded = false;
    let previousDataUrl = "";
    let statusMsg = "";
    let statusType = "";

    $: if (contentBase64) {
        decodeContent();
    }

    function decodeContent() {
        hasError = false;
        errorMsg = "";
        dataUrl = "";
        textContent = "";
        jsonContent = null;
        decoded = false;

        if (!contentBase64) return;

        try {
            const bytes = Uint8Array.from(atob(contentBase64), (c) => c.charCodeAt(0));

            if (contentType.startsWith("image/")) {
                if (previousDataUrl) URL.revokeObjectURL(previousDataUrl);
                const blob = new Blob([bytes], { type: contentType });
                dataUrl = URL.createObjectURL(blob);
                previousDataUrl = dataUrl;
            } else if (contentType === "application/json") {
                const str = new TextDecoder().decode(bytes);
                try {
                    jsonContent = JSON.parse(str);
                    textContent = JSON.stringify(jsonContent, null, 2);
                } catch {
                    textContent = str;
                }
            } else if (contentType.startsWith("text/") || contentType === "application/json") {
                textContent = new TextDecoder().decode(bytes);
            } else {
                hasError = false;
            }
            decoded = true;
        } catch (err) {
            hasError = true;
            errorMsg = "Failed to decode content: " + String(err);
        }
    }

    function renderMarkdown(md) {
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
        html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, url) => {
            const safeUrl = sanitizeMarkdownUrl(url);
            return safeUrl ? `<a href="${escapeAttribute(safeUrl)}" target="_blank" rel="noopener noreferrer">${label}</a>` : label;
        });
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

    function sanitizeMarkdownUrl(url) {
        const trimmed = String(url || "").trim();
        if (/^(https?:|ipfs:|ipns:)/i.test(trimmed)) return trimmed;
        return "";
    }

    function escapeAttribute(value) {
        return String(value)
            .replace(/&/g, "&amp;")
            .replace(/"/g, "&quot;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;");
    }

    function formatSize(bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
        return (bytes / (1024 * 1024)).toFixed(1) + " MB";
    }

    async function saveFile() {
        statusMsg = "";
        statusType = "";
        if (!contentBase64) return;

        const extension = contentType.includes("/") ? contentType.split("/")[1].split(";")[0] : "bin";
        const defaultName = `content.${extension || "bin"}`;

        try {
            if (cid) {
                try {
                    const result = await core.invoke("dialog_content_library_save_cached", {
                        cid,
                        defaultPath: defaultName,
                        title: "Save Content",
                        filters: [],
                    });
                    if (result) {
                        statusMsg = "Saved";
                        statusType = "ok";
                        return;
                    }
                } catch (err) {
                    if (String(err).includes("No file selected")) return;
                }
            }

            const result = await core.invoke("dialog_content_library_write_to_path", {
                contentBase64,
                defaultPath: defaultName,
                title: "Save Content",
                filters: [],
            });
            if (result) {
                statusMsg = "Saved";
                statusType = "ok";
            }
        } catch (err) {
            statusMsg = "Save failed: " + String(err);
            statusType = "error";
        }
    }

    async function openExternal() {
        statusMsg = "";
        statusType = "";
        if (cid) {
            try {
                await core.invoke("content_library_open_cached_file", { cid });
                statusMsg = "Opened";
                statusType = "ok";
            } catch (err) {
                statusMsg = "Open failed: " + String(err);
                statusType = "error";
            }
        } else {
            statusMsg = "Open external is available for cached CID content.";
            statusType = "error";
        }
    }

    onDestroy(() => {
        if (previousDataUrl) URL.revokeObjectURL(previousDataUrl);
    });
</script>

<div class="content-renderer">
    {#if statusMsg}
        <div class="status-bar" class:status-ok={statusType === "ok"} class:status-error={statusType === "error"}>
            {statusMsg}
        </div>
    {/if}

    {#if hasError}
        <div class="renderer-error">
            <span class="error-icon">x</span>
            <span>{errorMsg}</span>
        </div>
    {:else if contentType.startsWith("image/") && dataUrl}
        <div class="render-image">
            <img src={dataUrl} alt="Content" on:error={() => { hasError = true; errorMsg = "Image failed to load"; }} />
            <div class="image-controls">
                <button class="renderer-btn" on:click={openExternal} disabled={!cid}>OPEN EXTERNAL</button>
                <button class="renderer-btn" on:click={saveFile}>SAVE</button>
            </div>
        </div>
    {:else if contentType === "text/markdown" && textContent}
        <div class="render-markdown">
            {@html renderMarkdown(textContent)}
        </div>
    {:else if contentType === "application/json" && jsonContent}
        <div class="render-json">
            <pre class="json-pre mono">{textContent}</pre>
            <div class="json-controls">
                <button class="renderer-btn" on:click={() => {
                    if (textContent) {
                        const compact = JSON.stringify(jsonContent);
                        textContent = JSON.stringify(jsonContent, null, 2);
                    }
                }}>PRETTY</button>
                <button class="renderer-btn" on:click={saveFile}>SAVE</button>
            </div>
        </div>
    {:else if contentType === "application/pdf"}
        <div class="render-binary">
            <div class="binary-icon">PDF</div>
            <div class="binary-meta">
                <div>PDF Document</div>
                <div class="binary-size">{formatSize(sizeBytes)}</div>
            </div>
            <div class="binary-controls">
                <button class="renderer-btn" on:click={saveFile}>SAVE FILE</button>
                <button class="renderer-btn" on:click={openExternal} disabled={!cid}>
                    OPEN EXTERNAL
                </button>
            </div>
            <div class="binary-note">
                PDF files are saved locally. Open with your system viewer.
            </div>
        </div>
    {:else if textContent}
        <div class="render-text">
            <pre class="text-pre mono">{textContent}</pre>
        </div>
    {:else if decoded && !textContent && !contentType.startsWith("image/")}
        <div class="render-binary">
            <div class="binary-icon">BIN</div>
            <div class="binary-meta">
                <div>Unknown / Binary Content</div>
                <div class="binary-size">{formatSize(sizeBytes)}</div>
                <div class="binary-type">Type: {contentType}</div>
            </div>
            <div class="binary-controls">
                <button class="renderer-btn" on:click={saveFile}>SAVE FILE</button>
                {#if cid}
                    <button class="renderer-btn" on:click={openExternal}>OPEN EXTERNAL</button>
                {/if}
            </div>
            <div class="binary-note">
                This content type cannot be previewed. Save and open with a compatible application.
            </div>
        </div>
    {:else if !contentBase64}
        <div class="render-empty">No content loaded</div>
    {:else}
        <div class="render-loading">Decoding content...</div>
    {/if}
</div>

<style>
    .content-renderer {
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 6px;
        padding: 0.6rem;
    }
    .status-bar {
        padding: 0.3rem 0.6rem;
        font-size: 0.6rem;
        border-radius: 4px;
        margin-bottom: 0.4rem;
    }
    .status-ok {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
    }
    .status-error {
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.2);
        color: #ff6666;
    }
    .renderer-error {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.5rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.2);
        border-radius: 4px;
        color: #ff6666;
        font-size: 0.7rem;
    }
    .error-icon {
        font-weight: bold;
        flex-shrink: 0;
        width: 18px;
        height: 18px;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(255, 68, 68, 0.15);
        border-radius: 50%;
        font-size: 0.6rem;
    }
    .render-image {
        text-align: center;
    }
    .render-image img {
        max-width: 100%;
        max-height: min(400px, 50vh);
        border-radius: 6px;
        border: 1px solid rgba(0, 255, 65, 0.1);
    }
    .image-controls {
        display: flex;
        gap: 0.4rem;
        margin-top: 0.5rem;
        justify-content: center;
    }
    .render-markdown {
        padding: 0.3rem 0.5rem;
        color: #ccc;
        font-size: 0.78rem;
        line-height: 1.55;
        max-height: min(500px, 55vh);
        overflow-y: auto;
    }
    .render-markdown :global(h1) { font-size: 1.1rem; color: var(--color-primary); margin: 0.4rem 0; }
    .render-markdown :global(h2) { font-size: 0.95rem; color: var(--color-primary); margin: 0.35rem 0; }
    .render-markdown :global(h3) { font-size: 0.8rem; color: var(--color-primary); margin: 0.3rem 0; }
    .render-markdown :global(strong) { color: #fff; }
    .render-markdown :global(em) { color: #aaa; }
    .render-markdown :global(code) {
        background: rgba(0, 255, 65, 0.1);
        padding: 2px 5px;
        border-radius: 3px;
        font-size: 0.7rem;
        color: var(--color-primary);
    }
    .render-markdown :global(a) { color: var(--color-primary); }
    .render-markdown :global(li) { margin: 0.15rem 0; padding-left: 0.4rem; }
    .render-json {
        max-height: min(500px, 55vh);
        overflow-y: auto;
    }
    .json-pre {
        padding: 0.5rem;
        font-size: 0.72rem;
        color: #afa;
        background: #000;
        border: 1px solid #222;
        border-radius: 4px;
        overflow-x: auto;
        margin: 0;
        line-height: 1.4;
    }
    .json-controls {
        display: flex;
        gap: 0.4rem;
        margin-top: 0.4rem;
    }
    .text-pre {
        padding: 0.5rem;
        font-size: 0.72rem;
        color: #ccc;
        background: #000;
        border: 1px solid #222;
        border-radius: 4px;
        overflow-x: auto;
        margin: 0;
        max-height: min(500px, 55vh);
        overflow-y: auto;
        white-space: pre-wrap;
        word-break: break-word;
        line-height: 1.4;
    }
    .render-binary {
        text-align: center;
        padding: 1rem;
    }
    .binary-icon {
        font-size: 1.2rem;
        color: #555;
        font-weight: bold;
        letter-spacing: 1px;
        margin-bottom: 0.5rem;
    }
    .binary-meta {
        font-size: 0.7rem;
        color: #aaa;
        margin-bottom: 0.6rem;
    }
    .binary-size {
        font-size: 0.6rem;
        color: #666;
    }
    .binary-type {
        font-size: 0.55rem;
        color: #555;
        margin-top: 0.2rem;
    }
    .binary-controls {
        display: flex;
        gap: 0.4rem;
        justify-content: center;
        margin-bottom: 0.4rem;
    }
    .binary-note {
        font-size: 0.55rem;
        color: #555;
        max-width: 300px;
        margin: 0 auto;
    }
    .renderer-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        padding: 0.25rem 0.6rem;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
        border-radius: 3px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .renderer-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
    }
    .renderer-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }
    .render-empty {
        text-align: center;
        color: #555;
        font-size: 0.7rem;
        padding: 1rem;
    }
    .render-loading {
        text-align: center;
        color: #555;
        font-size: 0.7rem;
        padding: 1rem;
    }
</style>
