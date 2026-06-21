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

    // Save to library from fetched CID
    let creatingPackage = false;
    let createPackageMsg = "";
    let saveFolders = [];
    let selectedSaveFolder = "";

    // Gateway consent
    let consentGiven = false;
    let gatewayIndex = 0;

    // Fetch state
    let fetchState = "idle"; // idle | consenting | fetching | fetched | cached | error
    let fetchResult = null;
    let fetchError = "";
    let cacheExists = false;
    let gatewayOptions = [];
    let pendingGatewayAction = "fetch";

    // Directory package state
    let directoryMode = false;
    let directoryLoading = false;
    let directoryError = "";
    let packageMeta = null;
    let nftMeta = null;
    let mainContentResult = null;
    let subpathFiles = [];

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
                analyzeFetchResult();
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
                analyzeFetchResult();
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
                analyzeFetchResult();
            } catch (err) {
                fetchState = "error";
                fetchError = String(err);
            }
        })();
    }

    async function saveToLibrary() {
        if (!cid.trim()) return;
        creatingPackage = true;
        createPackageMsg = "";
        try {
            let input = {
                name: "",
                description: "",
                tags: [],
                body: undefined,
                files: undefined,
                folder: selectedSaveFolder || undefined,
            };

            if (packageMeta) {
                input.name = packageMeta.name || `CID: ${cid.slice(0, 24)}`;
                input.description = packageMeta.description || "";
                input.tags = packageMeta.tags || [];
                if (mainContentResult && mainContentResult.content_base64) {
                    const ct = (mainContentResult.content_type || "").toLowerCase();
                    if (ct.includes("text/") || ct.includes("markdown")) {
                        const bytes = Uint8Array.from(atob(mainContentResult.content_base64), (c) => c.charCodeAt(0));
                        input.body = new TextDecoder().decode(bytes);
                    } else {
                        input.files = [{
                            path: packageMeta.main_content || "content",
                            mime: mainContentResult.content_type,
                            content_base64: mainContentResult.content_base64,
                        }];
                    }
                }
            } else if (fetchResult && fetchResult.content_base64) {
                input.name = `CID: ${cid.slice(0, cid.length > 24 ? 24 : cid.length)}`;
                const ct = (fetchResult.content_type || "").toLowerCase();
                if (ct.includes("text/") || ct.includes("markdown")) {
                    const bytes = Uint8Array.from(atob(fetchResult.content_base64), (c) => c.charCodeAt(0));
                    input.body = new TextDecoder().decode(bytes);
                } else {
                    const ext = ct.includes("/") ? ct.split("/")[1].split(";")[0] : "bin";
                    input.files = [{
                        path: `content.${ext || "bin"}`,
                        mime: fetchResult.content_type,
                        content_base64: fetchResult.content_base64,
                    }];
                }
            } else {
                createPackageMsg = "No content available to save.";
                creatingPackage = false;
                return;
            }

            const result = await core.invoke("content_library_create", { input });
            createPackageMsg = `Saved to library: ${result.name}`;
            dispatch("created", result);
        } catch (err) {
            createPackageMsg = "Failed: " + String(err);
        }
        creatingPackage = false;
    }

    function goToCid() {
        cid = cidInput.trim();
        consentGiven = false;
        fetchState = "idle";
        fetchResult = null;
        fetchError = "";
        createPackageMsg = "";
        resetDirectoryState();
    }

    function handleKeydown(e) {
        if (e.key === "Enter") {
            goToCid();
        }
    }

    onMount(() => {
        core.invoke("content_library_default_gateways")
            .then((gateways) => {
                gatewayOptions = Array.isArray(gateways) && gateways.length ? gateways : [];
            })
            .catch(() => {
                gatewayOptions = [];
            });
        core.invoke("content_library_list_folders")
            .then((folders) => {
                saveFolders = Array.isArray(folders) ? folders : [];
            })
            .catch(() => {
                saveFolders = [];
            });
    });

    function resetDirectoryState() {
        directoryMode = false;
        directoryLoading = false;
        directoryError = "";
        packageMeta = null;
        nftMeta = null;
        mainContentResult = null;
        subpathFiles = [];
    }

    function isGatewayDirectoryListing(result) {
        if (!result || !result.content_type || !result.content_base64) return false;
        if (!result.content_type.toLowerCase().includes("text/html")) return false;
        try {
            const html = atob(result.content_base64);
            const lower = html.toLowerCase();
            return lower.includes("index of") && (lower.includes("/ipfs/") || lower.includes("directory listing"));
        } catch {
            return false;
        }
    }

    function getGatewayBase() {
        if (fetchResult && fetchResult.gateway_used) {
            return fetchResult.gateway_used.replace(/\/$/, "");
        }
        const gw = gatewayOptions[gatewayIndex] || "https://dweb.link/ipfs/";
        return gw.replace(/\/$/, "");
    }

    function getGatewayUrl(subpath) {
        const encodedPath = String(subpath || "")
            .split("/")
            .map((part) => encodeURIComponent(part))
            .join("/");
        return `${getGatewayBase()}/${cid}/${encodedPath}`;
    }

    async function copyText(text) {
        try {
            await navigator.clipboard.writeText(text);
        } catch {}
    }

    async function probeDirectoryPackage() {
        directoryLoading = true;
        directoryError = "";
        packageMeta = null;
        nftMeta = null;
        mainContentResult = null;
        subpathFiles = [];

        const gwIndex = gatewayIndex || null;

        // Try metadata.json first
        try {
            const metaResult = await core.invoke("content_library_fetch_cid_path", {
                cid: cid.trim(),
                path: "metadata.json",
                gatewayIndex: gwIndex,
            });
            const metaText = atob(metaResult.content_base64);
            const metaJson = JSON.parse(metaText);

            // Commander package metadata
            if (metaJson.package_id && Array.isArray(metaJson.files)) {
                packageMeta = metaJson;
                subpathFiles = metaJson.files || [];
                if (metaJson.main_content) {
                    try {
                        mainContentResult = await core.invoke("content_library_fetch_cid_path", {
                            cid: cid.trim(),
                            path: metaJson.main_content,
                            gatewayIndex: gwIndex,
                        });
                    } catch (_) {}
                }
                directoryLoading = false;
                return;
            }

            // NFT / IPFS metadata
            if (metaJson.name && (metaJson.image || metaJson.attributes)) {
                nftMeta = metaJson;
                directoryLoading = false;
                return;
            }
        } catch (_) {
            // metadata.json not found or not parseable
        }

        // Try content.md
        try {
            mainContentResult = await core.invoke("content_library_fetch_cid_path", {
                cid: cid.trim(),
                path: "content.md",
                gatewayIndex: gwIndex,
            });
            directoryLoading = false;
            return;
        } catch (_) {}

        // Try index.html
        try {
            mainContentResult = await core.invoke("content_library_fetch_cid_path", {
                cid: cid.trim(),
                path: "index.html",
                gatewayIndex: gwIndex,
            });
            directoryLoading = false;
            return;
        } catch (_) {}

        directoryError = "Directory listing detected, but no known entry files (metadata.json, content.md, index.html) were found.";
        directoryLoading = false;
    }

    function analyzeFetchResult() {
        resetDirectoryState();
        if (!fetchResult) return;
        if (isGatewayDirectoryListing(fetchResult)) {
            directoryMode = true;
            probeDirectoryPackage();
        }
    }

    function resolveNftMediaUrl(url) {
        if (!url) return "";
        const trimmed = String(url).trim();
        if (/^ipfs:\/\//i.test(trimmed)) {
            return `${getGatewayBase()}/${trimmed.replace(/^ipfs:\/\//i, "").replace(/^\/+/, "")}`;
        }
        if (/^(https?:|ipns:)/i.test(trimmed)) return trimmed;
        // Relative path under CID
        return getGatewayUrl(trimmed);
    }
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

            {#if directoryMode}
                <div class="directory-panel" in:fade={{ duration: 150 }}>
                    <div class="dir-badge">Directory package detected</div>

                    {#if directoryLoading}
                        <div class="directory-loading">
                            <div class="fetching-icon">⌂</div>
                            <div class="fetching-text">Probing directory contents...</div>
                        </div>
                    {:else if directoryError}
                        <div class="directory-error">{directoryError}</div>
                    {:else if packageMeta}
                        <div class="package-header">
                            <div class="package-title">{packageMeta.name || "Unnamed Package"}</div>
                            {#if packageMeta.description}
                                <div class="package-description">{packageMeta.description}</div>
                            {/if}
                            {#if packageMeta.tags && packageMeta.tags.length}
                                <div class="package-tags">
                                    {#each packageMeta.tags as tag}
                                        <span class="package-tag">{tag}</span>
                                    {/each}
                                </div>
                            {/if}
                        </div>

                        {#if mainContentResult}
                            <div class="main-content-label">Main Content</div>
                            <ContentRenderer
                                contentBase64={mainContentResult.content_base64}
                                contentType={mainContentResult.content_type}
                                sizeBytes={mainContentResult.size_bytes}
                                cid={cid}
                            />
                        {/if}

                        {#if subpathFiles.length}
                            <div class="files-section">
                                <div class="files-label">Files</div>
                                <div class="files-list">
                                    {#each subpathFiles as f}
                                        <div class="file-row">
                                            <span class="file-name mono">{f.path}</span>
                                            <span class="file-size">{f.size_bytes < 1024 ? f.size_bytes + ' B' : (f.size_bytes / 1024).toFixed(1) + ' KB'}</span>
                                            <div class="file-actions">
                                                <button class="copy-btn small" on:click={() => copyText(getGatewayUrl(f.path))}>COPY URL</button>
                                                <a class="renderer-btn small" href={getGatewayUrl(f.path)} target="_blank" rel="noopener noreferrer">OPEN</a>
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            </div>
                        {/if}

                        <div class="entry-actions">
                            <button class="copy-btn small" on:click={() => copyText(getGatewayUrl("metadata.json"))}>COPY metadata.json</button>
                            <button class="copy-btn small" on:click={() => copyText(getGatewayUrl("content.md"))}>COPY content.md</button>
                            <a class="renderer-btn small" href={getGatewayUrl("metadata.json")} target="_blank" rel="noopener noreferrer">OPEN metadata.json</a>
                            <a class="renderer-btn small" href={getGatewayUrl("content.md")} target="_blank" rel="noopener noreferrer">OPEN content.md</a>
                        </div>
                    {:else if nftMeta}
                        <div class="nft-header">
                            <div class="package-title">{nftMeta.name || "Unnamed NFT"}</div>
                            {#if nftMeta.description}
                                <div class="package-description">{nftMeta.description}</div>
                            {/if}
                            {#if nftMeta.image}
                                <div class="nft-image-wrap">
                                    <img class="nft-image" src={resolveNftMediaUrl(nftMeta.image)} alt={nftMeta.name || ""} on:error={(e) => { e.target.style.display = 'none'; }} />
                                </div>
                            {/if}
                            {#if nftMeta.animation_url}
                                <div class="nft-media-link">
                                    <a class="renderer-btn small" href={resolveNftMediaUrl(nftMeta.animation_url)} target="_blank" rel="noopener noreferrer">Open Animation</a>
                                </div>
                            {/if}
                            {#if nftMeta.external_url}
                                <div class="nft-media-link">
                                    <a class="renderer-btn small" href={nftMeta.external_url} target="_blank" rel="noopener noreferrer">External Link</a>
                                </div>
                            {/if}
                            {#if nftMeta.attributes && nftMeta.attributes.length}
                                <div class="files-section">
                                    <div class="files-label">Attributes</div>
                                    <div class="files-list">
                                        {#each nftMeta.attributes as attr}
                                            <div class="file-row">
                                                <span class="file-name">{attr.trait_type || attr.traitType || "Trait"}</span>
                                                <span class="file-size">{attr.value}</span>
                                            </div>
                                        {/each}
                                    </div>
                                </div>
                            {/if}
                        </div>
                    {:else if mainContentResult}
                        <ContentRenderer
                            contentBase64={mainContentResult.content_base64}
                            contentType={mainContentResult.content_type}
                            sizeBytes={mainContentResult.size_bytes}
                            cid={cid}
                        />
                    {/if}
                </div>
            {:else}
                <ContentRenderer
                    contentBase64={fetchResult.content_base64}
                    contentType={fetchResult.content_type}
                    sizeBytes={fetchResult.size_bytes}
                    cid={fetchResult.cid}
                />
            {/if}

            <div class="content-actions">
                <button class="cyber-btn small" on:click={saveToLibrary} disabled={creatingPackage}>
                    {creatingPackage ? "SAVING..." : "+ SAVE TO LIBRARY"}
                </button>
                <select class="form-input mono folder-select" bind:value={selectedSaveFolder} title="Save folder">
                    <option value="">Unsorted</option>
                    {#each saveFolders as f}
                        <option value={f}>{f}</option>
                    {/each}
                </select>
                {#if fetchState === "cached"}
                    <button class="cyber-btn ghost small" on:click={doRefresh}>REFRESH</button>
                {:else}
                    <button class="cyber-btn ghost small" on:click={doRefresh}>RE-FETCH</button>
                {/if}
                <button class="copy-btn small" on:click={async () => { try { await navigator.clipboard.writeText(fetchResult.cid); } catch {} }}>COPY CID</button>
            </div>
            {#if createPackageMsg}
                <div class="create-msg">{createPackageMsg}</div>
            {/if}
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

</div>

<style>
    .cid-viewer {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 8px;
        padding: 1rem;
    }
    .viewer-header {
        padding-top: 0.25rem;
        margin-bottom: 0.8rem;
        width: 100%;
    }
    .viewer-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        font-weight: 700;
    }
    .cid-form {
        margin-bottom: 0.8rem;
        width: 100%;
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
        width: 100%;
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
    .cyber-btn.small {
        padding: 0.3rem 0.7rem;
        font-size: 0.6rem;
        letter-spacing: 0.5px;
    }

    /* Consent panel */
    .consent-panel {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.2);
        border-radius: 6px;
        padding: 0.8rem;
        margin-bottom: 0.8rem;
        width: 100%;
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
        width: 100%;
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
        padding: 1rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 8px;
    }
    .content-meta-bar {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
        margin-bottom: 0.75rem;
        justify-content: flex-start;
    }
    .meta-item {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        padding: 0.35rem 0.65rem;
        background: rgba(0, 0, 0, 0.35);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
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
        padding: 2px 6px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .copy-btn:hover {
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .copy-btn.small {
        font-size: 0.55rem;
        padding: 3px 8px;
    }
    .content-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.75rem;
        padding-top: 0.6rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        justify-content: flex-start;
        flex-wrap: wrap;
    }
    .create-msg {
        margin-top: 0.5rem;
        font-size: 0.65rem;
        color: var(--color-primary);
    }

    /* Empty hint */
    .empty-hint {
        text-align: center;
        padding: 3rem 1rem;
        width: 100%;
    }
    .hint-icon {
        font-size: 2rem;
        color: #333;
        margin-bottom: 0.75rem;
    }
    .hint-text {
        font-size: 0.75rem;
        color: #666;
        max-width: 400px;
        margin: 0 auto 0.5rem;
        line-height: 1.5;
    }

    /* Directory package panel */
    .directory-panel {
        margin-top: 0.4rem;
    }
    .dir-badge {
        display: inline-block;
        font-size: 0.6rem;
        letter-spacing: 1px;
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        padding: 0.25rem 0.6rem;
        border-radius: 4px;
        margin-bottom: 0.6rem;
    }
    .directory-loading {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 0.8rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 6px;
        color: #aaa;
        width: 100%;
        margin-bottom: 0.6rem;
    }
    .directory-error {
        padding: 0.5rem;
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.2);
        border-radius: 4px;
        color: #ff6666;
        font-size: 0.7rem;
        margin-bottom: 0.6rem;
    }
    .package-header {
        margin-bottom: 0.75rem;
        padding-bottom: 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .package-title {
        font-size: 1rem;
        font-weight: 600;
        color: var(--color-primary);
        margin-bottom: 0.3rem;
    }
    .package-description {
        font-size: 0.72rem;
        color: #aaa;
        line-height: 1.5;
        margin-bottom: 0.4rem;
    }
    .package-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 0.3rem;
    }
    .package-tag {
        font-size: 0.55rem;
        color: #888;
        background: rgba(0, 0, 0, 0.35);
        border: 1px solid rgba(255, 255, 255, 0.08);
        padding: 0.15rem 0.4rem;
        border-radius: 3px;
        letter-spacing: 0.5px;
    }
    .main-content-label {
        font-size: 0.6rem;
        color: #555;
        letter-spacing: 1px;
        margin-bottom: 0.4rem;
    }
    .files-section {
        margin-top: 0.75rem;
        margin-bottom: 0.5rem;
    }
    .files-label {
        font-size: 0.6rem;
        color: #555;
        letter-spacing: 1px;
        margin-bottom: 0.4rem;
    }
    .files-list {
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        overflow: hidden;
    }
    .file-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.4rem 0.6rem;
        font-size: 0.68rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        flex-wrap: wrap;
    }
    .file-row:last-child {
        border-bottom: none;
    }
    .file-name {
        color: #ccc;
        flex: 1;
        min-width: 0;
        overflow-wrap: anywhere;
    }
    .file-size {
        color: #666;
        font-size: 0.6rem;
        white-space: nowrap;
    }
    .file-actions {
        display: flex;
        gap: 0.3rem;
    }
    .entry-actions {
        display: flex;
        gap: 0.4rem;
        margin-top: 0.6rem;
        flex-wrap: wrap;
    }
    .nft-header {
        margin-bottom: 0.6rem;
    }
    .nft-image-wrap {
        margin: 0.5rem 0;
        text-align: center;
    }
    .nft-image {
        max-width: 100%;
        max-height: min(320px, 40vh);
        border-radius: 6px;
        border: 1px solid rgba(0, 255, 65, 0.1);
    }
    .nft-media-link {
        margin: 0.3rem 0;
    }
    a.renderer-btn {
        text-decoration: none;
        display: inline-flex;
        align-items: center;
    }
    .folder-select {
        padding: 0.3rem 0.5rem;
        font-size: 0.6rem;
        background-color: #000;
        border: 1px solid #333;
        color: #0f0;
        border-radius: 4px;
        cursor: pointer;
        min-width: 120px;
        appearance: none;
        -webkit-appearance: none;
    }
    select.form-input {
        background-color: #000;
        color: #0f0;
        appearance: none;
        -webkit-appearance: none;
    }

</style>
