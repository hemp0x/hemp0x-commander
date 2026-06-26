<script>
    import { onDestroy, onMount } from "svelte";
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
    import ModalAssetAdvanced from "./modals/ModalAssetAdvanced.svelte";
    import Tooltip from "./ui/Tooltip.svelte";
    import WalletUnlockModal from "./ui/WalletUnlockModal.svelte";
    import {
        loadWalletPinStatus,
        defaultUnlockMode,
        pinRequiresPassphrase,
        unlockWalletWithPin,
        unlockRuntimeWalletWithPassphrase,
        forgotWalletPin,
        isValidPin,
    } from "./walletPinUnlock.js";
    import eyeOpen from "../assets/eye-open.png";
    import eyeClosed from "../assets/eye-closed.png";

    /**
     * @typedef {{ name: string, balance?: number|string, type?: string, hasOwner?: boolean, units?: number }} AssetItem
     * @typedef {{ name: string, amount?: number, units?: number, reissuable?: boolean, block_height?: number, has_ipfs?: boolean, ipfs_hash?: string }} AssetMetadata
     * @typedef {AssetItem & { hasOwner: boolean, ownerBalance: string|number|null, isSubAsset: boolean, parentName: string|null, isQualifier: boolean, isRestricted: boolean }} GroupedAsset
     * @typedef {{
     *   asset?: string,
     *   to?: string,
     *   amount?: string|number,
     *   name?: string,
     *   qty?: string|number,
     *   units?: number|string,
     *   reissuable?: boolean,
     *   ipfs?: string,
     *   ipfs_hash?: string,
     *   asset_name?: string,
     *   operation_type?: string,
     *   parent_asset?: string,
     *   tags?: string[],
     *   summary?: string,
     *   warnings?: string[]
     * }} IssuePreviewData
     * @typedef {{ id: string, status: string, operation_type: string, summary?: string, txid?: string|null, details?: unknown }} JournalEntry
     * @typedef {{ asset: string, to: string, amount: string|number }} TransferPayload
     * @typedef {AssetItem & AssetMetadata} AssetDetail
     */

    /** @type {AssetItem[]} */
    let myAssets = [];
    let tauriReady = false;
    let status = "";

    // Detail View
    /** @type {AssetDetail | null} */
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
    /** @type {AssetDetail | null} */
    let selectedGovAsset = null;
    let advancedModalOpen = false;
    let advancedInitialTab = "";
    let advancedInitialTagName = "";

    // Back navigation: remember last detail asset when opening action panels
    /** @type {AssetDetail | null} */
    let lastDetailAsset = null;
    let lastDetailTab = "DETAILS";
    let initialDetailTab = "DETAILS";

    // Track the active tab inside the current detail view for back-navigation
    let activeDetailTab = "DETAILS";

    // Browse Modal
    let browseModalOpen = false;
    let detailFromBrowse = false;
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
    let reissueNewIpfs = "";

    // Confirm Modal
    let confirmOpen = false;
    /** @type {TransferPayload | null} */
    let confirmPayload = null;
    let confirmType = "";
    /** @type {IssuePreviewData | null} */
    let previewData = null;
    /** @type {string | null} */
    let previewJournalId = null;
    let isBroadcasting = false;

    // Persistent UI State
    let showHidden = false;
    let typeFilter = "ALL"; // ALL | TOKEN | OWNER | SUB | NFT | QUALIFIER | RESTRICTED

    /**
     * @param {GroupedAsset} a
     * @param {string} filter
     * @returns {boolean}
     */
    function matchesTypeFilter(a, filter) {
        switch (filter) {
            case "OWNER":
                return a.name.endsWith("!") && a.hasOwner;
            case "TOKEN":
                return !a.isSubAsset && !a.isQualifier && !a.isRestricted
                    && !a.name.endsWith("!") && !a.name.includes("#");
            case "SUB":
                return a.isSubAsset;
            case "NFT":
                return a.name.includes("#") && !a.isQualifier;
            case "QUALIFIER":
                return a.isQualifier;
            case "RESTRICTED":
                return a.isRestricted;
            default:
                return true;
        }
    }

    /**
     * @param {GroupedAsset} a
     * @returns {string}
     */
    function assetTypeLabel(a) {
        if (a.name.endsWith("!")) return "OWNER";
        if (a.isQualifier) return "QUALIFIER";
        if (a.isRestricted) return "RESTRICTED";
        if (a.name.includes("#")) return "NFT";
        if (a.isSubAsset) return "SUB-ASSET";
        return "TOKEN";
    }

    // Wallet unlock state for broadcast retry
    let showUnlockModal = false;
    let unlockPassword = "";
    let unlocking = false;
    let unlockError = "";
    // Runtime Wallet PIN unlock (slice 76b)
    let walletPinStatus = null;
    let walletUnlockMode = "passphrase";
    let unlockPin = "";
    $: walletPinUsable = !!walletPinStatus?.pin_configured && !pinRequiresPassphrase(walletPinStatus);
    /** @type {Set<string>} */
    let hiddenAssets = new Set();
    /** @type {string[]} */
    let assetOrder = [];
    /** @type {AssetItem | null} */
    let draggingItem = null;

    // Allow parent to pass initial state

    import { nodeStatus } from "../stores.js";
    import { addNotification } from "./stores/notifications.js";
    import { ensureNodeSyncedForBroadcast } from "./utils/nodeSync.js";
    $: isNodeOnline = $nodeStatus.online;

    let nodeOnline = false;
    $: modalBlocking = advancedModalOpen || createModalOpen || browseModalOpen
        || !!selectedDetail || transferModalOpen || reissueModalOpen
        || subModalOpen || nftModalOpen || govModalOpen;

    /** @param {DragEvent} e */
    function handleGlobalDrag(e) {
        e.preventDefault();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    }

    /** @param {DragEvent} e */
    function preventGlobalDrop(e) {
        e.preventDefault();
    }

    function openNetworkAssetBrowser() {
        browseModalOpen = true;
        advancedModalOpen = false;
        createModalOpen = false;
        selectedDetail = null;
    }

    onMount(async () => {
        tauriReady =
            typeof core?.isTauri === "function" ? core.isTauri() : false;
        if (tauriReady) {
            // Try to refresh assets, will fail gracefully if node is offline
            await refreshAssets();
        }

        // Global DnD Fix: Explicitly allow 'move' everywhere to prevent forbidden cursor
        window.addEventListener("dragover", handleGlobalDrag, false);
        window.addEventListener("drop", preventGlobalDrop, false);
        window.addEventListener("commander-open-asset-browser", openNetworkAssetBrowser);

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

    onDestroy(() => {
        dragGhostEl?.remove();
        dragGhostEl = null;
        window.removeEventListener("dragover", handleGlobalDrag, false);
        window.removeEventListener("drop", preventGlobalDrop, false);
        window.removeEventListener("commander-open-asset-browser", openNetworkAssetBrowser);
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
                const isQualifier = asset.name.startsWith("#");
                const isRestricted = asset.name.startsWith("$");
                groups.set(asset.name, {
                    ...asset,
                    hasOwner: false,
                    ownerBalance: null,
                    isSubAsset: asset.name.includes("/") && !isQualifier && !isRestricted,
                    parentName: asset.name.includes("/")
                        ? asset.name.split("/")[0]
                        : null,
                    isQualifier,
                    isRestricted,
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
                const isQualifier = ownerAsset.name.startsWith("#");
                const isRestricted = ownerAsset.name.startsWith("$");
                groups.set(ownerAsset.name, {
                    ...ownerAsset,
                    hasOwner: true,
                    ownerBalance: ownerAsset.balance,
                    isSubAsset: false,
                    parentName: null,
                    isQualifier,
                    isRestricted,
                });
            }
        }

        let results = Array.from(groups.values());

        // 1. FILTER: Remove hidden (unless showHidden is true)
        if (!showHidden) {
            results = results.filter((a) => !hiddenAssets.has(a.name));
        }

        // 1b. FILTER: Asset type chip filter
        if (typeFilter !== "ALL") {
            results = results.filter((a) => matchesTypeFilter(a, typeFilter));
        }

        // 2. SORT: Use persisted order
        if (assetOrder.length > 0) {
            const orderMap = new Map(assetOrder.map((n, i) => [n, i]));
            results.sort((a, b) => {
                const idxA = orderMap.has(a.name)
                    ? /** @type {number} */ (orderMap.get(a.name))
                    : 99999;
                const idxB = orderMap.has(b.name)
                    ? /** @type {number} */ (orderMap.get(b.name))
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
    /**
     * @param {MouseEvent} e
     * @param {string} assetName
     */
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

    /**
     * @param {DragEvent} e
     * @param {AssetItem} asset
     */
    function handleDragStart(e, asset) {
        draggingItem = asset;
        if (e.dataTransfer) {
            e.dataTransfer.effectAllowed = "move";
            e.dataTransfer.setData("text/plain", asset.name);
            // Use a tiny 1x1 hidden image as the drag preview so the native
            // browser ghost doesn't render the full card at its rendered size.
            if (!dragGhostEl) {
                dragGhostEl = document.createElement("div");
                dragGhostEl.style.cssText =
                    "position:absolute;top:-9999px;left:-9999px;width:1px;height:1px;opacity:0;pointer-events:none;";
                document.body.appendChild(dragGhostEl);
            }
            e.dataTransfer.setDragImage(dragGhostEl, 0, 0);
        }
        // HACK: Delay adding class to avoid browser canceling drag immediately
        setTimeout(() => document.body.classList.add("dragging-active"), 0);
    }

    /** @param {DragEvent} e */
    function handleDragOver(e) {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
        return false;
    }

    let lastSwap = 0;

    /**
     * @param {DragEvent} e
     * @param {AssetItem} targetAsset
     */
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

    /** @param {DragEvent} e */
    function handleDragEnd(e) {
        document.body.classList.remove("dragging-active");
        draggingItem = null;
    }

    /** @type {HTMLDivElement | null} */
    let dragGhostEl = null;

    /**
     * @param {DragEvent} e
     * @param {AssetItem} targetAsset
     */
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
    /** @type {AssetMetadata | null} */
    let assetMetadata = null;
    let metadataLoading = false;
    let slideDirection = 0; // -1 = left, 1 = right, 0 = none

    /**
     * @param {AssetItem} asset
     * @param {string} [tab]
     */
    async function openDetail(asset, tab = "DETAILS") {
        selectedDetail = asset;
        advancedModalOpen = false;
        createModalOpen = false;
        browseModalOpen = false;
        transferModalOpen = false;
        reissueModalOpen = false;
        subModalOpen = false;
        nftModalOpen = false;
        govModalOpen = false;
        assetMetadata = null;
        metadataLoading = true;
        initialDetailTab = tab;

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
        if (detailFromBrowse) {
            detailFromBrowse = false;
            browseModalOpen = true;
        }
    }

    function navigatePrev() {
        if (!selectedDetail || groupedAssets.length <= 1) return;
        const current = selectedDetail;
        const currentIdx = groupedAssets.findIndex(
            (a) => a.name === current.name,
        );
        const prevIdx =
            currentIdx <= 0 ? groupedAssets.length - 1 : currentIdx - 1;
        slideDirection = -1;
        openDetail(groupedAssets[prevIdx], activeDetailTab);
    }

    function navigateNext() {
        if (!selectedDetail || groupedAssets.length <= 1) return;
        const current = selectedDetail;
        const currentIdx = groupedAssets.findIndex(
            (a) => a.name === current.name,
        );
        const nextIdx =
            currentIdx >= groupedAssets.length - 1 ? 0 : currentIdx + 1;
        slideDirection = 1;
        openDetail(groupedAssets[nextIdx], activeDetailTab);
    }

    /**
     * @param {string} assetName
     */
    function goToTransfer(assetName) {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        selectedAsset = assetName;
        selectedDetail = null;
        detailFromBrowse = false;
        transferModalOpen = true;
    }

    /**
     * @param {AssetItem} asset
     */
    function goToReissue(asset) {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        reissueAsset = asset.name;
        selectedDetail = null;
        detailFromBrowse = false;
        reissueModalOpen = true;
    }

    /**
     * @param {AssetItem} [asset]
     */
    function goToManageTags(asset) {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        selectedDetail = null;
        detailFromBrowse = false;
        advancedInitialTab = "tags";
        advancedInitialTagName = asset?.name || "";
        advancedModalOpen = true;
    }

    /**
     * @param {string} parentName
     */
    function goToSubAsset(parentName, defaultChildName = "") {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        issueParent = parentName;
        issueType = "sub";
        issueName = defaultChildName;
        issueQty = "1";
        issueUnits = 0;
        issueReissue = true;
        issueIpfs = "";
        selectedDetail = null;
        detailFromBrowse = false;
        subModalOpen = true;
    }

    /**
     * @param {string} parentName
     */
    function goToNft(parentName) {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        issueParent = parentName;
        issueType = "nft";
        nftTag = "";
        issueIpfs = "";
        selectedDetail = null;
        detailFromBrowse = false;
        nftModalOpen = true;
    }

    /**
     * @param {AssetItem} asset
     */
    async function openGovernance(asset) {
        lastDetailAsset = selectedDetail;
        lastDetailTab = activeDetailTab;
        // We need full asset data (units, ipfs, etc) for governance
        try {
            const details = await core.invoke("get_asset_data", {
                name: asset.name,
            });
            selectedGovAsset = { ...asset, ...details }; // Merge list info with full details
            selectedDetail = null;
            detailFromBrowse = false;
            govModalOpen = true;
        } catch (e) {
            console.error("Failed to load details for governance", e);
        }
    }

    function openRewardsFromGovernance() {
        govModalOpen = false;
        selectedGovAsset = null;
        lastDetailAsset = null;
        lastDetailTab = "DETAILS";
        advancedInitialTab = "rewards";
        advancedInitialTagName = "";
        advancedModalOpen = true;
    }

    function returnToDetail() {
        const asset = lastDetailAsset;
        const tab = lastDetailTab;
        lastDetailAsset = null;
        lastDetailTab = "DETAILS";
        if (asset) openDetail(asset, tab);
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
        previewData = null;
        confirmOpen = true;
    }

    async function initiateIssue() {
        if (!issueName || !issueQty) {
            status = "Name and Qty required.";
            return;
        }
        if (issueType === "sub" && !issueParent) {
            status = "Parent asset required for sub-asset.";
            return;
        }

        let fullName = issueName.toUpperCase();
        if (issueType === "sub") {
            fullName = `${issueParent}/${issueName.toUpperCase()}`;
        }

        try { await ensureNodeSyncedForBroadcast(); } catch (e) { status = String(e); return; }
        status = "Building preview...";
        try {
            if (issueType === "sub") {
                previewData = await core.invoke("preview_issue_sub_asset", {
                    parent: issueParent,
                    name: issueName,
                    qty: String(issueQty),
                    reissuable: issueReissue,
                    units: Number(issueUnits),
                    ipfs: issueIpfs || "",
                });
                confirmType = "SUB-ASSET ISSUE";
            } else {
                previewData = await core.invoke("preview_issue_asset", {
                    name: fullName,
                    qty: String(issueQty),
                    units: Number(issueUnits),
                    reissuable: issueReissue,
                    ipfs: issueIpfs || "",
                });
                confirmType = "ROOT ASSET ISSUE";
            }
            status = "";
            const preview = previewData;
            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: issueType === "sub" ? "issue_sub" : "issue_root",
                        summary: preview ? preview.summary : "",
                        txid: null,
                        details: preview || {},
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            confirmPayload = null;
            confirmOpen = true;
        } catch (err) {
            status = `Preview failed: ${err}`;
            previewData = null;
        }
    }

    async function initiateReissue() {
        if (!reissueAsset || !reissueQty) {
            status = "Asset and Qty required.";
            return;
        }
        try { await ensureNodeSyncedForBroadcast(); } catch (e) { status = String(e); return; }
        status = "Building preview...";
        try {
            previewData = await core.invoke("preview_reissue_asset", {
                name: reissueAsset,
                qty: String(reissueQty),
                reissuable: reissueReissuable,
                newIpfs: reissueNewIpfs || "",
            });
            confirmType = "REISSUE";
            status = "";
            const preview = previewData;
            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "reissue",
                        summary: preview ? preview.summary : "",
                        txid: null,
                        details: preview || {},
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            confirmPayload = null;
            confirmOpen = true;
        } catch (err) {
            status = `Preview failed: ${err}`;
            previewData = null;
        }
    }

    async function initiateNft() {
        if (!issueParent) {
            status = "Parent asset required for NFT.";
            return;
        }
        if (!nftTag.trim()) {
            status = "NFT tag name required.";
            return;
        }
        try { await ensureNodeSyncedForBroadcast(); } catch (e) { status = String(e); return; }
        status = "Building preview...";
        try {
            previewData = await core.invoke("preview_issue_unique_asset", {
                rootName: issueParent,
                tags: [nftTag.trim()],
                ipfsHashes: issueIpfs ? [issueIpfs] : [],
            });
            confirmType = "NFT MINT";
            status = "";
            const preview = previewData;
            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "issue_unique",
                        summary: preview ? preview.summary : "",
                        txid: null,
                        details: preview || {},
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            confirmPayload = null;
            confirmOpen = true;
        } catch (err) {
            status = `Preview failed: ${err}`;
            previewData = null;
        }
    }

    async function confirmAction() {
        if (!tauriReady || isBroadcasting) return;
        isBroadcasting = true;
        try {
            await ensureNodeSyncedForBroadcast();
            let txid = "";
            if (confirmType === "TRANSFER") {
                if (!confirmPayload) return;
                const payload = confirmPayload;
                txid = await core.invoke("transfer_asset", {
                    asset: payload.asset,
                    amount: payload.amount,
                    to: payload.to,
                });
                status = `Sent! TXID: ${txid.slice(0, 16)}...`;
                transferTo = "";
                transferAmt = "";
            } else if (confirmType === "ROOT ASSET ISSUE" || confirmType === "SUB-ASSET ISSUE") {
                if (!previewData) return;
                const preview = previewData;
                txid = await core.invoke("issue_asset", {
                    name: preview.asset_name,
                    qty: String(preview.qty),
                    units: Number(preview.units || 0),
                    reissuable: preview.reissuable,
                    ipfs: preview.ipfs_hash || "",
                });
                status = `${preview.operation_type === "issue_sub" ? "Sub-asset" : "Asset"} created! TXID: ${txid.slice(0, 16)}...`;
                issueName = "";
                issueQty = "1";
                issueIpfs = "";
                if (issueType === "sub") issueParent = "";
            } else if (confirmType === "REISSUE") {
                if (!previewData) return;
                const preview = previewData;
                txid = await core
                    .invoke("reissue_asset", {
                        name: preview.asset_name,
                        qty: String(preview.qty),
                        toAddress: "",
                        changeAddress: "",
                        reissuable: preview.reissuable,
                        newUnits: null,
                        newIpfs: preview.ipfs_hash || "",
                    })
                    .catch((e) => {
                        throw "Reissue failed: " + e;
                    });
                status = `Reissued! TXID: ${txid.slice(0, 16)}...`;
                reissueNewIpfs = "";
            } else if (confirmType === "NFT MINT") {
                if (!previewData) return;
                const preview = previewData;
                const tags = (preview.tags || []).map(/** @param {string} t */ (t) => t.split("#")[1] || t);
                const ipfsHashes = preview.ipfs_hash
                    ? preview.ipfs_hash.split(", ").filter(/** @param {string} h */ (h) => h)
                    : [];
                txid = await core.invoke("issue_unique_asset", {
                    rootName: preview.asset_name,
                    tags,
                    ipfsHashes,
                });
                status = `NFT minted! TXID: ${txid.slice(0, 16)}...`;
                nftTag = "";
                issueIpfs = "";
                issueParent = "";
                issueType = "root";
            }
            confirmOpen = false;
            addNotification({
                type: "asset",
                severity: "success",
                title: "Asset Transaction Broadcasted",
                body: status,
                action: { label: "Copy TXID", txid },
            });
            if (previewJournalId) {
                try {
                    await core.invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid,
                        details: null,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }
            previewJournalId = null;
            previewData = null;
            refreshAssets();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                confirmOpen = false;
                requestWalletUnlock();
                return;
            }
            status = "Error: " + err;
            addNotification({
                type: "asset",
                severity: "error",
                title: "Asset Transaction Failed",
                body: String(err),
            });
            if (previewJournalId) {
                try {
                    await core.invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(err) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
            }
            confirmOpen = false;
        } finally {
            isBroadcasting = false;
        }
    }

    function cancelConfirm() {
        if (previewJournalId) {
            core.invoke("update_tx_journal_entry", {
                id: previewJournalId,
                status: "Abandoned",
                txid: null,
                details: { reason: "user_cancelled" },
            }).catch((e) => console.warn("Failed to mark journal entry as abandoned:", e));
        }
        confirmOpen = false;
        previewData = null;
        previewJournalId = null;
    }

    function isWalletUnlockError(err) {
        const text = String(err || "");
        const lower = text.toLowerCase();
        return text.includes("ERROR CODE: -13")
            || lower.includes("walletpassphrase")
            || lower.includes("wallet passphrase")
            || lower.includes("please enter the wallet passphrase")
            || /wallet.*locked|passphrase|unlock/i.test(text);
    }

    function requestWalletUnlock() {
        unlockPassword = "";
        unlockPin = "";
        unlockError = "";
        walletUnlockMode = "passphrase";
        walletPinStatus = null;
        showUnlockModal = true;
        loadWalletPinStatus().then((status) => {
            if (showUnlockModal) {
                walletPinStatus = status;
                if (walletUnlockMode === "passphrase") {
                    walletUnlockMode = defaultUnlockMode(status);
                }
            }
        });
    }

    function switchWalletUnlockToPassphrase() {
        walletUnlockMode = "passphrase";
        unlockPin = "";
        unlockError = "";
    }

    function switchWalletUnlockToPin() {
        if (!walletPinUsable) return;
        walletUnlockMode = "pin";
        unlockPin = "";
        unlockError = "";
    }

    async function forgotWalletPinUnlock() {
        if (unlocking) return;
        unlocking = true;
        unlockError = "";
        try {
            await forgotWalletPin();
            walletPinStatus = await loadWalletPinStatus();
            walletUnlockMode = "passphrase";
            unlockPin = "";
            unlockError = "Device PIN cleared. Enter your wallet passphrase to unlock.";
        } catch (err) {
            unlockError = "Could not clear PIN: " + String(err);
        } finally {
            unlocking = false;
        }
    }

    async function unlockAndRetry() {
        if (unlocking) return;
        if (walletUnlockMode === "pin" && walletPinUsable) {
            if (!isValidPin(unlockPin)) {
                unlockError = "Enter your 6-digit PIN.";
                return;
            }
            unlocking = true;
            unlockError = "";
            try {
                await unlockWalletWithPin(unlockPin, 300);
                unlockPin = "";
                showUnlockModal = false;
                confirmOpen = true;
                isBroadcasting = false;
                await confirmAction();
            } catch (err) {
                unlockPin = "";
                unlockError = String(err);
                walletPinStatus = await loadWalletPinStatus();
                if (pinRequiresPassphrase(walletPinStatus)) {
                    walletUnlockMode = "passphrase";
                }
            } finally {
                unlocking = false;
            }
            return;
        }
        if (!unlockPassword.trim() || unlocking) return;
        unlocking = true;
        unlockError = "";
        try {
            await unlockRuntimeWalletWithPassphrase(unlockPassword, 300);
            unlockPassword = "";
            showUnlockModal = false;
            confirmOpen = true;
            isBroadcasting = false;
            await confirmAction();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                unlockError = "Wallet unlock failed. Check the passphrase and try again.";
            } else {
                unlockError = String(err);
            }
        } finally {
            unlocking = false;
        }
    }
</script>

<div class="view-assets">
    <div class="cyber-panel main-frame">
        <!-- HEADER -->
        <header class="panel-header">
            <div class="header-left">
                <button
                    type="button"
                    class="header-title"
                    class:active={advancedModalOpen || createModalOpen || browseModalOpen || !!selectedDetail || transferModalOpen || reissueModalOpen || subModalOpen || nftModalOpen || govModalOpen}
                    on:click={() => { advancedModalOpen = false; createModalOpen = false; browseModalOpen = false; selectedDetail = null; transferModalOpen = false; reissueModalOpen = false; subModalOpen = false; nftModalOpen = false; govModalOpen = false; }}
                >
                    ◈ MY ASSETS
                </button>
            </div>
            <div class="header-options">
                <div class="type-filter" class:disabled={modalBlocking} role="group" aria-label="Filter by asset type">
                    {#each [
                        { id: "ALL", label: "ALL" },
                        { id: "TOKEN", label: "TOKENS" },
                        { id: "SUB", label: "SUB" },
                        { id: "NFT", label: "NFT" },
                        { id: "QUALIFIER", label: "QUALIF" },
                        { id: "RESTRICTED", label: "RESTRIC" },
                    ] as opt}
                        <button
                            type="button"
                            class="type-chip"
                            class:active={typeFilter === opt.id}
                            class:qualifier={opt.id === "QUALIFIER"}
                            disabled={modalBlocking}
                            on:click={() => (typeFilter = opt.id)}
                            title={`Show ${opt.label === "QUALIF" ? "Qualifier" : opt.label === "RESTRIC" ? "Restricted" : opt.label} assets`}
                        >
                            {opt.label}
                        </button>
                    {/each}
                </div>
                <Tooltip
                    text={showHidden ? "Hide hidden" : "Show hidden assets"}
                >
                    <label
                        class="toggle-hidden"
                        class:disabled={advancedModalOpen || createModalOpen || browseModalOpen || !!selectedDetail || transferModalOpen || reissueModalOpen || subModalOpen || nftModalOpen || govModalOpen}
                    >
                        <input
                            type="checkbox"
                            bind:checked={showHidden}
                            disabled={advancedModalOpen || createModalOpen || browseModalOpen || !!selectedDetail || transferModalOpen || reissueModalOpen || subModalOpen || nftModalOpen || govModalOpen}
                        />
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
                    class="header-btn refresh-btn"
                    on:click={refreshAssets}
                    disabled={!nodeOnline || advancedModalOpen || createModalOpen || browseModalOpen || !!selectedDetail || transferModalOpen || reissueModalOpen || subModalOpen || nftModalOpen || govModalOpen}
                    title="Refresh Asset List"
                >
                    <span class="btn-icon">↻</span> REFRESH
                </button>
                <button
                    class="header-btn create-btn"
                    class:active={createModalOpen}
                    on:click={() => { createModalOpen = !createModalOpen; advancedModalOpen = false; browseModalOpen = false; }}
                    disabled={!nodeOnline}
                    title="Create New Root Asset"
                >
                    <span class="btn-icon">+</span> CREATE
                </button>
                <button
                    class="header-btn browse-btn"
                    class:active={browseModalOpen}
                    on:click={() => { browseModalOpen = !browseModalOpen; advancedModalOpen = false; createModalOpen = false; }}
                    disabled={!nodeOnline}
                    title="Browse Network Assets"
                >
                    <span class="btn-icon">🔍</span> BROWSE
                </button>
                <button
                    class="header-btn advanced-btn"
                    class:active={advancedModalOpen}
                    on:click={() => {
                        advancedModalOpen = !advancedModalOpen;
                        createModalOpen = false;
                        browseModalOpen = false;
                        if (advancedModalOpen) {
                            advancedInitialTab = "";
                            advancedInitialTagName = "";
                        }
                    }}
                    disabled={!nodeOnline}
                    title="Advanced Asset Controls"
                >
                    <span class="btn-icon">⚙</span> ADVANCED
                </button>
            </div>
        </header>

        <!-- CONTENT -->
        <div class="content-area">
            <div class="tab-content" class:panel-open={advancedModalOpen || createModalOpen || browseModalOpen || !!selectedDetail || transferModalOpen || reissueModalOpen || subModalOpen || nftModalOpen || govModalOpen}>
                {#if advancedModalOpen}
                    <ModalAssetAdvanced
                        inline
                        isOpen={true}
                        {nodeOnline}
                        assets={groupedAssets}
                        initialTab={advancedInitialTab}
                        initialTagName={advancedInitialTagName}
                        on:close={() => (advancedModalOpen = false)}
                    />
                {:else if createModalOpen}
                    <ModalIssueAsset
                        inline
                        isOpen={true}
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
                {:else if browseModalOpen}
                    <ModalBrowse
                        inline
                        isOpen={true}
                        on:close={() => (browseModalOpen = false)}
                        on:viewAsset={(e) => {
                            const a = e.detail;
                            detailFromBrowse = true;
                            openDetail({
                                name: a.name,
                                balance: a.balance,
                                type: a.type,
                                units: a.units,
                            });
                        }}
                    />
                {:else if selectedDetail}
                    <ModalAssetDetail
                        inline
                        asset={selectedDetail}
                        metadata={assetMetadata}
                        loading={metadataLoading}
                        {slideDirection}
                        initialActiveTab={initialDetailTab}
                        hasMultipleAssets={groupedAssets.length > 1}
                        on:close={closeDetail}
                        on:prev={navigatePrev}
                        on:next={navigateNext}
                        on:transfer={(e) => goToTransfer(e.detail.name)}
                        on:reissue={(e) => goToReissue(e.detail)}
                        on:createSub={(e) => goToSubAsset(e.detail.name)}
                        on:createH0xC={(e) => goToSubAsset(e.detail.name, "H0XC")}
                        on:createNft={(e) => goToNft(e.detail.name)}
                        on:gov={(e) => openGovernance(e.detail)}
                        on:manageTags={(e) => goToManageTags(e.detail)}
                        on:tabChange={(e) => (activeDetailTab = e.detail)}
                    />
                {:else if transferModalOpen}
                    <ModalTransfer
                        inline
                        isOpen={true}
                        {nodeOnline}
                        assets={groupedAssets}
                        showBack={lastDetailAsset !== null}
                        bind:selectedAsset
                        bind:toAddress={transferTo}
                        bind:amount={transferAmt}
                        on:close={() => (transferModalOpen = false)}
                        on:back={() => { transferModalOpen = false; returnToDetail(); }}
                        on:transfer={() => {
                            initiateTransfer();
                            transferModalOpen = false;
                        }}
                    />
                {:else if reissueModalOpen}
                    <ModalReissue
                        inline
                        isOpen={true}
                        {nodeOnline}
                        assets={groupedAssets.filter((a) => a.hasOwner)}
                        currentIpfs={assetMetadata?.ipfs_hash || ""}
                        currentSupply={assetMetadata?.amount || 0}
                        showBack={lastDetailAsset !== null}
                        bind:name={reissueAsset}
                        bind:qty={reissueQty}
                        bind:newIpfs={reissueNewIpfs}
                        bind:reissuable={reissueReissuable}
                        on:close={() => (reissueModalOpen = false)}
                        on:back={() => { reissueModalOpen = false; returnToDetail(); }}
                        on:reissue={() => {
                            initiateReissue();
                            reissueModalOpen = false;
                        }}
                    />
                {:else if subModalOpen}
                    <ModalIssueSub
                        inline
                        isOpen={true}
                        {nodeOnline}
                        parentName={issueParent}
                        showBack={lastDetailAsset !== null}
                        bind:name={issueName}
                        bind:qty={issueQty}
                        bind:units={issueUnits}
                        bind:ipfs={issueIpfs}
                        bind:reissuable={issueReissue}
                        on:close={() => { subModalOpen = false; if (lastDetailAsset) returnToDetail(); }}
                        on:back={() => { subModalOpen = false; returnToDetail(); }}
                        on:create={() => {
                            initiateIssue();
                            subModalOpen = false;
                        }}
                    />
                {:else if nftModalOpen}
                    <ModalIssueNFT
                        inline
                        isOpen={true}
                        {nodeOnline}
                        parentName={issueParent}
                        showBack={lastDetailAsset !== null}
                        bind:tag={nftTag}
                        bind:ipfs={issueIpfs}
                        on:close={() => (nftModalOpen = false)}
                        on:back={() => { nftModalOpen = false; returnToDetail(); }}
                        on:create={() => {
                            initiateNft();
                            nftModalOpen = false;
                        }}
                    />
                {:else if govModalOpen}
                    <ModalAssetGovernance
                        inline
                        isOpen={true}
                        asset={selectedGovAsset}
                        showBack={lastDetailAsset !== null}
                        on:close={() => { govModalOpen = false; if (lastDetailAsset) returnToDetail(); }}
                        on:back={() => { govModalOpen = false; returnToDetail(); }}
                        on:openRewards={openRewardsFromGovernance}
                    />
                {:else}
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
                                    title={asset.name}
                                >
                                    ↳
                                </div>
                            {/if}
                            <div class="card-content">
                                {#if asset.isSubAsset}
                                    <div class="asset-name" title={asset.name}>
                                        {asset.name.split("/").pop()}
                                    </div>
                                    <div class="asset-parent-path" title={asset.name}>
                                        {asset.name.split("/").slice(0, -1).join(" / ")}
                                    </div>
                                {:else}
                                    <div class="asset-name">
                                        {asset.name}
                                    </div>
                                {/if}
                                <div class="asset-balance">
                                    {formatBalance(asset.balance)}
                                </div>
                                <div class="asset-meta">
                                    <span class="asset-type" class:qualifier={asset.isQualifier}>{assetTypeLabel(asset)}</span>
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
                {/if}
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

    <!-- ═══════════════ CONFIRM MODAL ═══════════════ -->
    <!-- ═══════════════ CONFIRM MODAL ═══════════════ -->
    <ModalConfirm
        isOpen={confirmOpen}
        type={confirmType}
        payload={confirmPayload || {}}
        previewData={previewData || null}
        {isBroadcasting}
        on:close={cancelConfirm}
        on:confirm={confirmAction}
    />

    <WalletUnlockModal
        show={showUnlockModal}
        mode={walletUnlockMode}
        bind:password={unlockPassword}
        bind:pin={unlockPin}
        {unlocking}
        error={unlockError}
        title={walletUnlockMode === "pin" && walletPinUsable ? "UNLOCK WITH DEVICE PIN" : "UNLOCK WALLET"}
        body={walletUnlockMode === "pin" && walletPinUsable ? "Enter this device's 6-digit PIN to unlock the local Core wallet for signing." : "Your wallet is locked. Commander will unlock it for 5 minutes to broadcast this asset transaction."}
        confirmLabel={walletUnlockMode === "pin" && walletPinUsable ? "UNLOCK" : "UNLOCK AND BROADCAST"}
        pinConfigured={walletPinUsable}
        pinRequiresPassphrase={walletUnlockMode === "pin" && pinRequiresPassphrase(walletPinStatus)}
        pinRequiresPassphraseReason={walletPinStatus?.reason || ""}
        lockoutRemainingSecs={walletPinStatus?.lockout_remaining_secs || 0}
        on:cancel={() => { showUnlockModal = false; unlockPassword = ""; unlockPin = ""; unlockError = ""; }}
        on:confirm={unlockAndRetry}
        on:usepassphrase={switchWalletUnlockToPassphrase}
        on:usepin={switchWalletUnlockToPin}
        on:forgotpin={forgotWalletPinUnlock}
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
            rgba(4, 6, 5, 0.95) 0%,
            rgba(2, 4, 3, 0.98) 100%
        );
        border: 1px solid rgba(0, 255, 65, 0.14);
        border-radius: 8px;
        overflow: hidden;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }

    /* ═══════════════ HEADER / NAV ═══════════════ */
    .panel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.75rem;
        padding: 5px 1rem; /* Added vertical padding for button glows/borders */
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }

    .header-left {
        display: flex;
        align-items: center;
        flex: 0 0 auto;
    }
    .header-options {
        flex: 1 1 auto;
        min-width: 0;
        margin-left: 0;
        display: flex;
        align-items: center;
        gap: 0.4rem;
        flex-wrap: nowrap;
        overflow: hidden;
    }
    .type-filter {
        display: flex;
        flex: 0 1 auto;
        min-width: 0;
        gap: 4px;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        padding: 3px;
        overflow-x: auto;
        scrollbar-width: none;
        white-space: nowrap;
    }
    .type-filter::-webkit-scrollbar {
        display: none;
    }
    .type-chip {
        flex: 0 0 auto;
        background: transparent;
        border: 1px solid transparent;
        color: #666;
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        padding: 4px 8px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .type-chip:hover {
        color: #aaa;
        background: rgba(255, 255, 255, 0.04);
    }
    .type-chip.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .type-chip.active.qualifier {
        background: rgba(170, 130, 255, 0.12);
        border-color: rgba(170, 130, 255, 0.4);
        color: #c2a8ff;
    }
    .type-filter.disabled {
        opacity: 0.4;
        pointer-events: none;
    }
    .type-chip:disabled {
        cursor: not-allowed;
    }
    .toggle-hidden {
        flex: 0 0 48px;
        width: 48px;
        height: 32px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(255, 255, 255, 0.05);
        padding: 0;
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        transition: all 0.2s;
    }
    .toggle-hidden:hover {
        background: rgba(255, 255, 255, 0.1);
    }
    .toggle-hidden.disabled {
        cursor: not-allowed;
        opacity: 0.4;
        pointer-events: none;
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
        background: transparent;
        border: 0;
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 2px;
        cursor: default;
        user-select: none;
        transition: all 0.2s;
        padding: 0;
    }
    .header-title.active {
        cursor: pointer;
        text-shadow: 0 0 14px rgba(0, 255, 65, 0.5);
        border-bottom: 1px solid rgba(0, 255, 65, 0.4);
        padding-bottom: 1px;
    }
    .header-title.active:hover {
        color: #fff;
        text-shadow: 0 0 18px rgba(0, 255, 65, 0.7);
    }
    .header-actions {
        flex: 0 0 auto;
        display: flex;
        gap: 0.5rem;
        margin-left: 0;
        margin-right: 1rem;
        flex-wrap: nowrap;
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
    .header-btn.active {
        background: rgba(0, 255, 65, 0.2);
        border-color: var(--color-primary);
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.25);
    }
    .header-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .btn-icon {
        font-size: 0.9rem;
    }
    @media (max-width: 1100px) {
        .panel-header {
            gap: 0.5rem;
            padding-inline: 0.75rem;
        }
        .header-title {
            max-width: 7rem;
            line-height: 1.2;
        }
        .type-filter {
            gap: 2px;
            padding: 2px;
        }
        .type-chip {
            padding: 4px 6px;
            font-size: 0.52rem;
        }
        .toggle-hidden {
            flex-basis: 44px;
            width: 44px;
        }
        .header-actions {
            gap: 0.4rem;
            margin-right: 0.5rem;
        }
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
    .tab-content.panel-open {
        overflow: hidden;
        display: flex;
        flex-direction: column;
        padding: 0;
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
        display: flex;
        flex-direction: column;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
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
        display: flex;
        flex-direction: column;
        height: 100%;
        min-height: 100%;
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
    .asset-parent-path {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 0.5px;
        text-overflow: ellipsis;
        overflow: hidden;
        white-space: nowrap;
        margin-bottom: 0.35rem;
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
        margin-top: auto;
        padding-top: 0.4rem;
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
    .asset-type.qualifier {
        border-color: rgba(170, 130, 255, 0.35);
        color: #c2a8ff;
        background: rgba(170, 130, 255, 0.08);
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
