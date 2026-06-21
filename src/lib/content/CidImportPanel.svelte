<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";

    const dispatch = createEventDispatcher();

    let cid = "";
    let name = "";
    let description = "";
    let tagsStr = "";
    let importing = false;
    let errorMsg = "";
    let successMsg = "";

    function resetForm() {
        cid = "";
        name = "";
        description = "";
        tagsStr = "";
        errorMsg = "";
        successMsg = "";
    }

    async function importCid() {
        importing = true;
        errorMsg = "";
        successMsg = "";

        const trimmedCid = cid.trim();
        if (!trimmedCid) {
            errorMsg = "CID is required.";
            importing = false;
            return;
        }

        try {
            await core.invoke("content_library_import_cid", {
                input: {
                    cid: trimmedCid,
                    name: name.trim() || null,
                    description: description.trim() || null,
                    tags: tagsStr
                        ? tagsStr.split(",").map((t) => t.trim().toLowerCase()).filter((t) => t.length > 0)
                        : null,
                },
            });
            successMsg = "CID imported successfully.";
            resetForm();
            dispatch("imported");
        } catch (err) {
            errorMsg = String(err);
        }
        importing = false;
    }
</script>

<div class="cid-import" in:fade={{ duration: 150 }}>
    <h3 class="panel-title">CID VIEWER / IMPORT</h3>

    {#if errorMsg}
        <div class="error-bar">{errorMsg}</div>
    {/if}
    {#if successMsg}
        <div class="success-bar">{successMsg}</div>
    {/if}

    <div class="form-group">
        <label class="form-label" for="cid-input">CID / HASH</label>
        <input
            id="cid-input"
            class="form-input mono"
            type="text"
            bind:value={cid}
            placeholder="Qm... or bafy..."
        />
    </div>

    <div class="form-group">
        <label class="form-label" for="cid-name">NAME (optional)</label>
        <input
            id="cid-name"
            class="form-input mono"
            type="text"
            bind:value={name}
            placeholder="Auto-generated from CID if empty"
        />
    </div>

    <div class="form-group">
        <label class="form-label" for="cid-desc">DESCRIPTION (optional)</label>
        <input
            id="cid-desc"
            class="form-input mono"
            type="text"
            bind:value={description}
            placeholder="Short description"
        />
    </div>

    <div class="form-group">
        <label class="form-label" for="cid-tags">TAGS (comma-separated)</label>
        <input
            id="cid-tags"
            class="form-input mono"
            type="text"
            bind:value={tagsStr}
            placeholder="nft, metadata"
        />
    </div>

    <div class="form-actions">
        <button class="cyber-btn" on:click={importCid} disabled={importing}>
            {importing ? "IMPORTING..." : "IMPORT CID"}
        </button>
        <button class="cyber-btn ghost" on:click={resetForm} disabled={importing}>
            CLEAR
        </button>
    </div>

    <div class="privacy-note">
        No gateway requests are made. This creates a local record only.
    </div>
</div>

<style>
    .cid-import {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1rem 1.2rem;
        max-width: 600px;
    }
    .panel-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0 0 1rem 0;
    }
    .error-bar {
        padding: 0.4rem 0.8rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.25);
        color: #ff6666;
        font-size: 0.7rem;
        border-radius: 4px;
        margin-bottom: 0.8rem;
        font-family: var(--font-mono);
    }
    .success-bar {
        padding: 0.4rem 0.8rem;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
        font-size: 0.7rem;
        border-radius: 4px;
        margin-bottom: 0.8rem;
        font-family: var(--font-mono);
    }
    .form-group {
        margin-bottom: 0.8rem;
    }
    .form-label {
        display: block;
        font-size: 0.6rem;
        letter-spacing: 1px;
        color: #555;
        margin-bottom: 0.3rem;
    }
    .form-input {
        width: 100%;
        background: #000;
        border: 1px solid #333;
        color: #0f0;
        padding: 0.45rem 0.65rem;
        border-radius: 4px;
        font-size: 0.75rem;
        outline: none;
        box-sizing: border-box;
    }
    .form-input:focus {
        border-color: var(--color-primary);
    }
    .form-input::placeholder {
        color: #444;
    }
    .form-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 1rem;
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
    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.22);
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
    .privacy-note {
        margin-top: 0.8rem;
        padding: 0.5rem 0.75rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px;
        font-size: 0.6rem;
        color: #666;
    }
</style>
