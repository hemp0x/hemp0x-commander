<script>
    import { createEventDispatcher } from "svelte";
    import { onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import ContentRenderer from "./ContentRenderer.svelte";

    const dispatch = createEventDispatcher();

    // CID to look up
    let cidInput = "";
    let cid = "";
    let packageIdToLoad = null;

    // Gateway consent
    let consentGiven = false;
    let gatewayIndex = 0;

    // Fetch state
    let fetchState = "idle"; // idle | consenting | fetching | fetched | cached | error
    let fetchResult = null;
    let fetchError = "";
    let cacheExists = false;
    let cacheStatus = null;
    let gatewayOptions = [];
    let pendingGatewayAction = "fetch";

    // For imported CID packages
    export let loadPackageId = null;
    export let loadCid = null;
    export let initialConsent = false;

    $: if (loadPackageId) {
        packageIdToLoad = loadPackageId;
    }
    $: if (loadCid) {
        cid = loadCid;
        cidInput = loadCid;
        if (initialConsent) {
            consentGiven = true;
        }
    }

    async function checkCache(c) {
        if (!c) return;
        try {
            cacheExists = await core.invoke("content_library_has_cache", { cid: c });
        } catch {
            cacheExists = false;
        }
    }

    $: if (cid) {
        checkCache(cid);
    }

    async function loadCacheStatus() {
        try {
            cacheStatus = await core.invoke("content_library_cache_status");
        } catch {
            cacheStatus = null;
        }
    }

    function askConsent(action = "fetch") {
        if (!cid.trim()) {
            fetchError = "Enter a CID to fetch.";
            return;
        }
        pendingGatewayAction = action;
        fetchError = "";
        fetchState = "consenting";
    }

    function grantConsent() {
        consentGiven = true;
        fetchState = "idle";
        if (pendingGatewayAction === "refresh") {
            doRefresh();
        } else {
            doFetch();
        }
    }

    function denyConsent() {
        consentGiven = false;
        fetchState = "idle";
    }

    function loadCached() {
        fetchError = "";
        fetchState = "fetching";
        (async () => {
            try {
                fetchResult = await core.invoke("content_library_get_cached", { cid: cid.trim() });
                fetchState = "cached";
            } catch (err) {
                fetchState = "error";
                fetchError = String(err);
            }
        })();
    }

    function doFetch() {
        if (!consentGiven && !cacheExists) {
            askConsent();
            return;
        }

        if (cacheExists) {
            loadCached();
            return;
        }

        fetchError = "";
        fetchState = "fetching";
        (async () => {
            try {
                fetchResult = await core.invoke("content_library_fetch_cid", {
                    cid: cid.trim(),
                    gatewayIndex: gatewayIndex || null,
                });
                fetchState = "fetched";
                cacheExists = true;
                await loadCacheStatus();
            } catch (err) {
                fetchState = "error";
                fetchError = String(err);
            }
        })();
    }

    function doRefresh() {
        fetchError = "";
        fetchState = "fetching";
        (async () => {
            try {
                fetchResult = await core.invoke("content_library_refresh_cached", {
                    cid: cid.trim(),
                    gatewayIndex: gatewayIndex || null,
                });
                fetchState = "fetched";
                cacheExists = true;
                await loadCacheStatus();
            } catch (err) {
                fetchState = "error";
                fetchError = String(err);
            }
        })();
    }

    function goToCid() {
        cid = cidInput.trim();
        consentGiven = false;
        fetchState = "idle";
        fetchResult = null;
        fetchError = "";
    }

    function handleKeydown(e) {
        if (e.key === "Enter") {
            goToCid();
        }
    }

    onMount(() => {
        loadCacheStatus();
        core.invoke("content_library_default_gateways")
            .then((gateways) => {
                gatewayOptions = Array.isArray(gateways) && gateways.length ? gateways : [];
            })
            .catch(() => {
                gatewayOptions = [];
            });
    });
</script>

<div class="cid-viewer" in:fade={{ duration: 150 }}>
    <div class="viewer-header">
        <span class="viewer-title">CID VIEWER</span>
    </div>

    <div class="cid-form">
        <div class="form-group">
            <label class="form-label mono" for="cid-viewer-input">CID / IPFS HASH</label>
            <div class="input-row">
                <input
                    id="cid-viewer-input"
                    class="form-input mono"
                    type="text"
                    bind:value={cidInput}
                    on:keydown={handleKeydown}
                    placeholder="Qm... or bafy..."
                />
                <button class="cyber-btn" on:click={goToCid}>VIEW</button>
            </div>
        </div>
    </div>

    {#if fetchError && fetchState === "error"}
        <div class="error-bar">{fetchError}</div>
    {/if}

    {#if fetchState === "consenting"}
        <div class="consent-panel" in:fade={{ duration: 150 }}>
            <div class="consent-header">Gateway Fetch Notice</div>
            <div class="consent-body">
                <p>Fetching content from a public IPFS gateway will expose the CID and your IP address to the gateway operator.</p>
                <p>Content from public gateways is not private and gateway availability may change over time.</p>
                <p class="consent-cid">CID: <span class="mono">{cid}</span></p>
            </div>
            <div class="consent-gateways">
                <div class="form-group">
                    <label class="form-label mono" for="cid-viewer-gateway">GATEWAY</label>
                    <select id="cid-viewer-gateway" class="form-input mono" bind:value={gatewayIndex}>
                        {#if gatewayOptions.length}
                            {#each gatewayOptions as gateway, idx}
                                <option value={idx}>{gateway.replace(/^https?:\/\//, "").replace(/\/ipfs\/?$/, "")}{idx === 0 ? " (default)" : ""}</option>
                            {/each}
                        {:else}
                            <option value={0}>dweb.link (default)</option>
                        {/if}
                    </select>
                </div>
            </div>
            <div class="consent-actions">
                <button class="cyber-btn" on:click={grantConsent}>FETCH CONTENT</button>
                <button class="cyber-btn ghost" on:click={denyConsent}>CANCEL</button>
            </div>
        </div>
    {/if}

    {#if cid && fetchState === "idle" && !fetchResult}
        <div class="cid-ready-panel" in:fade={{ duration: 150 }}>
            <div class="ready-meta">
                <span class="ready-label">CID</span>
                <span class="ready-value mono">{cid}</span>
            </div>
            <div class="ready-copy">
                Public gateway access requires confirmation before any network request.
            </div>
            <div class="ready-actions">
                {#if cacheExists}
                    <button class="cyber-btn" on:click={loadCached}>OPEN CACHED</button>
                    <button class="cyber-btn ghost" on:click={() => askConsent("refresh")}>REFRESH FROM GATEWAY</button>
                {:else}
                    <button class="cyber-btn" on:click={() => askConsent("fetch")}>FETCH FROM GATEWAY</button>
                {/if}
            </div>
        </div>
    {/if}

    {#if fetchState === "fetching"}
        <div class="fetching-status">
            <div class="fetching-icon">⌂</div>
            <div class="fetching-text">Fetching from gateway...</div>
        </div>
    {/if}

    {#if (fetchState === "fetched" || fetchState === "cached") && fetchResult}
        <div class="fetched-content" in:fade={{ duration: 150 }}>
            <div class="content-meta-bar">
                <div class="meta-item">
                    <span class="meta-label">CID</span>
                    <span class="meta-value mono">{fetchResult.cid.length > 24 ? fetchResult.cid.slice(0, 12) + "..." + fetchResult.cid.slice(-12) : fetchResult.cid}</span>
                    <button class="copy-btn" on:click={async () => { try { await navigator.clipboard.writeText(fetchResult.cid); } catch {} }}>
                        COPY
                    </button>
                </div>
                <div class="meta-item">
                    <span class="meta-label">TYPE</span>
                    <span class="meta-value">{fetchResult.content_type}</span>
                </div>
                <div class="meta-item">
                    <span class="meta-label">SIZE</span>
                    <span class="meta-value">{fetchResult.size_bytes < 1024 ? fetchResult.size_bytes + " B" : (fetchResult.size_bytes / 1024).toFixed(1) + " KB"}</span>
                </div>
                <div class="meta-item">
                    <span class="meta-label">GATEWAY</span>
                    <span class="meta-value mono" style="font-size:0.55rem;">{fetchResult.gateway_used}</span>
                </div>
                <div class="meta-item">
                    <span class="meta-label">FETCHED</span>
                    <span class="meta-value">{(fetchResult.fetched_at || "").slice(0, 19)}</span>
                </div>
                <div class="meta-item">
                    <span class="meta-label">STATUS</span>
                    <span class="meta-value" style="color: {fetchState === 'cached' ? '#cca' : 'var(--color-primary)'};">
                        {fetchState === "cached" ? "CACHED" : "FETCHED"}
                    </span>
                </div>
            </div>

            <ContentRenderer
                contentBase64={fetchResult.content_base64}
                contentType={fetchResult.content_type}
                sizeBytes={fetchResult.size_bytes}
                cid={fetchResult.cid}
            />

            <div class="content-actions">
                {#if fetchState === "cached"}
                    <button class="cyber-btn ghost" on:click={doRefresh}>REFRESH</button>
                {:else}
                    <button class="cyber-btn ghost" on:click={doRefresh}>RE-FETCH</button>
                {/if}
            </div>
        </div>
    {/if}

    {#if !cid && !fetchResult && fetchState === "idle"}
        <div class="empty-hint">
            <div class="hint-icon">◉</div>
            <div class="hint-text">
                Enter an IPFS CID and click VIEW to fetch and preview content from public gateways.
            </div>
        </div>
    {/if}

    {#if cacheStatus}
        <div class="cache-status-bar">
            <span class="cache-label">CACHE</span>
            <span class="cache-stat">{cacheStatus.entry_count} entries</span>
            <span class="cache-stat">{cacheStatus.total_size_bytes < 1024 ? cacheStatus.total_size_bytes + " B" : cacheStatus.total_size_bytes < 1048576 ? (cacheStatus.total_size_bytes / 1024).toFixed(0) + " KB" : (cacheStatus.total_size_bytes / 1048576).toFixed(1) + " MB"}</span>
            <div class="cache-actions">
                <button class="cache-action-btn" on:click={async () => {
                    try { await core.invoke("content_library_clear_cache"); cacheExists = false; fetchResult = null; fetchState = "idle"; await loadCacheStatus(); }
                    catch (e) { fetchError = String(e); fetchState = "error"; }
                }}>CLEAR</button>
                <button class="cache-action-btn" on:click={async () => {
                    try { await core.invoke("content_library_open_cache_folder"); }
                    catch (e) { fetchError = String(e); fetchState = "error"; }
                }}>FOLDER</button>
                <button class="cache-action-btn" on:click={loadCacheStatus}>REFRESH</button>
            </div>
        </div>
    {/if}
</div>

<style>
    .cid-viewer {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 8px;
        padding: 1rem;
        max-width: 800px;
    }
    .viewer-header {
        margin-bottom: 0.8rem;
    }
    .viewer-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        font-weight: 700;
    }
    .cid-form {
        margin-bottom: 0.8rem;
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
    .input-row {
        display: flex;
        gap: 0.5rem;
    }
    .form-input {
        flex: 1;
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
    select.form-input {
        cursor: pointer;
    }
    .error-bar {
        padding: 0.4rem 0.8rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.25);
        color: #ff6666;
        font-size: 0.7rem;
        border-radius: 4px;
        margin-bottom: 0.8rem;
    }
    .cid-ready-panel {
        padding: 0.8rem;
        margin-bottom: 0.8rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 6px;
    }
    .ready-meta {
        display: flex;
        align-items: baseline;
        gap: 0.5rem;
        min-width: 0;
        margin-bottom: 0.4rem;
    }
    .ready-label {
        color: #555;
        font-size: 0.6rem;
        letter-spacing: 1px;
        flex-shrink: 0;
    }
    .ready-value {
        color: #aaa;
        font-size: 0.68rem;
        overflow-wrap: anywhere;
    }
    .ready-copy {
        color: #777;
        font-size: 0.65rem;
        margin-bottom: 0.6rem;
    }
    .ready-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
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
        white-space: nowrap;
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

    /* Consent panel */
    .consent-panel {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.2);
        border-radius: 6px;
        padding: 0.8rem;
        margin-bottom: 0.8rem;
    }
    .consent-header {
        font-size: 0.7rem;
        font-weight: 600;
        color: var(--color-primary);
        letter-spacing: 1px;
        margin-bottom: 0.5rem;
    }
    .consent-body {
        font-size: 0.65rem;
        color: #aaa;
        line-height: 1.5;
        margin-bottom: 0.6rem;
    }
    .consent-body p {
        margin: 0.25rem 0;
    }
    .consent-cid {
        margin-top: 0.3rem;
        font-size: 0.55rem;
        color: #888;
    }
    .consent-gateways {
        margin-bottom: 0.5rem;
    }
    .consent-actions {
        display: flex;
        gap: 0.5rem;
    }

    /* Fetching status */
    .fetching-status {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 6px;
        color: #aaa;
    }
    .fetching-icon {
        font-size: 1.2rem;
        color: var(--color-primary);
        animation: pulse 1.5s ease-in-out infinite;
    }
    .fetching-text {
        font-size: 0.7rem;
    }

    @keyframes pulse {
        0%, 100% { opacity: 0.4; }
        50% { opacity: 1; }
    }

    /* Fetched content */
    .fetched-content {
        margin-top: 0.6rem;
    }
    .content-meta-bar {
        display: flex;
        flex-wrap: wrap;
        gap: 0.4rem;
        margin-bottom: 0.6rem;
    }
    .meta-item {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.6rem;
    }
    .meta-label {
        color: #555;
        letter-spacing: 0.5px;
    }
    .meta-value {
        color: #aaa;
    }
    .copy-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
        font-size: 0.5rem;
        padding: 1px 5px;
        border-radius: 3px;
        cursor: pointer;
    }
    .copy-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .content-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.6rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }

    /* Empty hint */
    .empty-hint {
        text-align: center;
        padding: 2rem 1rem;
    }
    .hint-icon {
        font-size: 1.5rem;
        color: #444;
        margin-bottom: 0.5rem;
    }
    .hint-text {
        font-size: 0.7rem;
        color: #666;
        max-width: 400px;
        margin: 0 auto 0.4rem;
        line-height: 1.4;
    }
    /* Cache status bar */
    .cache-status-bar {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 0.4rem 0.6rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px;
        margin-top: 0.8rem;
        font-size: 0.6rem;
    }
    .cache-label {
        color: #555;
        letter-spacing: 1px;
    }
    .cache-stat {
        color: #888;
    }
</style>
