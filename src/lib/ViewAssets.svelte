<script>
    import { onMount } from "svelte";
    import { fly, fade, scale } from "svelte/transition";
    import { flip } from "svelte/animate";
    import { core } from "@tauri-apps/api";
    import ModalConfirm from "./modals/ModalConfirm.svelte";
    import ModalAssetDetail from "./modals/ModalAssetDetail.svelte";
    import ModalIssueAsset from "./modals/ModalIssueAsset.svelte";
    import ModalIssueSub from "./modals/ModalIssueSub.svelte";
    import ModalIssueNFT from "./modals/ModalIssueNFT.svelte";
    import ModalTransfer from "./modals/ModalTransfer.svelte";
    import ModalReissue from "./modals/ModalReissue.svelte";
    import ModalBrowse from "./modals/ModalBrowse.svelte";
    import ModalAssetGovernance from "./modals/ModalAssetGovernance.svelte";
    import Tooltip from "./ui/Tooltip.svelte";
    import eyeOpen from "../assets/eye-open.png";
    import eyeClosed from "../assets/eye-closed.png";

    let myAssets = [];
    let tauriReady = false;
    let status = "";

    // Detail View
    let selectedDetail = null;

    // Transfer
    let selectedAsset = "";
    let transferTo = "";
    let transferAmt = "";

    // Create Asset Modal (Root Only)
    let createModalOpen = false;

    // Transfer & Reissue Modals (NEW)
    let transferModalOpen = false;
    let reissueModalOpen = false;

    // Sub-Asset & NFT Modals
    let subModalOpen = false;
    let nftModalOpen = false;
    let govModalOpen = false;
    let selectedGovAsset = null;

    // Browse Modal
    let browseModalOpen = false;
    let browsePattern = "";
    let browseResults = [];
    let browseLoading = false;
    let issueType = "root"; // "root" | "sub" | "nft"
    let issueParent = ""; // Parent asset name for sub-assets
    let issueIpfs = ""; // Optional IPFS hash
    let nftTag = ""; // Tag name for NFT/unique assets

    // Issue
    let issueName = "";
    let issueQty = "1";
    let issueUnits = 0;
    let issueReissue = true;

    // Reissue
    let reissueAsset = "";
    let reissueQty = "0";
    let reissueReissuable = true;

    // Confirm Modal
    let confirmOpen = false;
    let confirmPayload = null;
    let confirmType = "";

    // Persistent UI State
    let showHidden = false;
    let hiddenAssets = new Set();
    let assetOrder = [];
    let draggingItem = null;

    // Allow parent to pass initial state

    import { nodeStatus } from "../stores.js";
    $: isNodeOnline = $nodeStatus.online;

    let nodeOnline = false;

    onMount(async () => {
        tauriReady =
            typeof core?.isTauri === "function" ? core.isTauri() : false;
        if (tauriReady) {
            // Try to refresh assets, will fail gracefully if node is offline
            await refreshAssets();
        }

        // Global DnD Fix: Explicitly allow 'move' everywhere to prevent forbidden cursor
        const handleGlobalDrag = (e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move"; // CRITICAL: Must match effectAllowed
            return false;
        };
        window.addEventListener("dragover", handleGlobalDrag, false);
        window.addEventListener("drop", (e) => e.preventDefault(), false); // Prevent actual file open behavior

        // Load UI settings from backend
        try {
            const settings = await core.invoke("load_app_settings");
            if (settings.hidden_assets)
                hiddenAssets = new Set(settings.hidden_assets);
            if (settings.asset_order) assetOrder = settings.asset_order;
        } catch (err) {
            console.warn("Failed to load app settings:", err);
            // Fallback to localStorage for compatibility or first run
            const savedHidden = localStorage.getItem("hemp0x_hidden_assets");
            if (savedHidden) {
                try {
                    hiddenAssets = new Set(JSON.parse(savedHidden));
                } catch (e) {}
            }
            const savedOrder = localStorage.getItem("hemp0x_asset_order");
            if (savedOrder) {
                try {
                    assetOrder = JSON.parse(savedOrder);
                } catch (e) {}
            }
        }
    });

    // Save helpers
    async function persistSettings() {
        if (!tauriReady) return;
        try {
            // We need to preserve other settings (hide_balance etc) so we should ideally load first or just partial update if we had a partial update command.
            // For now, let's load current, update ours, and save.
            let current = await core.invoke("load_app_settings");
            current.hidden_assets = [...hiddenAssets];
            current.asset_order = assetOrder;
            await core.invoke("save_app_settings", { settings: current });
        } catch (e) {
            console.warn("Persist failed:", e);
        }
    }

    // Re-check when parent state changes
    $: if (tauriReady && isNodeOnline) {
        refreshAssets();
    }

    async function refreshAssets() {
        if (!tauriReady) return;
        try {
            myAssets = await core.invoke("list_assets");
            nodeOnline = true;
            status = "";
            if (!selectedAsset && myAssets.length > 0)
                selectedAsset = myAssets[0].name;
            if (!reissueAsset && myAssets.length > 0)
                reissueAsset = myAssets[0].name;
        } catch (err) {
            console.warn("list_assets error:", err);
            nodeOnline = false;
            status = String(err).includes("error") ? "Node may be offline" : "";
            myAssets = [];
        }
    }

    // Group assets: bundle OWNER tokens (!) with their parent TOKEN
    $: groupedAssets = (() => {
        const groups = new Map();
        const owners = new Map();

        // First pass: separate owners from tokens
        for (const asset of myAssets) {
            if (asset.name.endsWith("!")) {
                // This is an owner token
                const baseName = asset.name.slice(0, -1);
                owners.set(baseName, asset);
            } else {
                // Regular token or sub-asset
                groups.set(asset.name, {
                    ...asset,
                    hasOwner: false,
                    ownerBalance: null,
                    isSubAsset: asset.name.includes("/"),
                    parentName: asset.name.includes("/")
                        ? asset.name.split("/")[0]
                        : null,
                });
            }
        }

        // Second pass: attach owners to their parent tokens
        for (const [baseName, ownerAsset] of owners) {
            if (groups.has(baseName)) {
                // Parent exists, attach owner info
                const parent = groups.get(baseName);
                parent.hasOwner = true;
                parent.ownerBalance = ownerAsset.balance;
            } else {
                // Orphan owner (no matching token visible) - show it separately
                groups.set(ownerAsset.name, {
                    ...ownerAsset,
                    hasOwner: true,
                    ownerBalance: ownerAsset.balance,
                    isSubAsset: false,
                    parentName: null,
                });
            }
        }

        let results = Array.from(groups.values());

        // 1. FILTER: Remove hidden (unless showHidden is true)
        if (!showHidden) {
            results = results.filter((a) => !hiddenAssets.has(a.name));
        }

        // 2. SORT: Use persisted order
        if (assetOrder.length > 0) {
            const orderMap = new Map(assetOrder.map((n, i) => [n, i]));
            results.sort((a, b) => {
                const idxA = orderMap.has(a.name)
                    ? orderMap.get(a.name)
                    : 99999;
                const idxB = orderMap.has(b.name)
                    ? orderMap.get(b.name)
                    : 99999;

                // Secondary sort: Owner tokens first, then alphabetic
                if (idxA === 99999 && idxB === 99999) {
                    if (a.hasOwner && !b.hasOwner) return -1;
                    if (!a.hasOwner && b.hasOwner) return 1;
                    return a.name.localeCompare(b.name);
                }
                return idxA - idxB;
            });
        }

        return results;
    })();

    // --- UI ACTIONS ---
    function toggleHide(e, assetName) {
        e.stopPropagation();
        if (hiddenAssets.has(assetName)) {
            hiddenAssets.delete(assetName);
        } else {
            hiddenAssets.add(assetName);
        }
        hiddenAssets = hiddenAssets; // trigger reactivity
        persistSettings();
    }

    function handleDragStart(e, asset) {
        draggingItem = asset;
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", asset.name);
        // HACK: Delay adding class to avoid browser canceling drag immediately
        setTimeout(() => document.body.classList.add("dragging-active"), 0);
    }

    function handleDragOver(e) {
        e.preventDefault();
        e.stopPropagation();
        e.dataTransfer.dropEffect = "move";
        return false;
    }

    let lastSwap = 0;

    function handleDragEnter(e, targetAsset) {
        // Debounce swaps to prevent fluttering (rapid oscillations)
        const now = Date.now();
        if (now - lastSwap < 150) return;

        // Real-time Sort: Swap items as we drag over them
        if (!draggingItem || draggingItem.name === targetAsset.name) return;

        // Create initial order if empty
        if (assetOrder.length === 0) {
            assetOrder = groupedAssets.map((a) => a.name);
        }

        let currentOrder = [...assetOrder];

        // Ensure completeness
        groupedAssets.forEach((a) => {
            if (!currentOrder.includes(a.name)) currentOrder.push(a.name);
        });

        const fromIdx = currentOrder.indexOf(draggingItem.name);
        const toIdx = currentOrder.indexOf(targetAsset.name);

        if (fromIdx !== -1 && toIdx !== -1) {
            // Move item
            currentOrder.splice(fromIdx, 1);
            currentOrder.splice(toIdx, 0, draggingItem.name);
            assetOrder = currentOrder;
            lastSwap = now;
        }
    }

    function handleDragEnd(e) {
        document.body.classList.remove("dragging-active");
        draggingItem = null;
    }

    function handleDrop(e, targetAsset) {
        e.preventDefault();
        e.stopPropagation();
        document.body.classList.remove("dragging-active");
        draggingItem = null;
        // Just save the final state
        persistSettings();
    }

    import { formatBalance } from "./utils.js";

    // Root assets only (for sub-asset parent selection)
    $: rootAssets = myAssets.filter(
        (a) =>
            !a.name.includes("/") &&
            !a.name.endsWith("!") &&
            a.name === a.name.toUpperCase(), // Only uppercase root assets
    );

    // Asset metadata from getassetdata
    let assetMetadata = null;
    let metadataLoading = false;
    let slideDirection = 0; // -1 = left, 1 = right, 0 = none

    async function openDetail(asset) {
        selectedDetail = asset;
        assetMetadata = null;
        metadataLoading = true;

        try {
            // Fetch full asset data from CLI
            const data = await core.invoke("get_asset_data", {
                name: asset.name,
            });
            assetMetadata = data;
        } catch (err) {
            console.warn("get_asset_data error:", err);
            // Non-fatal - just show basic info
        }
        metadataLoading = false;
    }

    function closeDetail() {
        selectedDetail = null;
        assetMetadata = null;
        slideDirection = 0;
    }

    function navigatePrev() {
        if (!selectedDetail || groupedAssets.length <= 1) return;
        const currentIdx = groupedAssets.findIndex(
            (a) => a.name === selectedDetail.name,
        );
        const prevIdx =
            currentIdx <= 0 ? groupedAssets.length - 1 : currentIdx - 1;
        slideDirection = -1;
        openDetail(groupedAssets[prevIdx]);
    }

    function navigateNext() {
        if (!selectedDetail || groupedAssets.length <= 1) return;
        const currentIdx = groupedAssets.findIndex(
            (a) => a.name === selectedDetail.name,
        );
        const nextIdx =
            currentIdx >= groupedAssets.length - 1 ? 0 : currentIdx + 1;
        slideDirection = 1;
        openDetail(groupedAssets[nextIdx]);
    }

    function goToTransfer(assetName) {
        selectedAsset = assetName;
        // selectedDetail = null; // Keep detail open behind it
        transferModalOpen = true; // Open as Modal
    }

    function initiateTransfer() {
        if (!selectedAsset || !transferTo || !transferAmt) {
            status = "Fill all fields.";
            return;
        }
        confirmPayload = {
            asset: selectedAsset,
            to: transferTo,
            amount: transferAmt,
        };
        confirmType = "TRANSFER";
        confirmOpen = true;
    }

    function initiateIssue() {
        if (!issueName || !issueQty) {
            status = "Name and Qty required.";
            return;
        }
        if (issueType === "sub" && !issueParent) {
            status = "Parent asset required for sub-asset.";
            return;
        }

        // Build full asset name
        let fullName = issueName.toUpperCase();
        if (issueType === "sub") {
            fullName = `${issueParent}/${issueName.toUpperCase()}`;
        }

        confirmPayload = {
            name: fullName,
            qty: issueQty,
            units: issueUnits,
            reissuable: issueReissue,
            ipfs: issueIpfs || null,
            type: issueType,
        };
        confirmType = "ISSUE";
        confirmOpen = true;
    }

    function goToSubAsset(parentName) {
        issueParent = parentName;
        issueType = "sub";
        issueName = "";
        issueQty = "1";
        issueUnits = 0;
        issueReissue = true;
        issueIpfs = "";
        subModalOpen = true; // Open as Modal
    }

    function goToNft(parentName) {
        issueParent = parentName;
        issueType = "nft";
        nftTag = "";
        issueIpfs = "";
        nftModalOpen = true; // Open as Modal
    }

    async function openGovernance(asset) {
        // We need full asset data (units, ipfs, etc) for governance
        try {
            const details = await core.invoke("get_asset_data", {
                name: asset.name,
            });
            selectedGovAsset = { ...asset, ...details }; // Merge list info with full details
            govModalOpen = true;
        } catch (e) {
            console.error("Failed to load details for governance", e);
        }
    }

    function initiateReissue() {
        if (!reissueAsset || !reissueQty) {
            status = "Asset and Qty required.";
            return;
        }
        confirmPayload = {
            name: reissueAsset,
            qty: reissueQty,
            reissuable: reissueReissuable,
        };
        confirmType = "REISSUE";
        confirmOpen = true;
    }

    function initiateNft() {
        if (!issueParent) {
            status = "Parent asset required for NFT.";
            return;
        }
        if (!nftTag.trim()) {
            status = "NFT tag name required.";
            return;
        }
        confirmPayload = {
            rootName: issueParent,
            tag: nftTag.trim(),
            ipfs: issueIpfs || "",
        };
        confirmType = "NFT";
        confirmOpen = true;
    }

    async function confirmAction() {
        if (!tauriReady) return;
        try {
            let txid = "";
            if (confirmType === "TRANSFER") {
                txid = await core.invoke("transfer_asset", {
                    asset: confirmPayload.asset,
                    amount: confirmPayload.amount,
                    to: confirmPayload.to,
                });
                status = `Sent! TXID: ${txid.slice(0, 16)}...`;
                transferTo = "";
                transferAmt = "";
            } else if (confirmType === "ISSUE") {
                txid = await core.invoke("issue_asset", {
                    name: confirmPayload.name,
                    qty: String(confirmPayload.qty),
                    units: Number(confirmPayload.units),
                    reissuable: confirmPayload.reissuable,
                    ipfs: confirmPayload.ipfs || "",
                });
                status = `${confirmPayload.type === "sub" ? "Sub-asset" : "Asset"} created! TXID: ${txid.slice(0, 16)}...`;
                issueName = "";
                issueQty = "1";
                issueIpfs = "";
                if (issueType === "sub") issueParent = "";
            } else if (confirmType === "REISSUE") {
                txid = await core
                    .invoke("reissue_asset", {
                        name: confirmPayload.name,
                        qty: String(confirmPayload.qty),
                        toAddress: "",
                        changeAddress: "",
                        reissuable: confirmPayload.reissuable,
                        newUnits: null,
                        newIpfs: "",
                    })
                    .catch((e) => {
                        throw "Reissue failed: " + e;
                    });
                status = `Reissued! TXID: ${txid.slice(0, 16)}...`;
            } else if (confirmType === "NFT") {
                txid = await core.invoke("issue_unique_asset", {
                    rootName: confirmPayload.rootName,
                    tags: [confirmPayload.tag],
                    ipfsHashes: confirmPayload.ipfs
                        ? [confirmPayload.ipfs]
                        : [],
                });
                status = `NFT minted: ${confirmPayload.rootName}#${confirmPayload.tag}! TXID: ${txid.slice(0, 16)}...`;
                nftTag = "";
                issueIpfs = "";
                issueParent = "";
                issueType = "root";
            }
            confirmOpen = false;
            refreshAssets();
        } catch (err) {
            status = "Error: " + err;
            confirmOpen = false;
        }
    }
