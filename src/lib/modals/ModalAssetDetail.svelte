<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher, tick } from "svelte";
    import { core } from "@tauri-apps/api";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { formatBalance } from "../utils.js";
    import "../../components.css";
    import Tooltip from "../ui/Tooltip.svelte";
    import IpfsReference from "../ui/IpfsReference.svelte";
    import ModalAlert from "./ModalAlert.svelte";
    import { cidViewerTarget } from "../stores/contentLibrary.js";
    import AssetDetailDetailsTab from "../ui/asset-detail/AssetDetailDetailsTab.svelte";
    import AssetMessageCompose from "../ui/asset-messages/AssetMessageCompose.svelte";

    const dispatch = createEventDispatcher();

    /**
     * @typedef {{
     *   name: string;
     *   balance?: number|string;
     *   units?: number;
     *   type?: string;
     *   isSubAsset?: boolean;
     *   hasOwner?: boolean;
     * }} AssetDetail
     * @typedef {{
     *   amount?: number;
     *   units?: number;
     *   reissuable?: boolean;
     *   block_height?: number;
     *   has_ipfs?: boolean;
     *   ipfs_hash?: string;
     * }} AssetMetadata
     * @typedef {{ enabled?: boolean, warnings?: string[] }} MessagesInfo
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     * }} AssetMessage
     * @typedef {{
     *   is_short_message?: boolean;
     *   text?: string;
     *   warnings?: string[];
     * }} ShortMessageDecodeResult
     */

    /** @type {AssetDetail | null} */
    export let asset = null;
    /** @type {AssetMetadata | null} */
    export let metadata = null;
    export let loading = false;
    export let slideDirection = 0;
    export let hasMultipleAssets = false;
    export let inline = false;
    export let initialActiveTab = "DETAILS";

    let showAlert = false;

    let activeTab = initialActiveTab;

    /** @type {MessagesInfo | null} */
    let messagesInfo = null;
    /** @type {AssetMessage[]} */
    let messages = [];
    /** @type {string[]} */
    let channels = [];
    let messagesLoading = false;
    let messagesError = "";
    let isSubscribed = false;

    let composeOpen = false;
    let currentAssetName = "";

    /**
     * @typedef {{
     *   name: string,
     *   version: string,
     *   fingerprint_sha256: string,
     *   origin?: "builtin"|"custom",
     *   active?: boolean,
     *   builtin?: boolean,
     *   path?: string|null
     * }} TablePackSummary
     * @typedef {{
     *   active?: TablePackSummary,
     *   built_in?: TablePackSummary,
     *   packs_dir?: string,
     *   selection_path?: string
     * }} TablePackStatus
     */
    /** @type {TablePackStatus | null} */
    let activeTablePack = null;
    /** @type {TablePackSummary[]} */
    let tablePacks = [];
    let tablePackBusy = false;
    let tablePackError = "";
    let tablePackStatus = "";
    let tablePackSelectionFingerprint = "";
    let preparedTablePackFingerprint = "";
    /** @type {ReturnType<typeof setTimeout> | null} */
    let tablePackStatusTimer = null;

    /** @param {TablePackSummary | null | undefined} pack */
    function tablePackLabel(pack) {
        if (!pack) return "Official HOXSHTV1.0";
        if (pack.builtin || pack.origin !== "custom") return pack.name;
        const version = pack.version && pack.version !== pack.name ? ` v${pack.version}` : "";
        return `Custom - ${pack.name}${version}`;
    }

    function displayedTablePack() {
        if (tablePackSelectionFingerprint) {
            const selected = tablePacks.find((pack) => pack.fingerprint_sha256 === tablePackSelectionFingerprint);
            if (selected) return selected;
        }
        return activeTablePack?.active || null;
    }

    function activePackLabel() {
        const pack = displayedTablePack();
        return tablePackLabel(pack);
    }

    function activePackFingerprint() {
        return displayedTablePack()?.fingerprint_sha256 || "";
    }

    function activePackFingerprintShort() {
        const fp = activePackFingerprint();
        return fp ? `${fp.slice(0, 12)}...${fp.slice(-8)}` : "";
    }

    function activePackStatusTitle() {
        const pack = activeTablePack?.active;
        const displayed = displayedTablePack();
        const fp = activePackFingerprint();
        const suffix = fp ? `\nFingerprint: ${fp}` : "";
        if (pack && displayed && pack.fingerprint_sha256 !== displayed.fingerprint_sha256) {
            return `Selected: ${tablePackLabel(displayed)}${suffix}\nActive status is refreshing.`;
        }
        return `${activePackLabel()}${suffix}`;
    }

    function selectedTablePack() {
        const fp = tablePackSelectionFingerprint || activeTablePack?.active?.fingerprint_sha256 || "";
        return tablePacks.find((pack) => pack.fingerprint_sha256 === fp) || null;
    }

    /** @param {TablePackSummary} pack */
    function setActivePackSummary(pack) {
        activeTablePack = {
            ...(activeTablePack || {}),
            active: pack,
        };
        tablePackSelectionFingerprint = pack.fingerprint_sha256;
        preparedTablePackFingerprint = "";
        tablePacks = tablePacks.map((item) => ({
            ...item,
            active: item.fingerprint_sha256 === pack.fingerprint_sha256,
        }));
    }

    function waitForPaint() {
        return new Promise((resolve) => {
            requestAnimationFrame(() => {
                requestAnimationFrame(resolve);
            });
        });
    }

    /** @param {string} message */
    async function showTablePackBusy(message) {
        if (tablePackStatusTimer) {
            clearTimeout(tablePackStatusTimer);
            tablePackStatusTimer = null;
        }
        tablePackBusy = true;
        tablePackError = "";
        tablePackStatus = message;
        await tick();
        await waitForPaint();
    }

    /** @param {string} message */
    function finishTablePackBusy(message = "") {
        tablePackBusy = false;
        tablePackStatus = message;
        if (tablePackStatusTimer) clearTimeout(tablePackStatusTimer);
        if (message) {
            tablePackStatusTimer = setTimeout(() => {
                tablePackStatus = "";
                tablePackStatusTimer = null;
            }, 1400);
        }
    }

    async function prepareShortMessagePack() {
        tablePackStatus = "Preparing text prediction...";
        await tick();
        await waitForPaint();
        try {
            await core.invoke("short_message_prepare_active_table_pack");
            preparedTablePackFingerprint = activePackFingerprint();
        } catch (err) {
            // The next encode request will surface any real issue.
        }
    }

    /** @param {unknown} err */
    function messageRpcError(err) {
        const text = String(err);
        if (text.includes("-32601") || /method not found/i.test(text)) {
            return "This Core build does not expose this asset messaging RPC. Update or start a Core build with messaging support, then refresh Commander and try again.";
        }
        if (/connection refused/i.test(text) || /could not connect/i.test(text)) {
            return "Core daemon is not running or RPC is unavailable. Start Core, then refresh Commander.";
        }
        if (/wallet.*locked|passphrase|unlock/i.test(text)) {
            return "Wallet is locked. Unlock the wallet before sending messages.";
        }
        if (/channel authority|does not.*own/i.test(text)) {
            return "Wallet does not hold the channel authority asset. Own the channel asset to send messages on this channel.";
        }
        if (/invalid.*channel/i.test(text)) {
            return text;
        }
        if (/invalid.*hash|invalid.*cid|invalid.*payload/i.test(text)) {
            return text;
        }
        return text;
    }

    /** @param {unknown} err */
    function isWalletUnlockError(err) {
        const text = String(err || "");
        const lower = text.toLowerCase();
        return text.includes("ERROR CODE: -13")
            || lower.includes("walletpassphrase")
            || lower.includes("wallet passphrase")
            || lower.includes("please enter the wallet passphrase")
            || /wallet.*locked|passphrase|unlock/i.test(text);
    }

    async function refreshTablePacks() {
        try {
            const [status, packs] = await Promise.all([
                core.invoke("short_message_get_active_table_pack"),
                core.invoke("short_message_list_table_packs"),
            ]);
            activeTablePack = status;
            tablePacks = Array.isArray(packs) ? packs : [];
            tablePackSelectionFingerprint = status?.active?.fingerprint_sha256 || "";
            tablePackError = "";
            tablePackStatus = "";
        } catch (err) {
            tablePackError = String(err);
            tablePackStatus = "";
        }
    }

    async function exportOfficialTablePack() {
        tablePackBusy = true;
        tablePackError = "";
        try {
            const targetPath = await save({
                title: "Export Official Short Message Table Pack",
                defaultPath: "HOXSHTV1.0-table-pack.json",
                filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (!targetPath) return;
            await core.invoke("short_message_export_built_in_table_pack", { targetPath });
            await refreshTablePacks();
        } catch (err) {
            tablePackError = String(err);
        } finally {
            tablePackBusy = false;
        }
    }

    async function importTablePack() {
        try {
            const sourcePath = await open({
                title: "Import Short Message Table Pack",
                multiple: false,
                filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (!sourcePath || Array.isArray(sourcePath)) return;
            await showTablePackBusy("Importing table pack...");
            await core.invoke("short_message_import_table_pack", { sourcePath });
            await refreshTablePacks();
            finishTablePackBusy("Table pack imported.");
        } catch (err) {
            tablePackError = String(err);
            finishTablePackBusy();
        }
    }

    async function resetTablePack() {
        await showTablePackBusy("Loading official table pack...");
        try {
            const selected = await core.invoke("short_message_reset_table_pack");
            if (selected && typeof selected === "object") {
                setActivePackSummary(/** @type {TablePackSummary} */ (selected));
            }
            await prepareShortMessagePack();
            finishTablePackBusy("Official table pack loaded.");
        } catch (err) {
            tablePackError = String(err);
            finishTablePackBusy();
        }
    }

    async function openCompose() {
        composeOpen = true;
    }

    // Short message decode cache for received messages
    /** @type {Record<string, ShortMessageDecodeResult | undefined>} */
    let shortMessageCache = {};
    /** @type {Set<string>} */
    let shortMessagePending = new Set();

    // Message inbox UI state
    /** @type {AssetMessage | null} */
    let selectedMessage = null;
    let messageExplorerMode = false;
    /** @type {Set<string>} */
    let pinnedMessageIds = new Set();
    /** @type {Set<string>} */
    let hiddenMessageIds = new Set();
    /** @type {Set<string>} */
    let readMessageIds = new Set();
    let inboxTablePackPanelOpen = false;
    let inboxTablePackBusy = false;
    let inboxTablePackStatus = "";
    let inboxTablePackError = "";

    /** @type {Set<string>} */
    let selectedMessageIds = new Set();

    /** @type {null | 'clearUnpinned' | 'deleteSelected'} */
    let pendingConfirmAction = null;

    const PINNED_MESSAGES_KEY = "hemp0x_pinnedMessageIds";
    const HIDDEN_MESSAGES_KEY = "hemp0x_hiddenMessageIds";
    const READ_MESSAGES_KEY = "hemp0x_readMessageIds";

    /** @param {string} key */
    function loadMessageIdSet(key) {
        try {
            const raw = localStorage.getItem(key);
            if (raw) {
                const parsed = JSON.parse(raw);
                if (Array.isArray(parsed)) {
                    return new Set(parsed.filter((item) => typeof item === "string"));
                }
            }
        } catch {
            // Ignore corrupt local state and start clean.
        }
        return new Set();
    }

    function loadMessageLocalState() {
        pinnedMessageIds = loadMessageIdSet(PINNED_MESSAGES_KEY);
        hiddenMessageIds = loadMessageIdSet(HIDDEN_MESSAGES_KEY);
        readMessageIds = loadMessageIdSet(READ_MESSAGES_KEY);
    }
    loadMessageLocalState();

    /**
     * @param {string} key
     * @param {Set<string>} ids
     */
    function saveMessageIdSet(key, ids) {
        try {
            localStorage.setItem(key, JSON.stringify(Array.from(ids)));
        } catch {
            // Local cleanup state is best effort.
        }
    }

    /** @param {AssetMessage} msg */
    function messageLocalId(msg) {
        return `${msg.asset_name}|${msg.time}|${msg.message.slice(0, 24)}`;
    }

    /** @param {AssetMessage} msg */
    function messagePinId(msg) {
        return messageLocalId(msg);
    }

    /** @param {AssetMessage} msg */
    function isPinned(msg) {
        return pinnedMessageIds.has(messagePinId(msg));
    }

    /** @param {AssetMessage} msg */
    function isHidden(msg) {
        return hiddenMessageIds.has(messageLocalId(msg));
    }

    /** @param {AssetMessage} msg */
    function displayMessageStatus(msg) {
        if (readMessageIds.has(messageLocalId(msg))) return "READ";
        return msg.status || "";
    }

    /** @param {AssetMessage} msg */
    function togglePin(msg) {
        const id = messagePinId(msg);
        if (pinnedMessageIds.has(id)) {
            pinnedMessageIds.delete(id);
        } else {
            pinnedMessageIds.add(id);
        }
        pinnedMessageIds = pinnedMessageIds;
        saveMessageIdSet(PINNED_MESSAGES_KEY, pinnedMessageIds);
    }

    function closeMessageDetail() {
        selectedMessage = null;
    }

    /** @param {AssetMessage} msg */
    function markMessageRead(msg) {
        const id = messageLocalId(msg);
        readMessageIds.add(id);
        readMessageIds = readMessageIds;
        saveMessageIdSet(READ_MESSAGES_KEY, readMessageIds);
        messages = messages.map((item) =>
            messageLocalId(item) === id ? { ...item, status: "READ" } : item
        );
        selectedMessage = selectedMessage && messageLocalId(selectedMessage) === id
            ? { ...selectedMessage, status: "READ" }
            : selectedMessage;
    }

    /** @param {AssetMessage} msg */
    function markMessageUnread(msg) {
        readMessageIds.delete(messageLocalId(msg));
        readMessageIds = readMessageIds;
        saveMessageIdSet(READ_MESSAGES_KEY, readMessageIds);
        messages = messages.map((item) =>
            messageLocalId(item) === messageLocalId(msg) ? { ...item, status: "UNREAD" } : item
        );
        selectedMessage = selectedMessage && messageLocalId(selectedMessage) === messageLocalId(msg)
            ? { ...selectedMessage, status: "UNREAD" }
            : selectedMessage;
    }

    /** @param {AssetMessage} msg */
    function msgSelectId(msg) {
        return messageLocalId(msg);
    }

    /** @param {AssetMessage} msg */
    function isSelected(msg) {
        return selectedMessageIds.has(msgSelectId(msg));
    }

    /** @param {AssetMessage} msg */
    function toggleSelect(msg) {
        const id = msgSelectId(msg);
        if (selectedMessageIds.has(id)) {
            selectedMessageIds.delete(id);
        } else {
            selectedMessageIds.add(id);
        }
        selectedMessageIds = selectedMessageIds;
    }

    function selectAllDisplayed() {
        displayedMessages.forEach((msg) => selectedMessageIds.add(msgSelectId(msg)));
        selectedMessageIds = selectedMessageIds;
    }

    function clearSelection() {
        selectedMessageIds = new Set();
    }

    function restoreHiddenMessages() {
        hiddenMessageIds = new Set();
        saveMessageIdSet(HIDDEN_MESSAGES_KEY, hiddenMessageIds);
    }

    /** @param {Set<string>} ids */
    function hideMessageIds(ids) {
        ids.forEach((id) => hiddenMessageIds.add(id));
        ids.forEach((id) => pinnedMessageIds.delete(id));
        hiddenMessageIds = hiddenMessageIds;
        pinnedMessageIds = pinnedMessageIds;
        saveMessageIdSet(HIDDEN_MESSAGES_KEY, hiddenMessageIds);
        saveMessageIdSet(PINNED_MESSAGES_KEY, pinnedMessageIds);
        selectedMessageIds = new Set();
        if (selectedMessage && ids.has(messageLocalId(selectedMessage))) {
            selectedMessage = null;
        }
    }

    function markSelectedUnread() {
        const ids = new Set(selectedMessageIds);
        ids.forEach((id) => readMessageIds.delete(id));
        readMessageIds = readMessageIds;
        saveMessageIdSet(READ_MESSAGES_KEY, readMessageIds);
        messages = messages.map((msg) => {
            if (ids.has(msgSelectId(msg))) {
                return { ...msg, status: 'UNREAD' };
            }
            return msg;
        });
        selectedMessageIds = new Set();
    }

    /** @param {AssetMessage} msg */
    function openMessageDetail(msg) {
        selectedMessage = msg;
        markMessageRead(msg);
    }

    function requestClearUnpinned() {
        pendingConfirmAction = 'clearUnpinned';
    }

    function requestDeleteSelected() {
        pendingConfirmAction = 'deleteSelected';
    }

    function cancelConfirmAction() {
        pendingConfirmAction = null;
    }

    function executeConfirmedAction() {
        if (pendingConfirmAction === 'clearUnpinned') {
            hideMessageIds(new Set(displayedMessages.filter((msg) => !isPinned(msg)).map(messageLocalId)));
        } else if (pendingConfirmAction === 'deleteSelected') {
            hideMessageIds(new Set(selectedMessageIds));
        }
        shortMessageCache = {};
        shortMessagePending.clear();
        pendingConfirmAction = null;
    }

    /** @param {string|number|null|undefined} isoLike */
    function formatMessageTime(isoLike) {
        if (!isoLike) return "";
        let d = new Date(isoLike);
        if (isNaN(d.getTime())) {
            const n = Number(isoLike);
            if (!Number.isNaN(n) && n > 1000000000) {
                d = new Date(n * 1000);
            }
        }
        if (isNaN(d.getTime())) return isoLike;
        return d.toLocaleString(undefined, {
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
        });
    }

    /** @param {string|number|null|undefined} isoLike */
    function formatExpireTime(isoLike) {
        if (!isoLike) return "";
        let d = new Date(isoLike);
        if (isNaN(d.getTime())) {
            const n = Number(isoLike);
            if (!Number.isNaN(n) && n > 1000000000) {
                d = new Date(n * 1000);
            }
        }
        if (isNaN(d.getTime())) return isoLike;
        const now = Date.now();
        const diff = d.getTime() - now;
        if (diff <= 0) return "Expired";
        const mins = Math.floor(diff / 60000);
        if (mins < 60) return `${mins}m`;
        const hrs = Math.floor(mins / 60);
        if (hrs < 24) return `${hrs}h ${mins % 60}m`;
        const days = Math.floor(hrs / 24);
        return `${days}d ${hrs % 24}h`;
    }

    function refreshMessages() {
        loadMessages();
    }

    function toggleExplorer() {
        messageExplorerMode = !messageExplorerMode;
    }

    $: displayedMessages = (() => {
        const hidden = hiddenMessageIds;
        return (messageExplorerMode ? messages : filteredMessages)
            .filter((msg) => !hidden.has(messageLocalId(msg)));
    })();

    $: inboxRows = (() => {
        const _p = pinnedMessageIds;
        const _s = selectedMessageIds;
        const _r = readMessageIds;
        return displayedMessages.map((msg) => {
            const sm = shortMessageCache[msg.message];
            const id = messageLocalId(msg);
            return {
                msg,
                pinned: _p.has(id),
                selected: _s.has(id),
                status: _r.has(id) ? "READ" : (msg.status || ""),
                decoded: sm?.is_short_message && sm.text,
                sm,
            };
        });
    })();

    /** @param {Event} event */
    async function selectInboxTablePack(event) {
        const target = event.target;
        if (!(target instanceof HTMLSelectElement) || !target.value) return;
        const pack = tablePacks.find((p) => p.fingerprint_sha256 === target.value);
        if (!pack) return;
        inboxTablePackBusy = true;
        inboxTablePackError = "";
        inboxTablePackStatus = "Loading table pack...";
        try {
            let selected;
            if (pack.builtin) {
                selected = await core.invoke("short_message_reset_table_pack");
            } else {
                selected = await core.invoke("short_message_select_table_pack", {
                    name: pack.name,
                    version: pack.version,
                    fingerprintSha256: pack.fingerprint_sha256,
                });
            }
            if (selected && typeof selected === "object") {
                setActivePackSummary(/** @type {TablePackSummary} */ (selected));
            }
            shortMessageCache = {};
            shortMessagePending.clear();
            await prepareShortMessagePack();
            if (displayedMessages.length > 0) {
                await decodePendingShortMessages(displayedMessages);
            }
            inboxTablePackStatus = "Table pack loaded.";
            setTimeout(() => { inboxTablePackStatus = ""; }, 1400);
        } catch (err) {
            inboxTablePackError = String(err);
            inboxTablePackStatus = "";
        } finally {
            inboxTablePackBusy = false;
        }
    }

    async function openInboxTablePackPanel() {
        inboxTablePackPanelOpen = !inboxTablePackPanelOpen;
        if (!inboxTablePackPanelOpen) return;
        inboxTablePackBusy = true;
        inboxTablePackStatus = "Loading table packs...";
        inboxTablePackError = "";
        try {
            await refreshTablePacks();
            inboxTablePackStatus = "";
        } catch (err) {
            inboxTablePackError = String(err);
            inboxTablePackStatus = "";
        } finally {
            inboxTablePackBusy = false;
        }
    }

    $: if (activeTab) {
        dispatch("tabChange", activeTab);
    }

    $: if (displayedMessages.length > 0) {
        decodePendingShortMessages(displayedMessages);
    }

    /** @param {AssetMessage[]} msgs */
    async function decodePendingShortMessages(msgs) {
        const toDecode = [];
        for (const msg of msgs) {
            if (shortMessageCache[msg.message] === undefined && !shortMessagePending.has(msg.message)) {
                toDecode.push(msg.message);
                shortMessagePending.add(msg.message);
            }
        }
        if (toDecode.length === 0) return;
        const promises = toDecode.map(async (hex) => {
            try {
                const result = await core.invoke("short_message_decode", { hex });
                shortMessageCache[hex] = result;
            } catch (e) {
                shortMessageCache[hex] = { is_short_message: false };
            } finally {
                shortMessagePending.delete(hex);
            }
        });
        await Promise.all(promises);
        shortMessageCache = shortMessageCache;
    }

    $: assetChannelNames = asset ? channelNamesForAsset(asset.name) : [];
    $: assetComposeChannelName = assetChannelNames.find((name) => name.endsWith("!"))
        ?? assetChannelNames[0]
        ?? "";

    $: filteredMessages = messages.filter((msg) =>
        assetChannelNames.includes(msg.asset_name),
    );

    $: if (asset) {
        isSubscribed = assetChannelNames.some((name) => channels.includes(name));
    }

    $: if (asset && asset.name !== currentAssetName) {
        currentAssetName = asset.name;
        activeTab = "DETAILS";
        messages = [];
        channels = [];
        messagesInfo = null;
        messagesError = "";
        selectedMessage = null;
        selectedMessageIds = new Set();
        messageExplorerMode = false;
        cancelCompose();
    }

    /** @param {string} name */
    function channelNamesForAsset(name) {
        if (!name) return [];
        const names = new Set([name]);
        if (name.endsWith("!")) {
            names.add(name.slice(0, -1));
        } else if (!name.includes("~")) {
            names.add(`${name}!`);
        }
        return Array.from(names);
    }

    async function openMessagesTab() {
        activeTab = "MESSAGES";
        await loadMessages();
    }

    async function loadMessages() {
        if (messagesLoading) return;
        messagesLoading = true;
        messagesError = "";
        try {
            const info = await core.invoke("get_messaging_info");
            messagesInfo = info;
            if (!info.enabled) {
                messages = [];
                channels = [];
                return;
            }
            const [msgs, chans] = await Promise.all([
                core.invoke("view_asset_messages"),
                core.invoke("view_message_channels"),
            ]);
            messages = msgs;
            channels = chans;
        } catch (err) {
            messagesError = messageRpcError(err);
            messages = [];
            channels = [];
        } finally {
            messagesLoading = false;
        }
    }

    function close() {
        dispatch("close");
    }

    function next() {
        dispatch("next");
    }

    function prev() {
        dispatch("prev");
    }

    // Action dispatchers
    function onTransfer() {
        dispatch("transfer", asset);
    }

    function onReissue() {
        dispatch("reissue", asset);
    }

    function openCidViewer() {
        if (!metadata?.ipfs_hash) return;
        cidViewerTarget.set(metadata.ipfs_hash);
    }

    function onSubAsset() {
        dispatch("createSub", asset);
    }

    function onNft() {
        dispatch("createNft", asset);
    }

    function onGovernance() {
        if (asset && asset.hasOwner) {
            const govAsset = {
                ...asset,
                reissuable: metadata?.reissuable ?? true,
            };
            dispatch("gov", govAsset);
        }
    }

    function onManageTags() {
        dispatch("manageTags", asset);
    }

    async function toggleSubscription() {
        if (!asset) return;
        const channelName = assetChannelNames.find((name) => channels.includes(name))
            ?? assetChannelNames.find((name) => name.endsWith('!'))
            ?? assetChannelNames[0]
            ?? asset.name;
        if (!channelName) return;
        try {
            if (isSubscribed) {
                await core.invoke("unsubscribe_from_channel", { channelName });
            } else {
                await core.invoke("subscribe_to_channel", { channelName });
            }
            await loadMessages();
        } catch (err) {
            messagesError = messageRpcError(err);
        }
    }

    function cancelCompose() {
        composeOpen = false;
    }
</script>

{#if asset}
    {#snippet panelContent()}
        <div class="modal-header detail-header-bar">
            <div class="header-nav-group">
                {#if hasMultipleAssets}
                    <button class="nav-arrow" on:click={prev} title="Previous Asset">«</button>
                {/if}
            </div>
            <div class="detail-icon">◈</div>
            <div class="detail-title-group">
                <div class="detail-title" title={asset.name}>
                    {#if asset.isSubAsset}
                        {asset.name.split("/").pop()}
                    {:else}
                        {asset.name}
                    {/if}
                </div>
                {#if asset.isSubAsset}
                    <div class="detail-parent-path" title={asset.name}>
                        {asset.name.split("/").slice(0, -1).join(" / ")}
                    </div>
                {/if}
            </div>
            <div class="header-nav-group">
                {#if hasMultipleAssets}
                    <button class="nav-arrow" on:click={next} title="Next Asset">»</button>
                {/if}
            </div>
            <button class="close-btn" on:click={close}>&times;</button>
        </div>

        <div class="modal-body detail-body-scroll">
            <div class="detail-tabs">
                <button
                    class="tab-btn"
                    class:active={activeTab === "DETAILS"}
                    on:click={() => (activeTab = "DETAILS")}
                >DETAILS</button>
                <button
                    class="tab-btn"
                    class:active={activeTab === "MESSAGES"}
                    on:click={openMessagesTab}
                >MESSAGES</button>
            </div>

            {#if activeTab === "DETAILS"}
            <AssetDetailDetailsTab
                {asset}
                {metadata}
                {loading}
                onGovernance={onGovernance}
                onTransfer={onTransfer}
                onReissue={onReissue}
                onManageTags={onManageTags}
                onSubAsset={onSubAsset}
                onNft={onNft}
                openCidViewer={openCidViewer}
                onShowAlert={() => (showAlert = true)}
            />
			{:else if activeTab === "MESSAGES"}
				{#if composeOpen}
					<AssetMessageCompose channelName={assetComposeChannelName} on:close={cancelCompose} on:sent={loadMessages} />
				{:else}
						<div class="messages-panel">
							<div class="messages-header">
								<div class="messages-title">
									{#if messageExplorerMode}
										ALL MESSAGES
									{:else}
										MESSAGES
									{/if}
									<span class="messages-count">{displayedMessages.length}</span>
								</div>
								<div class="messages-header-right">
									<button
										class="table-pack-mini-btn"
										class:active={inboxTablePackPanelOpen}
										on:click={openInboxTablePackPanel}
										title="Short message table packs"
									>
										<span class="mini-icon">🕮</span> {activePackLabel()}
									</button>
									<div class="messages-actions">
										<button
											class="action-btn subscribe-btn"
											class:subscribed={isSubscribed}
											on:click={toggleSubscription}
											disabled={messagesLoading}
											title={isSubscribed ? "Unsubscribe from this channel" : "Subscribe to this channel"}
										>
											{isSubscribed ? "UNSUB" : "SUB"}
										</button>
										{#if asset.hasOwner}
											<button
												class="action-btn primary"
												on:click={openCompose}
												title="Send announcement"
											>
												<span class="action-icon">✉</span> SEND
											</button>
										{/if}
									</div>
								</div>
							</div>

							<div class="inbox-toolbar">
								{#if pendingConfirmAction}
									<div class="confirm-bar">
											<span class="confirm-text">
											{#if pendingConfirmAction === 'clearUnpinned'}
												Hide all unpinned messages from this view?
											{:else}
												Hide {selectedMessageIds.size} selected message{selectedMessageIds.size === 1 ? '' : 's'} from this view?
											{/if}
										</span>
										<div class="confirm-actions">
											<button class="toolbar-btn danger" on:click={executeConfirmedAction}>Yes</button>
											<button class="toolbar-btn" on:click={cancelConfirmAction}>No</button>
										</div>
									</div>
								{:else}
									<button class="toolbar-btn" on:click={refreshMessages} disabled={messagesLoading} title="Refresh messages">
										<span class="toolbar-icon">↻</span>
									</button>
									<button
										class="toolbar-btn"
										class:active={messageExplorerMode}
										on:click={toggleExplorer}
										title={messageExplorerMode ? "Show this channel only" : "Show all channels"}
									>
										<span class="toolbar-icon">{messageExplorerMode ? "⊘" : "☰"}</span>
									</button>
									{#if selectedMessageIds.size > 0}
										<div class="bulk-bar">
											<span class="bulk-count">{selectedMessageIds.size} selected</span>
											<button class="toolbar-btn" on:click={selectAllDisplayed} title="Select all displayed">All</button>
											<button class="toolbar-btn" on:click={clearSelection} title="Clear selection">Clear</button>
											<button class="toolbar-btn" on:click={markSelectedUnread} title="Mark selected as unread">Unread</button>
											<button class="toolbar-btn danger" on:click={requestDeleteSelected} title="Hide selected from local view">Hide</button>
										</div>
									{:else}
										<button
											class="toolbar-btn"
											on:click={requestClearUnpinned}
											disabled={messagesLoading || displayedMessages.length === 0}
											title="Hide unpinned messages from local view"
										>
											<span class="toolbar-icon">🗑</span>
										</button>
									{/if}
									{#if pinnedMessageIds.size > 0}
										<span class="toolbar-chip">{pinnedMessageIds.size} pinned</span>
									{/if}
									{#if hiddenMessageIds.size > 0}
										<button class="toolbar-btn" on:click={restoreHiddenMessages} title="Restore locally hidden messages">
											Show hidden
										</button>
									{/if}
								{/if}
							</div>

							{#if inboxTablePackPanelOpen}
								<div class="table-pack-panel inbox-pack-panel">
									<div class="table-pack-head">
										<span title={activePackStatusTitle()}>Active: {activePackLabel()}</span>
										{#if activePackFingerprintShort()}
											<span class="mono" title={activePackFingerprint()}>{activePackFingerprintShort()}</span>
										{/if}
									</div>
									<div class="table-pack-controls">
										<select
											class="table-pack-select"
											style="background-color: #020604; color: #9cffad;"
											on:change={selectInboxTablePack}
											disabled={inboxTablePackBusy}
											value={tablePackSelectionFingerprint}
										>
											{#each tablePacks as pack}
												<option value={pack.fingerprint_sha256}>
													{tablePackLabel(pack)}
												</option>
											{/each}
										</select>
										<button type="button" on:click={exportOfficialTablePack} disabled={tablePackBusy}>EXPORT</button>
										<button type="button" on:click={importTablePack} disabled={tablePackBusy}>IMPORT</button>
										<button type="button" on:click={resetTablePack} disabled={tablePackBusy}>OFFICIAL</button>
									</div>
									<div class="table-pack-note">
										Select a pack to re-decode received messages. Both sender and receiver need the same custom pack to read custom-table messages.
									</div>
									{#if inboxTablePackStatus}
										<div class="table-pack-status" class:busy={inboxTablePackBusy}>{inboxTablePackStatus}</div>
									{/if}
									{#if inboxTablePackError}
										<div class="table-pack-error">{inboxTablePackError}</div>
									{/if}
								</div>
							{/if}

							{#if messagesInfo && !messagesInfo.enabled}
								{@const messagingWarnings = messagesInfo.warnings || []}
								<div class="messages-status warn">
									{#if messagingWarnings.length > 0}
										{messagingWarnings[0]}
									{:else}
										Messaging is disabled on this node. Enable it by removing -disablemessaging or waiting for BIP9 activation.
									{/if}
								</div>
							{/if}

							{#if messagesError}
								<div class="messages-status error">
									{messagesError}
								</div>
							{/if}

							{#if messagesLoading}
								<div class="messages-loading">Loading messages...</div>
							{:else if displayedMessages.length === 0}
								<div class="messages-empty">
									{#if messageExplorerMode}
										No messages across all channels.
									{:else}
										No messages for this asset channel.
									{/if}
								</div>
							{:else}
								<div class="messages-list">
									{#each inboxRows as row (row.msg.asset_name + row.msg.time + row.msg.message)}
										<div
											class="message-row"
											class:unread={row.status === 'UNREAD'}
											class:pinned={row.pinned}
											class:selected={row.selected}
											on:click={() => openMessageDetail(row.msg)}
											on:keydown={(e) => e.key === 'Enter' && openMessageDetail(row.msg)}
											role="button"
											tabindex="0"
										>
											<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
											<label class="row-checkbox-wrap" on:click|stopPropagation>
												<input
													type="checkbox"
													checked={row.selected}
													on:click|stopPropagation={() => toggleSelect(row.msg)}
												/>
											</label>
											<div class="message-row-main">
												<div class="message-row-left">
													<div class="message-row-channel">{row.msg.asset_name}</div>
													<div class="message-row-preview">
														{#if row.decoded}
															<span class="row-short-badge">SHORT</span>
															<span class="row-short-text">{row.sm?.text}</span>
														{:else if row.msg.message.length > 12}
															<span class="row-hash">{row.msg.message.slice(0, 8)}...{row.msg.message.slice(-8)}</span>
														{:else}
															<span class="row-hash">{row.msg.message}</span>
														{/if}
													</div>
												</div>
												<div class="message-row-right">
													<div class="message-row-time" title={String(row.msg.time ?? "")}>{formatMessageTime(row.msg.time)}</div>
													<div class="message-row-meta">
														<span class="row-block">#{row.msg.block_height}</span>
														{#if row.msg.expire_time}
															<span class="row-expire" title={String(row.msg.expire_time)}>{formatExpireTime(row.msg.expire_time)}</span>
														{/if}
													</div>
												</div>
											</div>
											<div class="message-row-bar">
												<span class="row-status" class:unread={row.status === 'UNREAD'}>
													{row.status}
												</span>
												<div class="row-actions">
													<button
														class="row-pin-btn"
														class:pinned={row.pinned}
														on:click|stopPropagation={() => togglePin(row.msg)}
															title={row.pinned ? "Unpin message" : "Pin message"}
													>
														{row.pinned ? "★" : "☆"}
													</button>
												</div>
											</div>
										</div>
									{/each}
								</div>
							{/if}

							{#if selectedMessage}
								{@const dsm = shortMessageCache[selectedMessage.message]}
								<div
									class="message-detail-overlay"
									role="dialog"
									aria-modal="true"
									tabindex="-1"
									on:click={closeMessageDetail}
									on:keydown={(e) => e.key === 'Escape' && closeMessageDetail()}
								>
									<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
									<div class="message-detail-panel" role="document" on:click|stopPropagation on:keydown|stopPropagation>
										<div class="message-detail-header">
											<div class="message-detail-title">MESSAGE DETAIL</div>
											<button class="detail-close-btn" on:click={closeMessageDetail}>×</button>
										</div>
										<div class="message-detail-body">
											<div class="detail-row">
												<span class="detail-label">Channel</span>
												<span class="detail-value">{selectedMessage.asset_name}</span>
											</div>
											<div class="detail-row">
												<span class="detail-label">Time</span>
												<span class="detail-value">{selectedMessage.time}</span>
											</div>
											<div class="detail-row">
												<span class="detail-label">Block</span>
												<span class="detail-value">#{selectedMessage.block_height}</span>
											</div>
											<div class="detail-row">
												<span class="detail-label">Status</span>
												<span class="detail-value" class:unread={displayMessageStatus(selectedMessage) === 'UNREAD'}>{displayMessageStatus(selectedMessage)}</span>
											</div>
											{#if selectedMessage.expire_time}
												<div class="detail-row">
													<span class="detail-label">Expires</span>
													<span class="detail-value">{selectedMessage.expire_time} ({formatExpireTime(selectedMessage.expire_time)})</span>
												</div>
											{/if}

											<div class="detail-divider"></div>

											{#if dsm?.is_short_message}
												<div class="detail-row">
													<span class="detail-label">Type</span>
													<span class="detail-value short-badge">Short Message</span>
												</div>
												<div class="detail-row tall">
													<span class="detail-label">Decoded</span>
													<span class="detail-value decoded-text">{dsm.text}</span>
												</div>
												{#if dsm.warnings && dsm.warnings.length > 0}
													<div class="detail-warnings">
														{#each dsm.warnings as w}
															<div class="detail-warning">⚠ {w}</div>
														{/each}
													</div>
												{/if}
												<div class="detail-row tall">
													<span class="detail-label">Raw Hex</span>
													<span class="detail-value mono hex-text">{selectedMessage.message}</span>
												</div>
											{:else}
												<div class="detail-row">
													<span class="detail-label">Type</span>
													<span class="detail-value">CID / Hash Reference</span>
												</div>
												<div class="detail-row tall">
													<span class="detail-label">Payload</span>
													<IpfsReference hash={selectedMessage.message} compact={false} />
												</div>
												{#if dsm && !dsm.is_short_message}
													<div class="detail-note">
														Not a recognized short-message frame. If this is a custom-table message, select the matching table pack in the inbox toolbar to decode it.
													</div>
												{/if}
											{/if}

											<div class="detail-divider"></div>

										<div class="detail-actions-row">
											<button class="action-btn" on:click={() => { if (selectedMessage) togglePin(selectedMessage); }}>
												{selectedMessage && isPinned(selectedMessage) ? "★ Unpin" : "☆ Pin"}
											</button>
											<button class="action-btn" on:click={() => { if (selectedMessage) markMessageUnread(selectedMessage); closeMessageDetail(); }}>
												Mark Unread
											</button>
											<button class="action-btn" on:click={closeMessageDetail}>
												Close
											</button>
										</div>
										</div>
									</div>
								</div>
							{/if}
						</div>
					{/if}
                    {/if}
                </div>
            {/snippet}
            {#if inline}
                <div class="detail-panel" in:fade={{ duration: 150 }}>
                    {@render panelContent()}
                </div>
            {:else}
                <div
                    class="modal-overlay"
                    transition:fade={{ duration: 150 }}
                    on:click={close}
                    on:keydown={(e) => e.key === "Escape" && close()}
                    role="button"
                    tabindex="0"
                >
                    <div class="modal-container">
                        {#if hasMultipleAssets}
                            <button
                                class="nav-arrow nav-prev"
                                on:click|stopPropagation={prev}
                                title="Previous Asset">«</button
                            >
                        {/if}
                        {#key asset.name}
                            <div
                                class="detail-modal glass-modal"
                                in:fly={{ x: slideDirection * 40, duration: 180 }}
                                out:fly={{ x: slideDirection * -40, duration: 120, opacity: 0 }}
                                on:click|stopPropagation
                                on:keydown|stopPropagation
                                role="dialog"
                                aria-modal="true"
                                tabindex="-1"
                            >
                                <button class="modal-close" on:click={close}>×</button>
                                {@render panelContent()}
                            </div>
                        {/key}
                        {#if hasMultipleAssets}
                            <button
                                class="nav-arrow nav-next"
                                on:click|stopPropagation={next}
                                title="Next Asset">»</button
                            >
                        {/if}
                    </div>
                </div>
            {/if}

            <!-- Alert Modal for Coming Soon features -->
            <ModalAlert
                isOpen={showAlert}
                title="Coming Soon"
                message="Top 100 Holders list requires 'assetindex=1' node configuration. This feature is deferred."
                on:close={() => (showAlert = false)}
            />
        {/if}

<style>
    /* Local Styles extracted from ViewAssets */
    .detail-modal {
        width: 100%;
        max-width: 550px;
        position: relative;
        background: rgba(10, 15, 12, 0.95);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        box-shadow:
            0 0 80px rgba(0, 0, 0, 0.8),
            0 0 40px rgba(0, 255, 65, 0.1);
        overflow: visible;
    }
    .modal-container {
        display: flex;
        align-items: center;
        gap: 1rem;
        /* Ensure it fits in viewport */
        max-width: 100vw;
        justify-content: center;
    }
    .nav-arrow {
        flex-shrink: 0;
        background: transparent;
        border: none;
        color: var(--color-primary);
        font-size: 1.8rem;
        font-weight: 300;
        cursor: pointer;
        transition: all 0.2s;
        padding: 0.5rem;
        opacity: 0.5;
        text-shadow: 0 0 5px rgba(0, 255, 65, 0.3);
    }
    .nav-arrow:hover {
        opacity: 1;
        text-shadow:
            0 0 10px var(--color-primary),
            0 0 20px var(--color-primary),
            0 0 30px rgba(0, 255, 65, 0.5);
        transform: scale(1.2);
    }

    .modal-close {
        position: absolute;
        top: 1rem;
        right: 1rem;
        background: transparent;
        border: none;
        color: #555;
        font-size: 1.5rem;
        cursor: pointer;
        transition: all 0.15s;
        width: 32px;
        height: 32px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 8px;
    }
    .modal-close:hover {
        color: #fff;
        background: rgba(255, 255, 255, 0.1);
    }
    .detail-icon {
        font-size: 1.5rem;
        color: var(--color-primary);
        text-shadow: 0 0 30px rgba(0, 255, 65, 0.5);
    }
    .detail-title {
        font-size: 1.2rem;
        font-weight: 700;
        color: #fff;
        letter-spacing: 2px;
    }
    .detail-title-group {
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .detail-parent-path {
        font-size: 0.7rem;
        color: #666;
        letter-spacing: 0.5px;
        max-width: 320px;
        text-overflow: ellipsis;
        overflow: hidden;
        white-space: nowrap;
    }
    .action-btn {
        flex: 1;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        padding: 0.6rem;
        color: #aaa;
        font-size: 0.65rem;
        font-weight: 600;
        letter-spacing: 1px;
        cursor: pointer;
        transition: all 0.15s;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.4rem;
        white-space: nowrap;
    }
    .action-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .action-btn.primary {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .action-btn.primary:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 20px var(--color-primary);
    }
    .action-icon {
        font-size: 1rem;
    }

    /* Tabs */
    .detail-tabs {
        display: flex;
        gap: 0;
        margin-bottom: 0.35rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }
    .tab-btn {
        flex: 1;
        background: transparent;
        border: none;
        border-bottom: 2px solid transparent;
        color: #555;
        padding: 0.5rem;
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 1px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .tab-btn:hover {
        color: #aaa;
    }
    .tab-btn.active {
        color: var(--color-primary);
        border-bottom-color: var(--color-primary);
    }

    /* Messages Panel */
    .messages-panel {
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        min-height: 0;
    }
    .messages-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .messages-title {
        font-size: 0.7rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .messages-count {
        font-size: 0.55rem;
        color: #888;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        padding: 0.1rem 0.4rem;
        font-weight: 600;
    }
    .messages-header-right {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .table-pack-mini-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
        background: rgba(0, 255, 65, 0.04);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 5px;
        padding: 0.25rem 0.5rem;
        color: #8cff9f;
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        white-space: nowrap;
    }
    .table-pack-mini-btn:hover {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
    }
    .table-pack-mini-btn.active {
        background: rgba(0, 255, 65, 0.12);
        border-color: rgba(0, 255, 65, 0.35);
    }
    .mini-icon {
        font-size: 0.75rem;
    }
    .messages-actions {
        display: flex;
        gap: 0.4rem;
    }
    .subscribe-btn.subscribed {
        color: #ff5555;
        border-color: rgba(255, 85, 85, 0.4);
    }
    .inbox-toolbar {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        flex-wrap: wrap;
    }
    .toolbar-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        padding: 0.25rem 0.45rem;
        color: #888;
        font-size: 0.58rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .toolbar-btn:hover:not(:disabled) {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .toolbar-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .toolbar-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .toolbar-icon {
        font-size: 0.8rem;
    }
    .toolbar-chip {
        font-size: 0.5rem;
        color: #888;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        padding: 0.15rem 0.4rem;
    }
    .bulk-bar {
        display: inline-flex;
        align-items: center;
        gap: 0.3rem;
        background: rgba(0, 0, 0, 0.35);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 5px;
        padding: 0.2rem 0.4rem;
        margin-left: 0.2rem;
    }
    .bulk-count {
        font-size: 0.55rem;
        color: var(--color-primary);
        font-weight: 600;
        letter-spacing: 0.5px;
        padding: 0 0.2rem;
    }
    .toolbar-btn.danger {
        border-color: rgba(255, 84, 84, 0.25);
        color: #ff9999;
    }
    .toolbar-btn.danger:hover:not(:disabled) {
        background: rgba(255, 84, 84, 0.1);
        border-color: rgba(255, 84, 84, 0.4);
    }
    .confirm-bar {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
        flex: 1;
        background: rgba(255, 84, 84, 0.06);
        border: 1px solid rgba(255, 84, 84, 0.18);
        border-radius: 5px;
        padding: 0.25rem 0.55rem;
    }
    .confirm-text {
        font-size: 0.6rem;
        color: #ff9999;
        font-weight: 600;
        letter-spacing: 0.3px;
    }
    .confirm-actions {
        display: flex;
        gap: 0.3rem;
        margin-left: auto;
    }
    .messages-status {
        font-size: 0.6rem;
        padding: 0.5rem;
        border-radius: 6px;
        color: #aaa;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
    }
    .messages-status.warn {
        color: #ffaa00;
        border-color: rgba(255, 170, 0, 0.3);
    }
    .messages-status.error {
        color: #ff5555;
        border-color: rgba(255, 85, 85, 0.3);
    }
    .messages-loading,
    .messages-empty {
        font-size: 0.65rem;
        color: #555;
        text-align: center;
        padding: 1.5rem 0;
    }
    .messages-list {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
        max-height: 320px;
        overflow-y: auto;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .messages-list::-webkit-scrollbar {
        width: 6px;
    }
    .messages-list::-webkit-scrollbar-track {
        background: transparent;
    }
    .messages-list::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 3px;
    }
    .message-row {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        padding: 0.4rem 0.55rem;
        cursor: pointer;
        transition: all 0.12s;
    }
    .message-row:hover {
        background: rgba(0, 0, 0, 0.4);
        border-color: rgba(0, 255, 65, 0.2);
    }
    .message-row.unread {
        border-left: 2px solid var(--color-primary);
    }
    .message-row.pinned {
        border-color: rgba(0, 255, 65, 0.15);
    }
    .message-row.selected {
        background: rgba(0, 255, 65, 0.06);
        border-color: rgba(0, 255, 65, 0.22);
    }
    .row-checkbox-wrap {
        display: flex;
        align-items: center;
        padding: 0.1rem 0;
        margin: -0.15rem 0;
        cursor: pointer;
    }
    .row-checkbox-wrap input[type="checkbox"] {
        accent-color: var(--color-primary);
        width: 0.75rem;
        height: 0.75rem;
        cursor: pointer;
    }
    .message-row-main {
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        gap: 0.5rem;
    }
    .message-row-left {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
        min-width: 0;
        flex: 1;
    }
    .message-row-channel {
        font-size: 0.6rem;
        font-weight: 600;
        color: var(--color-primary);
        letter-spacing: 0.3px;
    }
    .message-row-preview {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        min-width: 0;
    }
    .row-short-badge {
        font-size: 0.45rem;
        font-weight: 700;
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 3px;
        padding: 0.05rem 0.25rem;
        letter-spacing: 0.5px;
        flex-shrink: 0;
    }
    .row-short-text {
        font-size: 0.65rem;
        color: #ccc;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .row-hash {
        font-size: 0.6rem;
        color: #777;
        font-family: var(--font-mono);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .message-row-right {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 0.15rem;
        flex-shrink: 0;
    }
    .message-row-time {
        font-size: 0.55rem;
        color: #888;
        white-space: nowrap;
    }
    .message-row-meta {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        font-size: 0.5rem;
        color: #666;
    }
    .row-block {
        font-family: var(--font-mono);
    }
    .row-expire {
        color: #aa8800;
    }
    .message-row-bar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        border-top: 1px solid rgba(255, 255, 255, 0.04);
        padding-top: 0.2rem;
        margin-top: 0.1rem;
    }
    .row-status {
        font-size: 0.5rem;
        color: #666;
        letter-spacing: 0.5px;
    }
    .row-status.unread {
        color: var(--color-primary);
    }
    .row-actions {
        display: flex;
        gap: 0.25rem;
    }
    .row-pin-btn {
        background: transparent;
        border: 1px solid transparent;
        border-radius: 4px;
        color: #444;
        font-size: 0.95rem;
        cursor: pointer;
        padding: 0.15rem 0.35rem;
        line-height: 1;
        transition: all 0.15s;
    }
    .row-pin-btn:hover {
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.08);
    }
    .row-pin-btn.pinned {
        color: #00ff41;
        background: rgba(0, 255, 65, 0.22);
        border-color: rgba(0, 255, 65, 0.55);
        text-shadow: 0 0 6px rgba(0, 255, 65, 0.45);
    }
    .inbox-pack-panel {
        margin-top: 0.15rem;
    }

    /* Message detail overlay */
    .message-detail-overlay {
        position: fixed;
        inset: 0;
        z-index: 500;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.75);
        backdrop-filter: blur(2px);
    }
    .message-detail-panel {
        width: min(32rem, 92vw);
        max-height: 85vh;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        background: linear-gradient(180deg, #080b09 0%, #0f1410 100%);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8), 0 0 36px rgba(0, 255, 65, 0.12);
    }
    .message-detail-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.6rem 0.8rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.05);
    }
    .message-detail-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.5px;
    }
    .detail-close-btn {
        background: transparent;
        border: none;
        color: #777;
        font-size: 1.4rem;
        cursor: pointer;
        line-height: 1;
        padding: 0.15rem 0.35rem;
        transition: color 0.15s;
    }
    .detail-close-btn:hover {
        color: #fff;
    }
    .message-detail-body {
        padding: 0.7rem 0.9rem;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .message-detail-body::-webkit-scrollbar {
        width: 6px;
    }
    .message-detail-body::-webkit-scrollbar-track {
        background: transparent;
    }
    .message-detail-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 3px;
    }
    .detail-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.5rem;
    }
    .detail-row.tall {
        align-items: flex-start;
        flex-direction: column;
        gap: 0.2rem;
    }
    .detail-label {
        font-size: 0.55rem;
        color: #777;
        letter-spacing: 0.5px;
        flex-shrink: 0;
    }
    .detail-value {
        font-size: 0.65rem;
        color: #ccc;
        text-align: right;
        word-break: break-word;
    }
    .detail-value.unread {
        color: var(--color-primary);
    }
    .detail-value.short-badge {
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        padding: 0.1rem 0.35rem;
        font-size: 0.55rem;
        font-weight: 600;
    }
    .detail-value.decoded-text {
        font-size: 0.75rem;
        color: #e0e0e0;
        line-height: 1.4;
        text-align: left;
    }
    .detail-value.hex-text {
        font-size: 0.6rem;
        color: #888;
        background: rgba(0, 0, 0, 0.3);
        padding: 0.35rem 0.45rem;
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.06);
        word-break: break-all;
        text-align: left;
        width: 100%;
    }
    .detail-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.25rem 0;
    }
    .detail-warnings {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .detail-warning {
        font-size: 0.6rem;
        color: #ffaa00;
    }
    .detail-note {
        font-size: 0.6rem;
        color: #888;
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 4px;
        padding: 0.4rem 0.55rem;
        line-height: 1.4;
    }
    .detail-actions-row {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
        margin-top: 0.25rem;
    }
    .detail-actions-row .action-btn {
        flex: none;
        padding: 0.35rem 0.65rem;
    }

    .table-pack-panel {
        border: 1px solid rgba(0, 255, 65, 0.14);
        border-radius: 6px;
        background: rgba(0, 255, 65, 0.035);
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        margin-top: 0.35rem;
        padding: 0.55rem;
    }
    .table-pack-head {
        align-items: center;
        color: #b9ffd0;
        display: flex;
        flex-wrap: wrap;
        font-size: 0.68rem;
        gap: 0.5rem;
        justify-content: space-between;
    }
    .table-pack-controls {
        display: grid;
        gap: 0.35rem;
        grid-template-columns: minmax(10rem, 0.8fr) repeat(4, auto);
    }
    .table-pack-controls select,
    .table-pack-controls button {
        background: rgba(0, 10, 4, 0.92);
        background-color: rgba(0, 10, 4, 0.92);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        color: #cfd8cf;
        font-family: var(--font-mono);
        font-size: 0.58rem;
        min-height: 1.8rem;
        padding: 0.25rem 0.45rem;
    }
    .table-pack-controls select,
    .table-pack-select {
        appearance: none;
        -webkit-appearance: none;
        background: #020604 !important;
        background-color: #020604 !important;
        color: #9cffad !important;
        color-scheme: dark;
        max-width: 23rem;
        padding-right: 1.6rem;
    }
    .table-pack-controls select option,
    .table-pack-select option {
        background: #020604;
        color: #d8f7dd;
    }
    .table-pack-controls button {
        color: var(--color-primary);
        cursor: pointer;
        font-weight: 700;
        letter-spacing: 0.6px;
    }
    .table-pack-controls button:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.1);
    }
    .table-pack-controls button:disabled,
    .table-pack-controls select:disabled {
        cursor: not-allowed;
        opacity: 0.5;
    }
    .table-pack-note {
        color: #8d968d;
        font-size: 0.62rem;
        line-height: 1.35;
    }
    .table-pack-status {
        align-items: center;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        color: #9cffad;
        display: flex;
        font-size: 0.62rem;
        gap: 0.45rem;
        margin-top: 0.35rem;
        padding: 0.35rem 0.45rem;
    }
    .table-pack-status.busy::before {
        animation: table-pack-spin 0.8s linear infinite;
        border: 2px solid rgba(0, 255, 65, 0.18);
        border-top-color: var(--color-primary);
        border-radius: 999px;
        content: "";
        display: inline-block;
        flex: 0 0 auto;
        height: 0.75rem;
        width: 0.75rem;
    }
    @keyframes table-pack-spin {
        to {
            transform: rotate(360deg);
        }
    }
    .table-pack-error {
        border: 1px solid rgba(255, 80, 80, 0.25);
        border-radius: 5px;
        color: #ff8a8a;
        font-size: 0.62rem;
        padding: 0.35rem 0.45rem;
    }
    @media (max-width: 880px) {
        .table-pack-controls {
            grid-template-columns: 1fr;
        }
    }
    .detail-panel {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
    .detail-header-bar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.5rem 1rem 0.65rem;
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        flex-shrink: 0;
        gap: 0.5rem;
    }
    .detail-header-bar .detail-icon {
        font-size: 1.2rem;
    }
    .detail-header-bar .detail-title {
        font-size: 0.9rem;
        letter-spacing: 1px;
    }
    .detail-header-bar .detail-title-group {
        flex: 1;
        align-items: flex-start;
    }
    .detail-header-bar .header-nav-group {
        display: flex;
        align-items: center;
        gap: 0.25rem;
    }
    .detail-header-bar .nav-arrow {
        font-size: 1rem;
        padding: 0.2rem 0.4rem;
        opacity: 0.6;
    }
    .detail-header-bar .nav-arrow:hover {
        opacity: 1;
    }
    .close-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .close-btn:hover { color: #fff; }
    .detail-body-scroll {
        padding: 0.4rem 0.9rem 0.6rem;
        overflow-y: auto;
        overflow-x: hidden;
        flex: 1 1 0%;
        display: flex;
        flex-direction: column;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .detail-body-scroll::-webkit-scrollbar {
        width: 8px;
    }
    .detail-body-scroll::-webkit-scrollbar-track {
        background: transparent;
    }
    .detail-body-scroll::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 4px;
    }
    .detail-body-scroll::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.55);
    }
</style>
