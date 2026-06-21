<script>
    import { fade } from "svelte/transition";
    import { createEventDispatcher, tick } from "svelte";
    import { core } from "@tauri-apps/api";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { insertSuggestion } from "../../utils.js";
    import "../../../components.css";
    import HelpHitbox from "../HelpHitbox.svelte";
    import IpfsHashField from "../IpfsHashField.svelte";
    import ShortMessageAutocomplete from "../ShortMessageAutocomplete.svelte";
    import { addNotification } from "../../stores/notifications.js";
    import { ensureNodeSyncedForBroadcast } from "../../utils/nodeSync.js";
    import TablePackPanel from "./TablePackPanel.svelte";
    import WalletUnlockModal from "../WalletUnlockModal.svelte";

    const dispatch = createEventDispatcher();


    export let channelName = "";

    // Compose state
    let composeIpfsHash = "";
    let composeExpireTime = "";
    let composeExpireDateInput = "";
    let composeExpireTimeInput = "";
    /** @type {any} */
    let composePreview = null;
    let composePreviewFee = "";
    let composeError = "";
    let composePreviewing = false;
    let composeBroadcasting = false;
    /** @type {"cid"|"short"} */
    let composeMode = "cid";
    let composeShortText = "";
    /** @type {any} */
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

    // Table pack state
    /** @type {any} */
    let activeTablePack = null;
    /** @type {any[]} */
    let tablePacks = [];
    let tablePackPanelOpen = false;
    let tablePackBusy = false;
    let tablePackError = "";
    let tablePackStatus = "";
    let tablePackSelectionFingerprint = "";
    let preparedTablePackFingerprint = "";
    /** @type {ReturnType<typeof setTimeout> | null} */
    let tablePackStatusTimer = null;

    // Unlock modal state
    let showMessageUnlockModal = false;
    let messageUnlockPassword = "";
    let messageUnlocking = false;
    let messageUnlockError = "";

    /** @param {any} pack */
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

    /** @param {any} pack */
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
            const emojis = await core.invoke("short_message_emojis");
            composeShortEmojis = Array.isArray(emojis) ? emojis : [];
            preparedTablePackFingerprint = activePackFingerprint();
        } catch (err) {
            // The next encode/suggestion request will surface any real issue.
        }
    }

    async function ensureShortMessagePackPrepared() {
        const activeFingerprint = activePackFingerprint();
        if (tablePackBusy || (activeFingerprint && preparedTablePackFingerprint === activeFingerprint)) return;
        await showTablePackBusy("Preparing text prediction...");
        await prepareShortMessagePack();
        finishTablePackBusy();
    }

    function refreshShortMessageAfterPackChange() {
        setTimeout(() => {
            queueEncodeShortMessage();
        }, 0);
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

    function requestMessageWalletUnlock() {
        messageUnlockPassword = "";
        messageUnlockError = "";
        showMessageUnlockModal = true;
        composeError = "Wallet unlock required before broadcasting this message.";
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
            activeTablePack = /** @type {any} */ (status);
            tablePackSelectionFingerprint = activeTablePack?.active?.fingerprint_sha256 || "";
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
            activeTablePack = /** @type {any} */ (status);
            tablePacks = Array.isArray(packs) ? /** @type {any[]} */ (packs) : [];
            tablePackSelectionFingerprint = activeTablePack?.active?.fingerprint_sha256 || "";
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

    /** @param {string} fingerprint */
    async function selectTablePack(fingerprint) {
        const pack = tablePacks.find((p) => p.fingerprint_sha256 === fingerprint);
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
                setActivePackSummary(/** @type {any} */ (selected));
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
                setActivePackSummary(/** @type {any} */ (selected));
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
                setActivePackSummary(/** @type {any} */ (selected));
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

    /** @param {"cid"|"short"} mode */
    async function switchComposeMode(mode) {
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
        await refreshActiveTablePack();
        await ensureShortMessagePackPrepared();
        refreshShortMessageAfterPackChange();
    }

    /** @param {string} emoji */
    function insertShortEmoji(emoji) {
        if (!emoji || composeBroadcasting || composePreview) return;
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
        const target = event.target instanceof Node ? event.target : null;
        if (composeShortEmojiOpen && composeShortEmojiWrap && (!target || !composeShortEmojiWrap.contains(target))) {
            composeShortEmojiOpen = false;
        }
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
            composeShortResult = /** @type {any} */ (result);
            if (composeShortResult.fits) {
                composeIpfsHash = composeShortResult.hex;
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

    /** @param {number} n */
    function pad(n) { return String(n).padStart(2, "0"); }

    function parseComposeExpireTime() {
        const raw = composeExpireTime.trim();
        if (!raw) return null;
        if (!/^\d+$/.test(raw)) {
            composeError = "Expire time must be a valid future date and time.";
            return undefined;
        }
        const parsed = Number(raw);
        const now = Math.floor(Date.now() / 1000);
        if (!Number.isSafeInteger(parsed) || parsed <= now) {
            composeError = "Expire time must be in the future.";
            return undefined;
        }
        return parsed;
    }

    async function previewAnnouncement() {
        const announcementChannel = channelName.trim();
        if (!announcementChannel) {
            composeError = "Asset message channel is unavailable. Close and reopen the asset, then try again.";
            return;
        }
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
        composePreviewFee = "";
        try {
            await ensureNodeSyncedForBroadcast();
            const ipfsHash = composeMode === "short" && composeShortResult
                ? composeShortResult.hex
                : composeIpfsHash.trim();
            const [preview, feeEstimate] = await Promise.all([
                core.invoke("preview_send_announcement", {
                    channelName: announcementChannel,
                    ipfsHash,
                    expireTime,
                }),
                core.invoke("estimate_announcement_fee").catch(() => null),
            ]);
            composePreview = /** @type {any} */ (preview);
            composePreviewFee = feeEstimate || "";
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
            await ensureNodeSyncedForBroadcast();
            const channelName = composePreview.channel_name;
            const ipfsHash = composeMode === "short" && composeShortResult
                ? composeShortResult.hex
                : composeIpfsHash.trim();
            const txid = await core.invoke("send_announcement", {
                channelName,
                ipfsHash,
                expireTime,
            });
            try {
                await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Broadcasted",
                        operation_type: "asset_message",
                        summary: `Send message on ${channelName}`,
                        txid,
                        details: {
                            channel_name: channelName,
                            asset_name: channelName,
                            message_payload: ipfsHash,
                            expire_time: expireTime,
                            mode: composeMode,
                            short_message_preview: composeMode === "short" ? composeShortResult?.decoded_preview ?? null : null,
                        },
                    },
                });
            } catch (journalErr) {
                console.warn("Failed to record asset message journal entry:", journalErr);
            }
            addNotification({
                type: "message",
                severity: "success",
                title: "Announcement Sent",
                body: `Message sent on channel ${channelName}`,
                action: { label: "Copy TXID", txid },
            });
            dispatch("sent");
            cancelCompose();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                requestMessageWalletUnlock();
                return;
            }
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

    async function unlockAndBroadcastAnnouncement() {
        if (!messageUnlockPassword.trim() || messageUnlocking) return;
        messageUnlocking = true;
        messageUnlockError = "";
        try {
            await core.invoke("wallet_unlock", { password: messageUnlockPassword, duration: 300 });
            messageUnlockPassword = "";
            showMessageUnlockModal = false;
            composeError = "";
            await broadcastAnnouncement();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                messageUnlockError = "Wallet unlock failed. Check the passphrase and try again.";
                showMessageUnlockModal = true;
            } else {
                messageUnlockError = messageRpcError(err);
            }
        } finally {
            messageUnlocking = false;
        }
    }

    function cancelCompose() {
        composePreview = null;
        composeError = "";
        showMessageUnlockModal = false;
        messageUnlockPassword = "";
        messageUnlockError = "";
        dispatch("close");
    }

    function cancelComposePreview() {
        composePreview = null;
        composeError = "";
    }

    /** @param {Date} date */
    function formatDateMMDDYYYY(date) {
        return `${pad(date.getMonth() + 1)}/${pad(date.getDate())}/${date.getFullYear()}`;
    }

    /** @param {string} raw */
    function formatDateInputMMDDYYYY(raw) {
        const digits = raw.replace(/\D/g, "").slice(0, 8);
        if (digits.length <= 2) return digits;
        if (digits.length <= 4) return `${digits.slice(0, 2)}/${digits.slice(2)}`;
        return `${digits.slice(0, 2)}/${digits.slice(2, 4)}/${digits.slice(4)}`;
    }

    /** @param {Event} event */
    function handleExpireDateInput(event) {
        const target = /** @type {HTMLInputElement | null} */ (event.currentTarget);
        composeExpireDateInput = formatDateInputMMDDYYYY(target?.value || "");
        validateExpireInputs();
    }

    /** @param {Date} date */
    function formatTimeHHMM(date) {
        return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
    }

    /** @param {string} str */
    function parseMMDDYYYY(str) {
        const parts = str.trim().split("/");
        if (parts.length !== 3) return null;
        const month = parseInt(parts[0], 10) - 1;
        const day = parseInt(parts[1], 10);
        const year = parseInt(parts[2], 10);
        const date = new Date(year, month, day);
        if (isNaN(date.getTime())) return null;
        if (date.getMonth() !== month || date.getDate() !== day || date.getFullYear() !== year) return null;
        return date;
    }

    /** @param {string} str */
    function parseHHMM(str) {
        const parts = str.trim().split(":");
        if (parts.length !== 2) return null;
        const hours = parseInt(parts[0], 10);
        const minutes = parseInt(parts[1], 10);
        if (hours < 0 || hours > 23 || minutes < 0 || minutes > 59 || !Number.isFinite(hours) || !Number.isFinite(minutes)) return null;
        return { hours, minutes };
    }

    function validateExpireInputs() {
        const dateStr = composeExpireDateInput.trim();
        const timeStr = composeExpireTimeInput.trim();
        if (!dateStr && !timeStr) {
            composeExpireTime = "";
            composeError = "";
            return;
        }
        if (!dateStr) {
            composeExpireTime = "";
            composeError = "Date is required. Use mm/dd/yyyy.";
            return;
        }
        const date = parseMMDDYYYY(dateStr);
        if (!date) {
            composeExpireTime = "";
            composeError = "Invalid date. Use mm/dd/yyyy.";
            return;
        }
        const time = parseHHMM(timeStr || "00:00");
        if (!time) {
            composeExpireTime = "";
            composeError = "Invalid time. Use HH:MM (24h).";
            return;
        }
        date.setHours(time.hours, time.minutes, 0, 0);
        const ms = date.getTime();
        const now = Date.now();
        if (!Number.isFinite(ms) || ms <= now) {
            composeExpireTime = "";
            composeError = "Expire time must be in the future.";
            return;
        }
        composeExpireTime = String(Math.floor(ms / 1000));
        composeError = "";
    }

    /** @param {number} seconds */
    function setComposeExpireOffset(seconds) {
        const date = new Date(Date.now() + seconds * 1000);
        composeExpireDateInput = formatDateMMDDYYYY(date);
        composeExpireTimeInput = formatTimeHHMM(date);
        validateExpireInputs();
    }

    function clearComposeExpire() {
        composeExpireDateInput = "";
        composeExpireTimeInput = "";
        composeExpireTime = "";
        composeError = "";
    }

    function openContentLibrary() {
        window.dispatchEvent(new CustomEvent("commander-open-content-library"));
        cancelCompose();
    }

    /** @param {CustomEvent<{ fingerprint?: string }>} event */
    function handleSelectPack(event) {
        const detail = event.detail;
        if (detail && detail.fingerprint) {
            selectTablePack(detail.fingerprint);
        }
    }
</script>

<svelte:window on:click={handleWindowClick} />

<div class="compose-pane" transition:fade={{ duration: 150 }}>
        <div class="compose-pane-header">
            <div class="compose-pane-title">Send Announcement</div>
            <button class="close-btn" on:click={cancelCompose}>×</button>
        </div>
        <div class="compose-pane-body">
            <div class="compose-mode-bar">
                <button
                    class="mode-btn"
                    class:active={composeMode === "cid"}
                    on:click={() => switchComposeMode("cid")}
                    disabled={composeBroadcasting || !!composePreview}
                >CID / Hash</button>
                <button
                    class="mode-btn"
                    class:active={composeMode === "short"}
                    on:click={() => switchComposeMode("short")}
                    disabled={composeBroadcasting || !!composePreview}
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
                    <IpfsHashField id="compose-ipfs" bind:value={composeIpfsHash} disabled={composeBroadcasting || !!composePreview} />
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
                            disabled={tablePackBusy || composeBroadcasting || !!composePreview}
                        ></textarea>
                        {#if autocompleteEnabled && !tablePackBusy && !composeBroadcasting && !composePreview}
                            <ShortMessageAutocomplete
                                text={composeShortText}
                                disabled={tablePackBusy || composeBroadcasting || !!composePreview}
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
                                    disabled={tablePackBusy || composeBroadcasting || !!composePreview}
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
                                                disabled={tablePackBusy || composeBroadcasting || !!composePreview}
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
                        <TablePackPanel
                            packs={tablePacks}
                            selectionFingerprint={tablePackSelectionFingerprint}
                            selectedPack={selectedTablePack()}
                            busy={tablePackBusy}
                            error={tablePackError}
                            status={tablePackStatus}
                            activePackLabel={activePackLabel()}
                            activePackFingerprint={activePackFingerprint()}
                            activePackFingerprintShort={activePackFingerprintShort()}
                            activePackStatusTitle={activePackStatusTitle()}
                            on:selectPack={handleSelectPack}
                            on:export={exportOfficialTablePack}
                            on:import={importTablePack}
                            on:reset={resetTablePack}
                            on:delete={deleteSelectedTablePack}
                        />
                    {:else if tablePackStatus}
                        <div class="table-pack-status standalone" class:busy={tablePackBusy}>{tablePackStatus}</div>
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
                <div class="compose-label-row">
                    <label for="compose-expire-date">EXPIRES</label>
                    <HelpHitbox title="Message Expiry">
                        <p>Expiry is optional metadata stored with the on-chain message. It tells wallets the message should be treated as valid until that UTC time.</p>
                        <p>The transaction itself stays on-chain. Expiry does not delete the message from blockchain history.</p>
                        <p>Leave this blank for a normal persistent announcement. Use expiry for temporary notices such as short-term status, event, or service updates.</p>
                    </HelpHitbox>
                </div>
                <div class="expire-control-row">
                    <input
                        id="compose-expire-date"
                        class="expire-date-text"
                        type="text"
                        value={composeExpireDateInput}
                        on:input={handleExpireDateInput}
                        on:change={validateExpireInputs}
                        on:keydown={(e) => {
                            if (e.key === "Escape") {
                                e.stopPropagation();
                                e.currentTarget.blur();
                            }
                        }}
                        placeholder="mm/dd/yyyy"
                        disabled={composeBroadcasting || !!composePreview}
                        aria-label="Expire date"
                    />
                    <input
                        class="expire-time-text"
                        type="text"
                        bind:value={composeExpireTimeInput}
                        on:input={validateExpireInputs}
                        on:change={validateExpireInputs}
                        on:keydown={(e) => {
                            if (e.key === "Escape") {
                                e.stopPropagation();
                                e.currentTarget.blur();
                            }
                        }}
                        placeholder="HH:MM"
                        disabled={composeBroadcasting || !!composePreview}
                        aria-label="Expire time"
                    />
                    <button type="button" on:click={() => setComposeExpireOffset(86400)} disabled={composeBroadcasting || !!composePreview}>+1D</button>
                    <button type="button" on:click={() => setComposeExpireOffset(604800)} disabled={composeBroadcasting || !!composePreview}>+7D</button>
                    <button type="button" on:click={() => setComposeExpireOffset(2592000)} disabled={composeBroadcasting || !!composePreview}>+30D</button>
                    <button type="button" on:click={clearComposeExpire} disabled={composeBroadcasting || !!composePreview || (!composeExpireDateInput && !composeExpireTimeInput)}>CLEAR</button>
                </div>
                <div class="expire-note" class:has-expiry={composeExpireTime}>
                    {#if composeExpireTime}
                        Sends UTC timestamp {composeExpireTime}. Leave blank for no expiry.
                    {:else}
                        No expiry set.
                    {/if}
                </div>
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
                    {#if composePreview.expire_time}
                        <div class="preview-row">
                            <span>Expires:</span> {composePreview.expire_time}
                        </div>
                    {/if}
                    {#if composePreviewFee}
                        <div class="preview-row">
                            <span>Fee (est):</span> {composePreviewFee} HEMP
                        </div>
                    {/if}
                    {#if (composePreview.warnings || []).length > 0}
                        <div class="preview-warnings">
                            {#each composePreview.warnings || [] as w}
                                <div class="preview-warning">⚠ {w}</div>
                            {/each}
                        </div>
                    {/if}
                </div>
            {/if}

            <div class="compose-actions">
                {#if !composePreview}
                    <button
                        class="action-btn primary"
                        on:click={previewAnnouncement}
                        disabled={composePreviewing || composeShortEncoding || (composeMode === "short" ? !composeShortResult?.fits : !composeIpfsHash.trim())}
                    >
                        {composePreviewing ? "PREVIEWING..." : "PREVIEW"}
                    </button>
                {:else}
                    <button
                        class="action-btn primary"
                        on:click={broadcastAnnouncement}
                        disabled={composeBroadcasting}
                    >
                        {composeBroadcasting ? "BROADCASTING..." : "BROADCAST"}
                    </button>
                    <button class="action-btn" on:click={cancelComposePreview}>
                        CANCEL
                    </button>
                {/if}
            </div>
        </div>
    </div>

<WalletUnlockModal
    show={showMessageUnlockModal}
    bind:password={messageUnlockPassword}
    unlocking={messageUnlocking}
    error={messageUnlockError}
    title="UNLOCK WALLET"
    body="Your wallet is locked. Commander will unlock it for 5 minutes to broadcast this asset message. Lock the wallet again when you are done sending transactions."
    confirmLabel="UNLOCK AND BROADCAST"
    on:cancel={() => { showMessageUnlockModal = false; messageUnlockError = ""; }}
    on:confirm={unlockAndBroadcastAnnouncement}
/>

<style>
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

    .compose-pane {
        flex: 1;
        display: flex;
        flex-direction: column;
        background: rgba(2, 4, 3, 0.98);
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
    .compose-pane-body::-webkit-scrollbar { width: 8px; }
    .compose-pane-body::-webkit-scrollbar-track { background: transparent; }
    .compose-pane-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 4px;
    }
    .compose-pane-body::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.55);
    }

    .compose-mode-bar { display: flex; gap: 0.4rem; flex-shrink: 0; }
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
    .mode-btn:hover:not(:disabled) { border-color: rgba(255, 255, 255, 0.2); color: #aaa; }
    .mode-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .mode-btn:disabled { opacity: 0.4; cursor: not-allowed; }

    .compose-field { display: flex; flex-direction: column; gap: 0.25rem; }
    .compose-field label { font-size: 0.62rem; color: #666; letter-spacing: 0.5px; }
    .compose-label-row { display: flex; align-items: center; gap: 0.5rem; }
    .compose-field input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.4rem 0.5rem;
        color: #fff;
        font-size: 0.72rem;
        font-family: var(--font-mono);
    }
    .compose-field input:focus { outline: none; border-color: var(--color-primary); }
    .compose-field input::placeholder { color: #555; }

    .expire-control-row {
        display: grid;
        grid-template-columns: 1fr auto repeat(4, auto);
        gap: 0.4rem;
        align-items: center;
    }
    .expire-control-row .expire-date-text,
    .expire-control-row .expire-time-text {
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 6px;
        color: #e8f5e9;
        cursor: text;
        font-family: var(--font-mono);
        font-size: 0.72rem;
        min-height: 2rem;
        padding: 0.35rem 0.5rem;
        width: 100%;
    }
    .expire-control-row .expire-date-text:focus,
    .expire-control-row .expire-time-text:focus {
        border-color: var(--color-primary);
        box-shadow: 0 0 0 1px rgba(0, 255, 65, 0.15);
        outline: none;
    }
    .expire-control-row .expire-date-text::placeholder,
    .expire-control-row .expire-time-text::placeholder { color: #555; }
    .expire-control-row .expire-date-text:disabled,
    .expire-control-row .expire-time-text:disabled { cursor: not-allowed; opacity: 0.5; }
    .expire-control-row button {
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 6px;
        color: var(--color-primary);
        cursor: pointer;
        font-family: var(--font-mono);
        font-size: 0.58rem;
        font-weight: 700;
        min-height: 2rem;
        padding: 0.35rem 0.5rem;
        white-space: nowrap;
    }
    .expire-control-row button:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.12);
        border-color: rgba(0, 255, 65, 0.35);
    }
    .expire-control-row button:disabled { cursor: not-allowed; opacity: 0.45; }
    .expire-note { color: #777; font-size: 0.56rem; line-height: 1.35; }
    .expire-note.has-expiry { color: #9cffad; }

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
    .compose-library-link:hover { background: rgba(0, 255, 65, 0.14); }

    .short-msg-status { font-size: 0.62rem; padding: 0.3rem; border-radius: 6px; }
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
    .short-msg-status:not(.fits):not(.no-fits) { color: #888; font-style: italic; }

    .short-tools-row { display: flex; align-items: center; flex-wrap: wrap; gap: 0.45rem; margin-top: 0.1rem; }
    .emoji-picker-wrap { position: relative; display: inline-flex; }
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
    .emoji-toggle:disabled { cursor: not-allowed; opacity: 0.45; }

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
    .table-pack-toggle:disabled { cursor: not-allowed; opacity: 0.5; }

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
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 6px;
        padding: 0.35rem;
        box-shadow: 0 10px 28px rgba(0, 0, 0, 0.65), 0 0 0 1px rgba(0, 255, 65, 0.08);
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
    .short-emoji-btn:disabled { cursor: not-allowed; opacity: 0.45; }

    .short-msg-warnings { display: flex; flex-direction: column; gap: 0.15rem; }
    .short-msg-warning { font-size: 0.55rem; color: #ffaa00; }
    .short-msg-preview { font-size: 0.68rem; color: #d8d8d8; }
    .short-msg-preview span { color: #777; font-size: 0.6rem; letter-spacing: 0.5px; }
    .short-msg-hex-row { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
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
    .short-msg-hex-toggle:hover { border-color: rgba(255, 255, 255, 0.2); color: #ccc; }
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
        border-radius: 6px;
        padding: 0.5rem;
    }
    .preview-label { font-size: 0.62rem; color: #6f766f; letter-spacing: 1px; margin-bottom: 0.35rem; }
    .preview-row { font-size: 0.72rem; color: #c9c9c9; margin-bottom: 0.16rem; font-family: var(--font-mono); line-height: 1.45; }
    .preview-row span { color: #7f877f; }
    .preview-warnings { margin-top: 0.4rem; display: flex; flex-direction: column; gap: 0.22rem; }
    .preview-warning { font-size: 0.68rem; color: #ffaa00; line-height: 1.45; }

    .compose-actions {
        display: flex;
        gap: 0.5rem;
        justify-content: flex-end;
        margin-top: 0.5rem;
        flex-shrink: 0;
    }
    .compose-actions .action-btn { flex: none; padding: 0.4rem 0.8rem; }

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
    .autocomplete-toggle input { accent-color: var(--color-primary); width: 0.7rem; height: 0.7rem; cursor: pointer; }

    .short-textarea-wrap { position: relative; display: flex; flex-direction: column; }

    @media (max-width: 760px) {
        .expire-control-row { grid-template-columns: 1fr auto; }
        .expire-control-row .expire-date-text,
        .expire-control-row .expire-time-text { grid-column: auto; }
        .expire-control-row button { grid-column: auto; }
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

    .action-btn {
        flex: 1;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
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
    .action-btn:hover { border-color: var(--color-primary); color: var(--color-primary); }
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

    .table-pack-status.standalone {
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
    .table-pack-status.standalone.busy::before {
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
    @keyframes table-pack-spin { to { transform: rotate(360deg); } }
</style>