</script>

<div class="view-assets">
    <div class="cyber-panel main-frame">
        <!-- HEADER -->
        <header class="panel-header">
            <div class="header-left">
                <span class="header-title">◈ MY ASSETS</span>
            </div>
            <div class="header-options">
                <Tooltip
                    text={showHidden ? "Hide hidden" : "Show hidden assets"}
                >
                    <label class="toggle-hidden">
                        <input type="checkbox" bind:checked={showHidden} />
                        <img
                            src={showHidden ? eyeOpen : eyeClosed}
                            alt="Visibility"
                            class="eye-icon-img"
                        />
                    </label>
                </Tooltip>
            </div>
            <div class="header-actions">
                <button
                    class="header-btn create-btn"
                    on:click={() => (createModalOpen = true)}
                    disabled={!nodeOnline}
                    title="Create New Root Asset"
                >
                    <span class="btn-icon">+</span> CREATE
                </button>
                <button
                    class="header-btn browse-btn"
                    on:click={() => (browseModalOpen = true)}
                    disabled={!nodeOnline}
                    title="Browse Network Assets"
                >
                    <span class="btn-icon">🔍</span> BROWSE
                </button>
            </div>
            <div class="header-status">
                <span class="pulse-dot" class:online={nodeOnline}></span>
                <span class="status-label"
                    >{nodeOnline ? "CONNECTED" : "OFFLINE"}</span
                >
            </div>
        </header>

        <!-- CONTENT -->
        <div class="content-area">
            <div class="tab-content">
                <!-- ═══════════════ MY ASSETS ═══════════════ -->
                <div
                    class="asset-grid"
                    on:dragover={handleDragOver}
                    on:drop={handleDragEnd}
                    role="group"
                >
                    {#each groupedAssets as asset (asset.name)}
                        <div
                            class="asset-card glass-card"
                            class:has-owner={asset.hasOwner}
                            class:is-sub-asset={asset.isSubAsset}
                            class:is-hidden={hiddenAssets.has(asset.name)}
                            role="button"
                            tabindex="0"
                            draggable="true"
                            on:dragstart={(e) => handleDragStart(e, asset)}
                            on:dragover={handleDragOver}
                            on:dragenter={(e) => handleDragEnter(e, asset)}
                            on:dragend={handleDragEnd}
                            on:drop={(e) => handleDrop(e, asset)}
                            animate:flip={{ duration: 300 }}
                            on:click={() => openDetail(asset)}
                            on:keydown={(e) =>
                                e.key === "Enter" && openDetail(asset)}
                        >
                            <div class="card-glow"></div>

                            <!-- Hide Toggle -->
                            <div class="hide-btn-frame">
                                <Tooltip
                                    text={hiddenAssets.has(asset.name)
                                        ? "Unhide Asset"
                                        : "Hide Asset"}
                                >
                                    <button
                                        class="hide-btn"
                                        on:click={(e) =>
                                            toggleHide(e, asset.name)}
                                    >
                                        <img
                                            src={hiddenAssets.has(asset.name)
                                                ? eyeOpen
                                                : eyeClosed}
                                            alt="Hide"
                                            class="card-eye-icon"
                                        />
                                    </button>
                                </Tooltip>
                            </div>

                            {#if asset.hasOwner}
                                <div
                                    class="owner-badge"
                                    title="Asset Ownership"
                                >
                                    👑
                                </div>
                            {/if}
                            {#if asset.isSubAsset}
                                <div
                                    class="sub-badge"
                                    title="Sub-asset of {asset.parentName}"
                                >
                                    ↳
                                </div>
                            {/if}
                            <div class="card-content">
                                <div class="asset-name">
                                    {asset.name}
                                </div>
                                <div class="asset-balance">
                                    {formatBalance(asset.balance)}
                                </div>
                                <div class="asset-meta">
                                    <span class="asset-type"
                                        >{asset.hasOwner
                                            ? "OWNER"
                                            : "LOCKED"}</span
                                    >
                                    <button
                                        class="quick-transfer"
                                        title="Transfer"
                                        on:click|stopPropagation={() =>
                                            goToTransfer(asset.name)}
                                    >
                                        →
                                    </button>
                                </div>
                            </div>
                        </div>
                    {/each}
                    {#if groupedAssets.length === 0}
                        <div class="empty-state">
                            <div class="empty-icon">◈</div>
                            <div class="empty-text">
                                {nodeOnline
                                    ? "No assets in wallet"
                                    : "Connect node to view assets"}
                            </div>
                        </div>
                    {/if}
                </div>
            </div>
        </div>

        <!-- STATUS BAR -->
        {#if status}
            <div
                class="status-bar"
                class:error={status.startsWith("Error")}
                transition:fly={{ y: 10 }}
            >
                <span class="status-indicator">▶</span>
                {status}
            </div>
        {/if}
    </div>

    <!-- ═══════════════ ASSET DETAIL MODAL ═══════════════ -->
    <!-- ═══════════════ ASSET DETAIL MODAL ═══════════════ -->
    <ModalAssetDetail
        asset={selectedDetail}
        metadata={assetMetadata}
        loading={metadataLoading}
        {slideDirection}
        hasMultipleAssets={groupedAssets.length > 1}
        on:close={closeDetail}
        on:prev={navigatePrev}
        on:next={navigateNext}
        on:transfer={(e) => goToTransfer(e.detail.name)}
        on:reissue={(e) => {
            reissueAsset = e.detail.name;
            reissueModalOpen = true;
        }}
        on:createSub={(e) => goToSubAsset(e.detail.name)}
        on:createNft={(e) => goToNft(e.detail.name)}
        on:gov={(e) => openGovernance(e.detail)}
    />

    <!-- ═══════════════ CREATE MODAL (ROOT) ═══════════════ -->
    <!-- ═══════════════ CREATE MODAL (ROOT) ═══════════════ -->
    <ModalIssueAsset
        isOpen={createModalOpen}
        {nodeOnline}
        bind:name={issueName}
        bind:qty={issueQty}
        bind:units={issueUnits}
        bind:ipfs={issueIpfs}
        bind:reissuable={issueReissue}
        on:close={() => (createModalOpen = false)}
        on:create={() => {
            issueType = "root";
            initiateIssue();
            createModalOpen = false;
        }}
    />

    <!-- ═══════════════ SUB-ASSET MODAL ═══════════════ -->
    <!-- ═══════════════ SUB-ASSET MODAL ═══════════════ -->
    <ModalIssueSub
        isOpen={subModalOpen}
        {nodeOnline}
        parentName={issueParent}
        bind:name={issueName}
        bind:qty={issueQty}
        bind:reissuable={issueReissue}
        on:close={() => (subModalOpen = false)}
        on:create={() => {
            initiateIssue();
            subModalOpen = false;
        }}
    />

    <!-- ═══════════════ NFT MODAL ═══════════════ -->
    <!-- ═══════════════ NFT MODAL ═══════════════ -->
    <ModalIssueNFT
        isOpen={nftModalOpen}
        {nodeOnline}
        parentName={issueParent}
        bind:tag={nftTag}
        bind:ipfs={issueIpfs}
        on:close={() => (nftModalOpen = false)}
        on:create={() => {
            initiateNft();
            nftModalOpen = false;
        }}
    />

    <!-- ═══════════════ TRANSFER MODAL ═══════════════ -->
    <!-- ═══════════════ TRANSFER MODAL ═══════════════ -->
    <ModalTransfer
        isOpen={transferModalOpen}
        {nodeOnline}
        assets={myAssets}
        bind:selectedAsset
        bind:toAddress={transferTo}
        bind:amount={transferAmt}
        on:close={() => (transferModalOpen = false)}
        on:transfer={() => {
            initiateTransfer();
            transferModalOpen = false;
        }}
    />

    <!-- ═══════════════ REISSUE MODAL ═══════════════ -->
    <!-- ═══════════════ REISSUE MODAL ═══════════════ -->
    <ModalReissue
        isOpen={reissueModalOpen}
        {nodeOnline}
        assets={myAssets}
        bind:name={reissueAsset}
        bind:qty={reissueQty}
        bind:reissuable={reissueReissuable}
        on:close={() => (reissueModalOpen = false)}
        on:reissue={() => {
            initiateReissue();
            reissueModalOpen = false;
        }}
    />

    <!-- ═══════════════ BROWSE MODAL ═══════════════ -->
    <!-- ═══════════════ BROWSE MODAL ═══════════════ -->
    <ModalBrowse
        isOpen={browseModalOpen}
        on:close={() => (browseModalOpen = false)}
    />

    <ModalAssetGovernance
        isOpen={govModalOpen}
        asset={selectedGovAsset}
        on:close={() => (govModalOpen = false)}
    />

    <!-- ═══════════════ CONFIRM MODAL ═══════════════ -->
    <!-- ═══════════════ CONFIRM MODAL ═══════════════ -->
    <ModalConfirm
        isOpen={confirmOpen}
        type={confirmType}
        payload={confirmPayload}
        on:close={() => (confirmOpen = false)}
        on:confirm={confirmAction}
    />
</div>

<style>
    /* ═══════════════ BASE LAYOUT ═══════════════ */
    .view-assets {
        height: 100%;
        display: flex;
        flex-direction: column;
    }
    .main-frame {
        flex: 1;
        display: flex;
        flex-direction: column;
        background: linear-gradient(
            180deg,
            rgba(8, 14, 12, 0.95) 0%,
            rgba(5, 10, 8, 0.98) 100%
        );
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 16px;
        overflow: hidden;
        box-shadow:
            0 0 60px rgba(0, 0, 0, 0.5),
            inset 0 0 80px rgba(0, 255, 65, 0.02);
    }

    /* ═══════════════ HEADER / NAV ═══════════════ */
    .panel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 5px 1rem; /* Added vertical padding for button glows/borders */
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }

    .header-status {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: 0.65rem;
        color: #555;
        letter-spacing: 1px;
    }
    .header-left {
        display: flex;
        align-items: center;
    }
    .header-options {
        margin-left: 1rem;
        display: flex;
        align-items: center;
    }
    .toggle-hidden {
        cursor: pointer;
        display: flex;
        align-items: center;
        background: rgba(255, 255, 255, 0.05);
        padding: 4px 8px;
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        transition: all 0.2s;
    }
    .toggle-hidden:hover {
        background: rgba(255, 255, 255, 0.1);
    }
    .toggle-hidden input {
        display: none;
    }
    .eye-icon-img {
        width: 20px;
        height: 20px;
        opacity: 0.8;
        transition: opacity 0.2s;
    }
    .toggle-hidden:hover .eye-icon-img {
        opacity: 1;
    }
    .header-title {
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 2px;
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
    }
    .header-actions {
        display: flex;
        gap: 0.5rem;
        margin-left: auto;
        margin-right: 1rem;
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
        display: flex;
        align-items: center;
        gap: 0.4rem;
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
    .btn-icon {
        font-size: 0.9rem;
    }
    .pulse-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: #333;
        animation: pulse 2s infinite;
    }
    .pulse-dot.online {
        background: var(--color-primary);
        box-shadow: 0 0 8px var(--color-primary);
    }
    @keyframes pulse {
        0%,
        100% {
            opacity: 1;
        }
        50% {
            opacity: 0.5;
        }
    }

    /* ═══════════════ CONTENT AREA ═══════════════ */
    .content-area {
        flex: 1;
        overflow: hidden;
        display: flex;
    }
    .tab-content {
        flex: 1;
        padding: 1.5rem;
        overflow-y: auto;
    }

    /* ═══════════════ ASSET GRID (Card Layout) ═══════════════ */
    .asset-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
        gap: 1rem;
    }
    .asset-card {
        user-select: none;
        -webkit-user-select: none;
        position: relative;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        padding: 1.2rem;
        cursor: grab;
        transition: all 0.25s;
        overflow: visible !important; /* Allow tooltip to break out */
    }
    .asset-card img {
        -webkit-user-drag: none;
        pointer-events: none;
    }
    .asset-card:active {
        cursor: grabbing;
    }
    .asset-card:hover {
        border-color: rgba(0, 255, 65, 0.4);
        transform: translateY(-2px);
        box-shadow:
            0 8px 30px rgba(0, 0, 0, 0.4),
            0 0 20px rgba(0, 255, 65, 0.1);
    }
    .asset-card:hover .card-glow {
        opacity: 1;
    }
    .card-glow {
        position: absolute;
        inset: 0;
        background: radial-gradient(
            circle at 50% 120%,
            rgba(0, 255, 65, 0.08) 0%,
            transparent 60%
        );
        opacity: 0;
        transition: opacity 0.3s;
        pointer-events: none;
    }
    .card-content {
        position: relative;
        z-index: 1;
    }

    /* Owner Badge - Crown icon for owned assets */
    .owner-badge {
        position: absolute;
        top: 8px;
        right: 8px;
        font-size: 1rem;
        z-index: 2;
        filter: drop-shadow(0 0 4px rgba(255, 215, 0, 0.6));
    }

    /* Sub-asset indicator */
    .sub-badge {
        position: absolute;
        top: 8px;
        left: 8px;
        font-size: 0.8rem;
        color: #888;
        z-index: 2;
    }

    /* Hide Button */
    .hide-btn-frame {
        position: absolute;
        top: 8px;
        right: 32px; /* Next to crown/badge */
        z-index: 10;
        opacity: 0;
        transition: opacity 0.2s;
        transform: translateY(-5px);
    }
    .asset-card {
        overflow: visible !important; /* Allow tooltip to break out */
    }
    .asset-card:hover .hide-btn-frame {
        opacity: 1;
        transform: translateY(0);
    }
    .hide-btn {
        background: none;
        border: none;
        cursor: pointer;
        padding: 4px;
        border-radius: 50%;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .hide-btn:hover {
        background: rgba(255, 255, 255, 0.1);
    }
    .card-eye-icon {
        width: 16px;
        height: 16px;
        opacity: 0.7;
    }
    .hide-btn:hover .card-eye-icon {
        opacity: 1;
    }

    /* Hidden State */
    .asset-card.is-hidden {
        opacity: 0.5;
        border: 1px dashed #444;
        filter: grayscale(0.8);
    }
    .asset-card.is-hidden:hover {
        opacity: 0.8;
    }

    /* Highlight cards with owner tokens */
    .asset-card.has-owner {
        border-color: rgba(255, 215, 0, 0.3);
    }
    .asset-card.has-owner:hover {
        border-color: rgba(255, 215, 0, 0.5);
        box-shadow:
            0 8px 30px rgba(0, 0, 0, 0.4),
            0 0 20px rgba(255, 215, 0, 0.15);
    }

    /* Sub-asset indent styling */
    .asset-card.is-sub-asset {
        border-left: 3px solid rgba(0, 255, 65, 0.3);
    }
    .asset-name {
        font-size: 0.9rem;
        font-weight: 700;
        color: #fff;
        letter-spacing: 1px;
        margin-bottom: 0.5rem;
        text-overflow: ellipsis;
        overflow: hidden;
        white-space: nowrap;
    }
    .asset-balance {
        font-size: 1.4rem;
        font-weight: 700;
        color: var(--color-primary);
        font-family: var(--font-mono);
        text-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
        margin-bottom: 0.8rem;
    }
    .asset-meta {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .asset-type {
        font-size: 0.55rem;
        padding: 3px 8px;
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 6px;
        color: #777;
        letter-spacing: 1px;
        background: rgba(0, 0, 0, 0.3);
    }
    .quick-transfer {
        width: 28px;
        height: 28px;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.3);
        border-radius: 8px;
        color: var(--color-primary);
        font-size: 1rem;
        cursor: pointer;
        transition: all 0.15s;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .quick-transfer:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px var(--color-primary);
    }

    .empty-state {
        grid-column: 1 / -1;
        text-align: center;
        padding: 4rem 2rem;
        color: #444;
    }
    .empty-icon {
        font-size: 3rem;
        margin-bottom: 1rem;
        opacity: 0.3;
    }

    /* Owner status highlight in detail modal */

    /* ═══════════════ BUTTONS ═══════════════ */

    /* ═══════════════ STATUS BAR ═══════════════ */
    .status-bar {
        padding: 0.4rem 1rem;
        font-size: 0.65rem;
        color: #666;
        background: rgba(0, 0, 0, 0.5);
        border-top: 1px solid rgba(255, 255, 255, 0.03);
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .status-bar.error {
        color: #ff5555;
    }
    .status-indicator {
        color: var(--color-primary);
    }

    /* ═══════════════ SCROLLBAR ═══════════════ */
    .tab-content::-webkit-scrollbar {
        width: 6px;
    }
    .tab-content::-webkit-scrollbar-track {
        background: transparent;
    }
    .tab-content::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.1);
        border-radius: 3px;
    }
    .tab-content::-webkit-scrollbar-thumb:hover {
        background: var(--color-primary);
    }

    /* Dragging Fix: Disable pointer events on children during drag */
    :global(body.dragging-active) .asset-card * {
        pointer-events: none !important;
    }
</style>
