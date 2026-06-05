<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher, tick } from "svelte";
    import { core } from "@tauri-apps/api";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { formatBalance, insertSuggestion } from "../utils.js";
    import "../../components.css";
    import Tooltip from "../ui/Tooltip.svelte";
    import IpfsReference from "../ui/IpfsReference.svelte";
    import IpfsHashField from "../ui/IpfsHashField.svelte";
    import HelpHitbox from "../ui/HelpHitbox.svelte";
    import ModalAlert from "./ModalAlert.svelte";
    import ShortMessageAutocomplete from "../ui/ShortMessageAutocomplete.svelte";
    import { addNotification } from "../stores/notifications.js";
    import { cidViewerTarget } from "../stores/contentLibrary.js";

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
     * @typedef {{ enabled?: boolean }} MessagesInfo
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     * }} AssetMessage
     * @typedef {{
     *   channel_name?: string;
     *   ipfs_hash?: string;
     *   expire_time?: number|null;
     *   has_ownership?: boolean;
     *   warnings?: string[];
     * }} ComposePreview
     * @typedef {{
     *   is_short_message?: boolean;
     *   text?: string;
     * }} ShortMessageDecodeResult
     * @typedef {{
     *   fits: boolean;
     *   hex: string;
     *   decoded_preview: string;
     *   encoded_payload_len: number;
     *   warnings: string[];
     *   dictionary_index?: number|null;
     *   dictionary_name?: string|null;
     * }} ShortMessageEncodeResult
     * @typedef {"cid"|"short"} ComposeMode
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
    let composeIpfsHash = "";
    let composeExpireTime = "";
    /** @type {ComposePreview | null} */
    let composePreview = null;
    let composeError = "";
    let composePreviewing = false;
    let composeBroadcasting = false;
    let composeSent = false;
    let currentAssetName = "";

    // Short message compose state
    /** @type {ComposeMode} */
    let composeMode = "cid";
    let composeShortText = "";
    /** @type {ShortMessageEncodeResult | null} */
    let composeShortResult = null;
    let composeShortEncoding = false;
    let composeShortShowHex = false;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let composeShortDebounce = null;
    /** @type {HTMLTextAreaElement | null} */
    let composeShortTextarea = null;
    let autocompleteEnabled = true;
    let composeShortFocused = false;
    /** @type {string[]} */
    let composeShortEmojis = [];
    let composeShortEmojiOpen = false;
    /** @type {HTMLDivElement | null} */
    let composeShortEmojiWrap = null;

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
    let tablePackPanelOpen = false;
    let tablePackBusy = false;
    let tablePackError = "";
    let tablePackStatus = "";
    let tablePackSelectionFingerprint = "";
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
            const emojis = await core.invoke("short_message_emojis");
            composeShortEmojis = Array.isArray(emojis) ? emojis : [];
        } catch (err) {
            // The next encode/suggestion request will surface any real issue.
        }
    }

    function refreshShortMessageAfterPackChange() {
        setTimeout(() => {
            queueEncodeShortMessage();
        }, 0);
    }

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

    /** @param {string} suggestion */
    function handleShortSuggestion(suggestion) {
        const nextText = insertSuggestion(composeShortText, suggestion);
        composeShortText = nextText;
        queueEncodeShortMessage();
        return nextText;
    }

    async function ensureShortEmojis() {
        if (composeShortEmojis.length > 0) return;
        try {
            const result = await core.invoke("short_message_emojis");
            composeShortEmojis = Array.isArray(result) ? result : [];
        } catch (err) {
            composeShortEmojis = [];
        }
    }

    async function refreshActiveTablePack() {
        try {
            const status = await core.invoke("short_message_get_active_table_pack");
            activeTablePack = status;
            tablePackSelectionFingerprint = status?.active?.fingerprint_sha256 || "";
        } catch (err) {
            activeTablePack = null;
            tablePackSelectionFingerprint = "";
        }
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

    async function toggleTablePackPanel() {
        tablePackPanelOpen = !tablePackPanelOpen;
        if (!tablePackPanelOpen) return;
        await showTablePackBusy("Loading table packs...");
        try {
            await refreshTablePacks();
            finishTablePackBusy();
        } catch (err) {
            tablePackError = String(err);
            finishTablePackBusy();
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

    /** @param {Event} event */
    async function selectTablePack(event) {
        const target = event.target;
        if (!(target instanceof HTMLSelectElement) || !target.value) return;
        const pack = tablePacks.find((p) => p.fingerprint_sha256 === target.value);
        if (!pack) return;
        tablePackSelectionFingerprint = pack.fingerprint_sha256;
        await showTablePackBusy("Loading table pack...");
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
            composeShortResult = null;
            await prepareShortMessagePack();
            refreshShortMessageAfterPackChange();
            finishTablePackBusy("Table pack loaded.");
        } catch (err) {
            tablePackError = String(err);
            tablePackSelectionFingerprint = activeTablePack?.active?.fingerprint_sha256 || "";
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
            composeShortResult = null;
            await prepareShortMessagePack();
            refreshShortMessageAfterPackChange();
            finishTablePackBusy("Official table pack loaded.");
        } catch (err) {
            tablePackError = String(err);
            finishTablePackBusy();
        }
    }

    async function deleteSelectedTablePack() {
        const pack = selectedTablePack();
        if (!pack || pack.builtin) return;
        const confirmed = window.confirm(`Delete custom table pack "${pack.name}" v${pack.version}?`);
        if (!confirmed) return;
        await showTablePackBusy("Deleting table pack...");
        try {
            const selected = await core.invoke("short_message_delete_table_pack", {
                name: pack.name,
                version: pack.version,
                fingerprintSha256: pack.fingerprint_sha256,
            });
            if (selected && typeof selected === "object") {
                setActivePackSummary(/** @type {TablePackSummary} */ (selected));
            }
            composeShortResult = null;
            await prepareShortMessagePack();
            refreshShortMessageAfterPackChange();
            finishTablePackBusy("Table pack deleted.");
        } catch (err) {
            tablePackError = String(err);
            finishTablePackBusy();
        }
    }

    /** @param {ComposeMode} mode */
    function switchComposeMode(mode) {
        composeMode = mode;
        composeError = "";
        if (mode === "cid") {
            composeShortText = "";
            composeShortResult = null;
            composeShortEmojiOpen = false;
            composeIpfsHash = "";
            return;
        }
        composeIpfsHash = "";
        ensureShortEmojis();
        refreshActiveTablePack();
    }

    /** @param {string} emoji */
    function insertShortEmoji(emoji) {
        if (!emoji || composeBroadcasting || composeSent || composePreview) return;
        const textarea = composeShortTextarea;
        if (!textarea) {
            composeShortText = `${composeShortText}${emoji}`;
            queueEncodeShortMessage();
            return;
        }

        const start = textarea.selectionStart ?? composeShortText.length;
        const end = textarea.selectionEnd ?? composeShortText.length;
        composeShortText =
            composeShortText.slice(0, start) + emoji + composeShortText.slice(end);
        queueEncodeShortMessage();
        requestAnimationFrame(() => {
            textarea.focus();
            const next = start + emoji.length;
            textarea.setSelectionRange(next, next);
        });
    }

    /** @param {FocusEvent} event */
    function handleTextareaFocusOut(event) {
        const related = event.relatedTarget;
        const wrap = event.currentTarget;
        if (
            wrap instanceof HTMLElement &&
            (!(related instanceof Node) || !wrap.contains(related))
        ) {
            composeShortFocused = false;
        }
    }

    /** @param {MouseEvent} event */
    function handleWindowClick(event) {
        if (!composeShortEmojiOpen || !composeShortEmojiWrap) return;
        const target = event.target instanceof Node ? event.target : null;
        if (target && composeShortEmojiWrap.contains(target)) return;
        composeShortEmojiOpen = false;
    }

    // Short message decode cache for received messages
    /** @type {Record<string, ShortMessageDecodeResult | undefined>} */
    let shortMessageCache = {};
    /** @type {Set<string>} */
    let shortMessagePending = new Set();

    $: if (activeTab) {
        dispatch("tabChange", activeTab);
    }

    $: if (filteredMessages.length > 0) {
        decodePendingShortMessages(filteredMessages);
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

    function queueEncodeShortMessage() {
        if (composeShortDebounce) clearTimeout(composeShortDebounce);
        composeShortDebounce = setTimeout(() => {
            encodeShortMessage();
        }, 300);
    }

    async function encodeShortMessage() {
        const text = composeShortText.trim();
        if (!text) {
            composeShortResult = null;
            composeIpfsHash = "";
            composeError = "";
            return;
        }
        composeShortEncoding = true;
        composeError = "";
        try {
            const result = await core.invoke("short_message_encode", { text });
            composeShortResult = result;
            if (result.fits) {
                composeIpfsHash = result.hex;
                composeError = "";
            } else {
                composeIpfsHash = "";
                composeError = "Message is too long to fit in the short-message field.";
            }
        } catch (err) {
            composeShortResult = null;
            composeIpfsHash = "";
            composeError = String(err);
        } finally {
            composeShortEncoding = false;
        }
    }

    $: assetChannelNames = asset ? channelNamesForAsset(asset.name) : [];

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

    async function previewAnnouncement() {
        if (composeMode === "short") {
            if (!composeShortResult?.fits) {
                composeError = "Short message does not fit. Edit the message to make it shorter.";
                return;
            }
        } else if (!composeIpfsHash.trim()) {
            composeError = "CID/hash is required";
            return;
        }
        const expireTime = parseComposeExpireTime();
        if (expireTime === undefined) return;
        composePreviewing = true;
        composeError = "";
        composePreview = null;
        try {
            const channelName = asset.name;
            const ipfsHash = composeMode === "short" && composeShortResult
                ? composeShortResult.hex
                : composeIpfsHash.trim();
            composePreview = await core.invoke("preview_send_announcement", {
                channelName,
                ipfsHash,
                expireTime,
            });
        } catch (err) {
            composeError = messageRpcError(err);
        } finally {
            composePreviewing = false;
        }
    }

    async function broadcastAnnouncement() {
        if (!composePreview) return;
        if (composeMode === "short" && !composeShortResult?.fits) {
            composeError = "Short message does not fit. Edit the message to make it shorter.";
            return;
        }
        const expireTime = composePreview.expire_time ?? null;
        composeBroadcasting = true;
        composeError = "";
        try {
            const channelName = asset.name;
            const ipfsHash = composeMode === "short" && composeShortResult
                ? composeShortResult.hex
                : composeIpfsHash.trim();
            const txid = await core.invoke("send_announcement", {
                channelName,
                ipfsHash,
                expireTime,
            });
            composeSent = true;
            composePreview = null;
            composeIpfsHash = "";
            composeExpireTime = "";
            addNotification({
                type: "message",
                severity: "success",
                title: "Announcement Sent",
                body: `Message sent on channel ${asset.name}`,
                action: { label: "Copy TXID", txid },
            });
            await loadMessages();
        } catch (err) {
            composeError = messageRpcError(err);
            addNotification({
                type: "message",
                severity: "error",
                title: "Announcement Failed",
                body: messageRpcError(err),
            });
        } finally {
            composeBroadcasting = false;
        }
    }

    function cancelCompose() {
        composeOpen = false;
        composePreview = null;
        composeError = "";
        composeIpfsHash = "";
        composeExpireTime = "";
        composeSent = false;
        composeMode = "cid";
        composeShortText = "";
        composeShortResult = null;
        composeShortShowHex = false;
        if (composeShortDebounce) {
            clearTimeout(composeShortDebounce);
            composeShortDebounce = null;
        }
    }

    function openContentLibrary() {
        window.dispatchEvent(new CustomEvent("commander-open-content-library"));
        cancelCompose();
    }

    function parseComposeExpireTime() {
        const raw = composeExpireTime.trim();
        if (!raw) return null;
        if (!/^\d+$/.test(raw)) {
            composeError = "Expire time must be a positive UTC timestamp.";
            return undefined;
        }
        const parsed = Number(raw);
        if (!Number.isSafeInteger(parsed) || parsed <= 0) {
            composeError = "Expire time must be a positive UTC timestamp.";
            return undefined;
        }
        return parsed;
    }
</script>

<svelte:window on:click={handleWindowClick} />

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
            <div class="detail-grid">
                <div class="detail-stat">
                    <div class="stat-label">YOUR BALANCE</div>
                    <div class="stat-value neon-text">
                        {formatBalance(asset.balance)}
                    </div>
                </div>
                <div class="detail-stat">
                    <div class="stat-label">TYPE</div>
                    <div class="stat-value">
                        {asset.name.includes("#")
                                ? "NFT"
                                : asset.isSubAsset
                                    ? "SUB-ASSET"
                                    : asset.type || "TOKEN"}
                    </div>
                </div>
                <div class="detail-stat">
                    <div class="stat-label">STATUS</div>
                    <div
                        class="stat-value"
                        class:owner-yes={asset.hasOwner}
                        class:clickable={asset.hasOwner}
                        role="button"
                        tabindex={asset.hasOwner ? 0 : -1}
                        title={asset.hasOwner
                            ? "Manage Governance"
                            : "Holder — no owner token"}
                        on:click={onGovernance}
                        on:keydown={(e) =>
                            e.key === "Enter" && onGovernance()}
                    >
                        {asset.hasOwner ? "👑 OWNER" : "HOLDER"}
                    </div>
                </div>
                <div class="detail-stat">
                    <div class="stat-label">DECIMALS</div>
                    <div class="stat-value">
                        {metadata?.units ?? asset.units ?? 0}
                    </div>
                </div>
            </div>

            <!-- Metadata Section -->
            {#if loading}
                <div class="metadata-section">
                    <div class="meta-loading">
                        Loading metadata...
                    </div>
                </div>
            {:else if metadata}
                <div class="metadata-section">
                    <div class="meta-card">
                                    <div class="meta-row">
                                        <span class="meta-label">TOTAL SUPPLY</span>
                                        <Tooltip text="Top 100 Holders (Coming Soon)">
                                            <span
                                                class="meta-value clickable"
                                                role="button"
                                                tabindex="0"
                                                on:click={() => (showAlert = true)}
                                                on:keydown={(e) =>
                                                    e.key === "Enter" &&
                                                    (showAlert = true)}
                                            >{metadata.amount.toLocaleString()}</span>
                                        </Tooltip>
                                    </div>
                                    <div class="meta-row">
                                        <span class="meta-label">REISSUABLE</span>
                                        <span class="meta-value" class:yes={metadata.reissuable}>
                                            {metadata.reissuable ? "YES" : "NO"}
                                        </span>
                                    </div>
                                    <div class="meta-row">
                                        <span class="meta-label">CREATED AT BLOCK</span>
                                        <span class="meta-value">{metadata.block_height.toLocaleString()}</span>
                                    </div>
                                </div>

                                {#if metadata.has_ipfs && metadata.ipfs_hash}
                                    <div class="meta-card ipfs-card">
                                        <div class="ipfs-header">
                                            <span class="meta-label">METADATA CID / HASH</span>
                                            <div class="ipfs-header-actions">
                                                {#if asset.hasOwner && metadata?.reissuable}
                                                    <button
                                                        class="meta-update-btn"
                                                        on:click={onReissue}
                                                        title="Update metadata via reissue"
                                                    >
                                                        <span class="action-icon">↻</span> UPDATE
                                                    </button>
                                                {/if}
                                                <button
                                                    class="meta-view-btn"
                                                    on:click={openCidViewer}
                                                    title="Open in CID viewer"
                                                >
                                                    <span class="action-icon">◉</span> VIEW
                                                </button>
                                            </div>
                                        </div>
                                        <div class="ipfs-value">
                                            <IpfsReference hash={metadata.ipfs_hash} compact={false} />
                                        </div>
                                    </div>
                                {:else}
                                    <div class="meta-card ipfs-card empty">
                                        <div class="ipfs-header">
                                            <span class="meta-label">METADATA CID / HASH</span>
                                            {#if asset.hasOwner && metadata?.reissuable}
                                                <button
                                                    class="meta-update-btn"
                                                    on:click={onReissue}
                                                    title="Add metadata via reissue"
                                                >
                                                    <span class="action-icon">+</span> ADD
                                                </button>
                                            {/if}
                                        </div>
                                        <div class="ipfs-placeholder">No metadata set</div>
                                    </div>
                                {/if}
                            </div>
                        {/if}

                        <div class="detail-actions">
                            <button
                                class="action-btn primary"
                                on:click={onTransfer}
                            >
                                <span class="action-icon">→</span> TRANSFER
                            </button>
                            {#if asset.hasOwner}
                                <button
                                    class="action-btn"
                                    class:disabled={!metadata?.reissuable}
                                    on:click={onReissue}
                                    disabled={!metadata?.reissuable}
                                    title={!metadata?.reissuable
                                        ? "Asset supply is locked"
                                        : "Reissue or update metadata"}
                                >
                                    <span class="action-icon">↻</span> REISSUE
                                </button>
                            {/if}
                        </div>
                        {#if asset.hasOwner && asset.name.startsWith("#")}
                            <div class="detail-actions owner-actions">
                                <button
                                    class="action-btn"
                                    on:click={onManageTags}
                                >
                                    <span class="action-icon">🏷</span> MANAGE TAGS
                                </button>
                            </div>
                        {/if}
                        {#if asset.hasOwner && !asset.name.includes("#")}
                            <div class="detail-actions owner-actions">
                                <button
                                    class="action-btn sub-btn"
                                    on:click={onSubAsset}
                                >
                                    <span class="action-icon">↳</span> CREATE SUB-ASSET
                                </button>
                                <button
                                    class="action-btn nft-btn"
                                    on:click={onNft}
                                >
                                    <span class="action-icon">#</span> MINT NFT
                                </button>
                            </div>
                        {/if}
				{:else if activeTab === "MESSAGES"}
					{#if composeOpen}
						<div class="compose-pane" transition:fade={{ duration: 150 }}>
							<div class="compose-pane-header">
								<div class="compose-pane-title">Send Announcement on {asset.name}</div>
								<button class="close-btn" on:click={cancelCompose}>×</button>
							</div>
							<div class="compose-pane-body">
								<div class="compose-mode-bar">
									<button
										class="mode-btn"
										class:active={composeMode === "cid"}
										on:click={() => switchComposeMode("cid")}
										disabled={composeBroadcasting || composeSent || !!composePreview}
									>CID / Hash</button>
									<button
										class="mode-btn"
										class:active={composeMode === "short"}
										on:click={() => switchComposeMode("short")}
										disabled={composeBroadcasting || composeSent || !!composePreview}
									>Short Message</button>
								</div>

								{#if composeMode === "cid"}
									<div class="compose-field">
										<div class="compose-label-row">
											<label for="compose-ipfs">MESSAGE CID / HASH</label>
											<HelpHitbox title="Asset Messages">
												<p>Asset messages store a CID/hash reference on-chain, not the full package body.</p>
												<p>Create content in Content Library, then publish or link it before selecting that CID here.</p>
											</HelpHitbox>
										</div>
										<IpfsHashField id="compose-ipfs" bind:value={composeIpfsHash} disabled={composeBroadcasting || composeSent || !!composePreview} />
										<button class="compose-library-link" type="button" on:click={openContentLibrary}>
											Go to Content Library
										</button>
									</div>
								{:else}
									<div class="compose-field">
											<div class="compose-label-row">
												<label for="compose-short">SHORT MESSAGE</label>
												<HelpHitbox title="Short Messages">
													<p>Short messages are encoded into Commander&apos;s fixed 32-byte on-chain message frame, then broadcast as a 64-character hex payload.</p>
													<p>The codec auto-selects the best fit from dictionary, raw, 5-bit, and 6-bit modes to compress short public updates as efficiently as possible.</p>
													<p>Messages are public, permanent, and not encrypted. Commander restores a readable preview locally, but other wallets may only show the raw hex unless they support this format.</p>
													<p>Text prediction uses the same dictionary families as the codec, so suggestions are biased toward phrases that compress well. For anything longer or richer than a short status/update, use Content Library and send a CID instead.</p>
													<p>Active table pack: {activePackLabel()}{activePackFingerprintShort() ? ` (${activePackFingerprintShort()})` : ""}. Custom packs change local encode/decode tables only; both sender and receiver need the same pack to read custom-table messages correctly.</p>
												</HelpHitbox>
											</div>
										<div class="short-textarea-wrap"
											on:focusin={() => (composeShortFocused = true)}
											on:focusout={handleTextareaFocusOut}
										>
											<textarea
												id="compose-short"
												bind:this={composeShortTextarea}
												bind:value={composeShortText}
												on:input={queueEncodeShortMessage}
												placeholder="Type a short message..."
												rows="3"
												disabled={tablePackBusy || composeBroadcasting || composeSent || !!composePreview}
											></textarea>
											{#if autocompleteEnabled && !tablePackBusy && !composeBroadcasting && !composeSent && !composePreview}
												<ShortMessageAutocomplete
													text={composeShortText}
													disabled={tablePackBusy || composeBroadcasting || composeSent || !!composePreview}
													targetElement={composeShortTextarea}
													onAccept={handleShortSuggestion}
													focused={composeShortFocused}
													preferredDict={composeShortResult?.dictionary_index ?? null}
												/>
											{/if}
										</div>
										<div class="short-tools-row">
											<label class="autocomplete-toggle">
												<input type="checkbox" bind:checked={autocompleteEnabled} disabled={tablePackBusy} />
												<span>Enable text prediction</span>
											</label>
											{#if composeShortEmojis.length > 0}
												<div class="emoji-picker-wrap" bind:this={composeShortEmojiWrap}>
													<button
														class="emoji-toggle"
														type="button"
														aria-label="Open emoji picker"
														title="Emoji picker"
														on:click|stopPropagation={() => (composeShortEmojiOpen = !composeShortEmojiOpen)}
														disabled={tablePackBusy || composeBroadcasting || composeSent || !!composePreview}
													>
														☺
													</button>
													{#if composeShortEmojiOpen}
														<div class="short-emoji-picker">
															{#each composeShortEmojis as emoji}
																<button
																	type="button"
																	class="short-emoji-btn"
																	on:click={() => insertShortEmoji(emoji)}
																	disabled={tablePackBusy || composeBroadcasting || composeSent || !!composePreview}
																>
																	{emoji}
																</button>
															{/each}
														</div>
													{/if}
												</div>
											{/if}
											{#if composeShortResult?.dictionary_name}
												<span class="short-dict-chip">DICT {composeShortResult.dictionary_name}</span>
											{/if}
											<button class="table-pack-toggle" type="button" on:click={toggleTablePackPanel} disabled={tablePackBusy}>
												TABLES
											</button>
										</div>
										{#if tablePackPanelOpen}
											<div class="table-pack-panel">
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
														on:change={selectTablePack}
														disabled={tablePackBusy}
														value={tablePackSelectionFingerprint}
													>
														{#each tablePacks as pack}
															<option value={pack.fingerprint_sha256}>
																{tablePackLabel(pack)}
															</option>
														{/each}
													</select>
													<button type="button" on:click={exportOfficialTablePack} disabled={tablePackBusy}>EXPORT OFFICIAL</button>
													<button type="button" on:click={importTablePack} disabled={tablePackBusy}>IMPORT PACK</button>
													<button type="button" on:click={resetTablePack} disabled={tablePackBusy}>USE OFFICIAL</button>
													<button
														type="button"
														class="danger-action"
														on:click={deleteSelectedTablePack}
														disabled={tablePackBusy || !selectedTablePack() || selectedTablePack()?.builtin}
													>DELETE</button>
												</div>
												<div class="table-pack-note">
													Export the official JSON, edit it, then import it as a custom pack. Token 255 must stay empty in every dictionary.
												</div>
												{#if tablePackStatus}
													<div class="table-pack-status" class:busy={tablePackBusy}>{tablePackStatus}</div>
												{/if}
												{#if tablePackError}
													<div class="table-pack-error">{tablePackError}</div>
												{/if}
											</div>
										{/if}
										{#if composeShortEncoding}
											<div class="short-msg-status">Encoding...</div>
										{:else if composeShortResult}
											<div class="short-msg-status" class:fits={composeShortResult.fits} class:no-fits={!composeShortResult.fits}>
												{#if composeShortResult.fits}
													✓ Fits ({composeShortResult.encoded_payload_len} / 27 bytes used)
												{:else}
													✗ Does not fit ({composeShortResult.encoded_payload_len} / 27 bytes)
												{/if}
											</div>
											{#if composeShortResult.warnings.length > 0}
												<div class="short-msg-warnings">
													{#each composeShortResult.warnings as w}
														<div class="short-msg-warning">⚠ {w}</div>
													{/each}
												</div>
											{/if}
											{#if composeShortResult.fits}
												<div class="short-msg-preview">
													<span>Preview:</span> {composeShortResult.decoded_preview}
												</div>
												<div class="short-msg-hex-row">
													<button
														class="short-msg-hex-toggle"
														type="button"
														on:click={() => composeShortShowHex = !composeShortShowHex}
													>
														{composeShortShowHex ? "Hide Hex" : "Show Hex"}
													</button>
													{#if composeShortShowHex}
														<span class="short-msg-hex mono">{composeShortResult.hex}</span>
													{/if}
												</div>
											{/if}
										{/if}
									</div>
								{/if}

								<div class="compose-field">
									<label for="compose-expire">Expire Time (UTC timestamp, optional)</label>
									<input
										id="compose-expire"
										type="text"
										bind:value={composeExpireTime}
										placeholder="e.g. 1737500000"
										disabled={composeBroadcasting || composeSent || !!composePreview}
									/>
								</div>

								{#if composeError}
									<div class="compose-error">{composeError}</div>
								{/if}

								{#if composePreview}
									<div class="compose-preview">
										<div class="preview-label">Preview</div>
										<div class="preview-row">
											<span>Channel:</span> {composePreview.channel_name}
										</div>
										<div class="preview-row">
											<span>Content:</span>
											{#if composeMode === "short" && composeShortResult}
												{composeShortResult.decoded_preview}
												<span class="short-msg-preview-badge">SHORT MSG</span>
											{:else}
												{composePreview.ipfs_hash}
											{/if}
										</div>
										<div class="preview-row">
											<span>Ownership:</span> {composePreview.has_ownership ? 'Confirmed' : 'Not confirmed'}
										</div>
										{#if composePreview.warnings.length > 0}
											<div class="preview-warnings">
												{#each composePreview.warnings as w}
													<div class="preview-warning">⚠ {w}</div>
												{/each}
											</div>
										{/if}
									</div>
								{/if}

								{#if composeSent}
									<div class="compose-sent">Announcement broadcast!</div>
								{/if}

								<div class="compose-actions">
									{#if !composePreview && !composeSent}
										<button
											class="action-btn primary"
											on:click={previewAnnouncement}
											disabled={composePreviewing || composeShortEncoding || (composeMode === "short" ? !composeShortResult?.fits : !composeIpfsHash.trim())}
										>
											{composePreviewing ? "PREVIEWING..." : "PREVIEW"}
										</button>
									{:else if composePreview && !composeSent}
										<button
											class="action-btn primary"
											on:click={broadcastAnnouncement}
											disabled={composeBroadcasting}
										>
											{composeBroadcasting ? "BROADCASTING..." : "BROADCAST"}
										</button>
										<button class="action-btn" on:click={cancelCompose}>
											CANCEL
										</button>
									{:else if composeSent}
										<button class="action-btn" on:click={cancelCompose}>
											CLOSE
										</button>
									{/if}
								</div>
							</div>
						</div>
					{:else}
						<div class="messages-panel">
							<div class="messages-header">
								<div class="messages-title">MESSAGES</div>
								<div class="messages-actions">
									<button
										class="action-btn subscribe-btn"
										class:subscribed={isSubscribed}
										on:click={toggleSubscription}
										disabled={messagesLoading}
									>
										{isSubscribed ? "UNSUBSCRIBE" : "SUBSCRIBE"}
									</button>
									{#if asset.hasOwner}
										<button
											class="action-btn primary"
											on:click={() => (composeOpen = true)}
										>
											<span class="action-icon">✉</span> SEND
										</button>
									{/if}
								</div>
							</div>

							{#if messagesInfo && !messagesInfo.enabled}
								<div class="messages-status warn">
									{#if messagesInfo.warnings.length > 0}
										{messagesInfo.warnings[0]}
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
							{:else if filteredMessages.length === 0}
								<div class="messages-empty">
									No messages for this asset channel.
								</div>
							{:else}
								<div class="messages-list">
									{#each filteredMessages as msg (msg.asset_name + msg.time + msg.message)}
										{@const sm = shortMessageCache[msg.message]}
										<div class="message-entry" class:unread={msg.status === 'UNREAD'}>
											<div class="message-channel">{msg.asset_name}</div>
											{#if sm?.is_short_message}
												<div class="short-msg-badge">SHORT MSG</div>
												<div class="short-msg-text">{sm.text}</div>
												<div class="short-msg-raw">
													<IpfsReference hash={msg.message} compact={true} />
												</div>
											{:else}
												<div class="message-hash">
													<IpfsReference hash={msg.message} compact={true} />
												</div>
											{/if}
											<div class="message-meta">
												<span class="message-time">{msg.time}</span>
												<span class="message-block">Block {msg.block_height}</span>
												<span class="message-status" class:unread={msg.status === 'UNREAD'}>
													{msg.status}
												</span>
											</div>
											{#if msg.expire_time}
												<div class="message-expire">Expires: {msg.expire_time}</div>
											{/if}
										</div>
									{/each}
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
    .detail-grid {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.5rem;
        margin-bottom: 0.75rem;
    }
    .detail-stat {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        padding: 0.6rem 0.4rem;
        text-align: center;
    }
    .stat-label {
        font-size: 0.5rem;
        color: #555;
        letter-spacing: 0.5px;
        margin-bottom: 0.2rem;
    }
    .stat-value {
        font-size: 0.75rem;
        font-weight: 600;
        color: #fff;
        font-family: var(--font-mono);
    }
    .stat-value.neon-text {
        color: var(--color-primary);
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.5);
    }
    .action-btn.disabled {
        opacity: 0.5;
        cursor: not-allowed;
        filter: grayscale(1);
    }
    .action-btn.disabled:hover {
        background: rgba(255, 255, 255, 0.05);
        color: #fff;
        transform: none;
        box-shadow: none;
    }
    .stat-value.clickable {
        cursor: pointer;
        transition: all 0.2s;
        border: 1px solid transparent;
        border-radius: 4px;
        padding: 0 4px;
    }
    .stat-value.clickable:hover {
        background: rgba(255, 215, 0, 0.15);
        border-color: rgba(255, 215, 0, 0.3);
        transform: scale(1.05);
    }

    /* Actions */
    .detail-actions {
        display: flex;
        gap: 0.8rem;
        margin: 0 2rem;
    }
    .owner-actions {
        margin-top: 0.8rem;
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

    /* Metadata */
    .metadata-section {
        margin-top: 0.75rem;
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .meta-card {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 0.6rem 0.75rem;
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 0.25rem 0.75rem;
    }
    .meta-loading {
        color: #555;
        font-size: 0.7rem;
        text-align: center;
        letter-spacing: 1px;
        grid-column: 1 / -1;
    }
    .meta-row {
        display: flex;
        flex-direction: column;
        align-items: center;
        padding: 0.25rem 0;
    }
    .meta-label {
        font-size: 0.45rem;
        color: #555;
        letter-spacing: 0.5px;
        margin-bottom: 0.1rem;
    }
    .meta-value {
        font-size: 0.7rem;
        color: #aaa;
    }
    .meta-value.yes {
        color: var(--color-primary);
    }
    .ipfs-card {
        grid-template-columns: 1fr;
        gap: 0.4rem;
    }
    .ipfs-card.empty {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
    }
    .ipfs-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .ipfs-value {
        font-size: 0.65rem;
    }
    .ipfs-value :global(.ipfs-ref) {
        font-size: 0.65rem;
    }
    .ipfs-placeholder {
        font-size: 0.65rem;
        color: #555;
        font-style: italic;
    }
    .meta-update-btn {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 6px;
        padding: 0.25rem 0.5rem;
        color: var(--color-primary);
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
    }
    .meta-update-btn:hover {
        background: var(--color-primary);
        color: #000;
    }
    .meta-update-btn .action-icon {
        font-size: 0.7rem;
    }
    .ipfs-header-actions {
        display: flex;
        gap: 0.4rem;
        align-items: center;
    }
    .meta-view-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 6px;
        padding: 0.25rem 0.5rem;
        color: var(--color-primary);
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
    }
    .meta-view-btn:hover {
        background: rgba(0, 255, 65, 0.12);
        border-color: var(--color-primary);
    }
    .meta-view-btn .action-icon {
        font-size: 0.7rem;
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
        gap: 0.5rem;
    }
    .messages-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .messages-title {
        font-size: 0.7rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }
    .messages-actions {
        display: flex;
        gap: 0.5rem;
    }
    .subscribe-btn.subscribed {
        color: #ff5555;
        border-color: rgba(255, 85, 85, 0.4);
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
        gap: 0.4rem;
        max-height: 300px;
        overflow-y: auto;
    }
    .message-entry {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 8px;
        padding: 0.5rem 0.75rem;
    }
    .message-entry.unread {
        border-left: 3px solid var(--color-primary);
    }
    .message-channel {
        font-size: 0.65rem;
        font-weight: 600;
        color: var(--color-primary);
        margin-bottom: 0.15rem;
    }
    .message-hash {
        font-size: 0.55rem;
        color: #888;
        font-family: var(--font-mono);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        margin-bottom: 0.15rem;
    }
    .message-meta {
        display: flex;
        gap: 0.5rem;
        font-size: 0.5rem;
        color: #666;
    }
    .message-status.unread {
        color: var(--color-primary);
    }
    .message-expire {
        font-size: 0.48rem;
        color: #555;
        margin-top: 0.15rem;
    }

    .short-textarea-wrap textarea {
        background: #000;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.45rem 0.55rem;
        color: #e0e0e0;
        font-size: 0.72rem;
        line-height: 1.4;
        font-family: var(--font-mono);
        resize: vertical;
        width: 100%;
        outline: none;
        transition: border-color 0.15s;
    }
    .short-textarea-wrap textarea:focus {
        border-color: var(--color-primary);
    }
    .short-textarea-wrap textarea::placeholder {
        color: #555;
    }

    /* Opaque full-panel compose pane */
    .compose-pane {
        flex: 1;
        display: flex;
        flex-direction: column;
        background: rgba(8, 12, 10, 0.98);
        border: none;
        border-radius: 0;
        overflow: hidden;
    }
    .compose-pane-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.4rem 0.7rem;
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        flex-shrink: 0;
    }
    .compose-pane-title {
        font-size: 0.72rem;
        font-weight: 600;
        color: var(--color-primary);
        letter-spacing: 1.5px;
    }
    .compose-pane-body {
        flex: 1;
        overflow-y: auto;
        overflow-x: hidden;
        padding: 0.75rem 1rem;
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .compose-pane-body::-webkit-scrollbar {
        width: 8px;
    }
    .compose-pane-body::-webkit-scrollbar-track {
        background: transparent;
    }
    .compose-pane-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 4px;
    }
    .compose-pane-body::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.55);
    }

    /* Mode toggle bar — cleaner pill-style buttons */
    .compose-mode-bar {
        display: flex;
        gap: 0.4rem;
        flex-shrink: 0;
    }
    .mode-btn {
        padding: 0.3rem 0.6rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #888;
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .mode-btn:hover:not(:disabled) {
        border-color: rgba(255, 255, 255, 0.2);
        color: #aaa;
    }
    .mode-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .mode-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .compose-field {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .compose-field label {
        font-size: 0.62rem;
        color: #666;
        letter-spacing: 0.5px;
    }
    .compose-label-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .compose-field input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.4rem 0.5rem;
        color: #fff;
        font-size: 0.72rem;
        font-family: var(--font-mono);
    }
    .compose-field input:focus {
        outline: none;
        border-color: var(--color-primary);
    }
    .compose-field input::placeholder {
        color: #555;
    }
    .compose-library-link {
        align-self: flex-start;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        border-radius: 6px;
        padding: 0.35rem 0.55rem;
        font-size: 0.62rem;
        cursor: pointer;
        margin-bottom: 0.2rem;
    }
    .compose-library-link:hover {
        background: rgba(0, 255, 65, 0.14);
    }

    /* Short message */
    .short-msg-status {
        font-size: 0.62rem;
        padding: 0.3rem;
        border-radius: 6px;
    }
    .short-msg-status.fits {
        color: #b9ffd0;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
    }
    .short-msg-status.no-fits {
        color: #ff8888;
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.2);
    }
    .short-msg-status:not(.fits):not(.no-fits) {
        color: #888;
        font-style: italic;
    }
    .short-tools-row {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 0.45rem;
        margin-top: 0.1rem;
    }
    .emoji-picker-wrap {
        position: relative;
        display: inline-flex;
    }
    .emoji-toggle {
        width: 1.75rem;
        height: 1.55rem;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 5px;
        color: var(--color-primary);
        cursor: pointer;
        font-size: 0.88rem;
        font-weight: 600;
        padding: 0;
        transition: all 0.15s;
    }
    .emoji-toggle:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.12);
        border-color: rgba(0, 255, 65, 0.35);
    }
    .emoji-toggle:disabled {
        cursor: not-allowed;
        opacity: 0.45;
    }
    .short-dict-chip {
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 999px;
        color: #8cff9f;
        font-family: var(--font-mono);
        font-size: 0.5rem;
        letter-spacing: 0.5px;
        padding: 0.16rem 0.42rem;
    }
    .table-pack-toggle {
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        background: rgba(0, 255, 65, 0.04);
        color: #8cff9f;
        cursor: pointer;
        font-family: var(--font-mono);
        font-size: 0.5rem;
        font-weight: 700;
        letter-spacing: 0.8px;
        padding: 0.22rem 0.5rem;
    }
    .table-pack-toggle:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.32);
    }
    .table-pack-toggle:disabled {
        cursor: not-allowed;
        opacity: 0.5;
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
    .table-pack-controls button.danger-action {
        border-color: rgba(255, 84, 84, 0.32);
        color: #ff8080;
    }
    .table-pack-controls button.danger-action:hover:not(:disabled) {
        background: rgba(255, 84, 84, 0.1);
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
    .short-emoji-picker {
        position: absolute;
        left: 0;
        top: calc(100% + 0.35rem);
        z-index: 30;
        width: min(18rem, 70vw);
        max-height: 9.5rem;
        overflow-y: auto;
        display: flex;
        flex-wrap: wrap;
        gap: 0.25rem;
        background: rgba(8, 12, 10, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 6px;
        padding: 0.35rem;
        box-shadow:
            0 10px 28px rgba(0, 0, 0, 0.65),
            0 0 0 1px rgba(0, 255, 65, 0.08);
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .short-emoji-btn {
        width: 1.9rem;
        height: 1.8rem;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 5px;
        cursor: pointer;
        font-size: 1rem;
        line-height: 1;
        transition: all 0.12s;
    }
    .short-emoji-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.12);
        border-color: rgba(0, 255, 65, 0.35);
    }
    .short-emoji-btn:disabled {
        cursor: not-allowed;
        opacity: 0.45;
    }
    .short-msg-warnings {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
    }
    .short-msg-warning {
        font-size: 0.55rem;
        color: #ffaa00;
    }
    .short-msg-preview {
        font-size: 0.62rem;
        color: #ccc;
    }
    .short-msg-preview span {
        color: #666;
        font-size: 0.56rem;
        letter-spacing: 0.5px;
    }
    .short-msg-hex-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
    }
    .short-msg-hex-toggle {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        padding: 0.25rem 0.4rem;
        color: #888;
        font-size: 0.55rem;
        cursor: pointer;
        transition: all 0.15s;
    }
    .short-msg-hex-toggle:hover {
        border-color: rgba(255, 255, 255, 0.2);
        color: #ccc;
    }
    .short-msg-hex {
        font-size: 0.55rem;
        color: var(--color-primary);
        background: rgba(0, 0, 0, 0.2);
        padding: 0.15rem 0.35rem;
        border-radius: 4px;
        border: 1px solid rgba(0, 255, 65, 0.15);
    }
    .short-msg-preview-badge {
        display: inline-block;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        font-size: 0.5rem;
        font-weight: 600;
        padding: 0.15rem 0.35rem;
        border-radius: 4px;
        margin-left: 0.4rem;
        letter-spacing: 0.5px;
    }
    .short-msg-badge {
        font-size: 0.5rem;
        font-weight: 600;
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        padding: 0.15rem 0.35rem;
        letter-spacing: 0.5px;
        display: inline-block;
        margin-bottom: 0.2rem;
    }
    .short-msg-text {
        font-size: 0.72rem;
        color: #ddd;
        margin-bottom: 0.2rem;
        line-height: 1.35;
    }
    .short-msg-raw {
        margin-top: 0.15rem;
        opacity: 0.7;
    }
    .compose-error {
        font-size: 0.62rem;
        color: #ff5555;
        padding: 0.3rem;
        background: rgba(255, 85, 85, 0.1);
        border-radius: 4px;
    }
    .compose-preview {
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 0.5rem;
    }
    .preview-label {
        font-size: 0.5rem;
        color: #555;
        letter-spacing: 1px;
        margin-bottom: 0.3rem;
    }
    .preview-row {
        font-size: 0.6rem;
        color: #aaa;
        margin-bottom: 0.1rem;
        font-family: var(--font-mono);
    }
    .preview-row span {
        color: #666;
    }
    .preview-warnings {
        margin-top: 0.3rem;
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
    }
    .preview-warning {
        font-size: 0.5rem;
        color: #ffaa00;
    }
    .compose-sent {
        font-size: 0.65rem;
        color: var(--color-primary);
        text-align: center;
        padding: 0.5rem;
    }
    .compose-actions {
        display: flex;
        gap: 0.5rem;
        justify-content: flex-end;
        margin-top: 0.5rem;
        flex-shrink: 0;
    }
    .compose-actions .action-btn {
        flex: none;
        padding: 0.4rem 0.8rem;
    }
    .autocomplete-toggle {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        font-size: 0.6rem;
        color: #666;
        margin-top: 0.15rem;
        cursor: pointer;
        letter-spacing: 0.3px;
        user-select: none;
    }
    .autocomplete-toggle input {
        accent-color: var(--color-primary);
        width: 0.7rem;
        height: 0.7rem;
        cursor: pointer;
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
    .short-textarea-wrap {
        position: relative;
        display: flex;
        flex-direction: column;
    }
    .autocomplete-toggle {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        font-size: 0.6rem;
        color: #666;
        margin-top: 0.15rem;
        cursor: pointer;
        letter-spacing: 0.3px;
        user-select: none;
    }
    .autocomplete-toggle input {
        accent-color: var(--color-primary);
        width: 0.7rem;
        height: 0.7rem;
        cursor: pointer;
    }
</style>
