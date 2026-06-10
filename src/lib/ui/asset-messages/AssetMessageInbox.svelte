<script>
    import { createEventDispatcher, tick } from "svelte";
    import { core } from "@tauri-apps/api";
    import { open } from "@tauri-apps/plugin-dialog";
    import TablePackPanel from "./TablePackPanel.svelte";
    import AssetMessageDetail from "./AssetMessageDetail.svelte";
    import CommanderLoader from "../CommanderLoader.svelte";

    /**
     * @typedef {{
     *   name: string;
     *   balance?: number|string;
     *   hasOwner?: boolean;
     *   isSubAsset?: boolean;
     * }} AssetDetail
     * @typedef {{
     *   enabled?: boolean,
     *   warnings?: string[]
     * }} MessagesInfo
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     *   txid?: string;
     *   channel?: string;
     *   authority_asset?: string;
     *   authority_address?: string;
     *   block_hash?: string;
     *   sender_address?: string;
     * }} AssetMessage
     * @typedef {{
     *   is_short_message?: boolean;
     *   text?: string;
     *   warnings?: string[];
     * }} ShortMessageDecodeResult
     * @typedef {{
     *   name: string,
     *   version: string,
     *   fingerprint_sha256: string,
     *   origin?: "builtin"|"custom",
     *   active?: boolean,
     *   builtin?: boolean,
     *   path?: string|null
     * }} TablePackSummary
     */

    /** @type {AssetDetail} */
    export let asset;
    /** @type {AssetMessage[]} */
    export let messages = [];
    /** @type {MessagesInfo | null} */
    export let messagesInfo = null;
    export let messagesLoading = false;
    export let messagesError = "";
    export let isSubscribed = false;

    const dispatch = createEventDispatcher();

    // Short message decode cache
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
    /** @type {Set<string>} */
    let selectedMessageIds = new Set();

    /** @type {null | 'clearUnpinned' | 'deleteSelected'} */
    let pendingConfirmAction = null;

    // Table pack state
    let inboxTablePackPanelOpen = false;
    let inboxTablePackBusy = false;
    let inboxTablePackStatus = "";
    let inboxTablePackError = "";
    /** @type {any} */
    let activeTablePack = null;
    /** @type {TablePackSummary[]} */
    let tablePacks = [];
    let tablePackSelectionFingerprint = "";
    let preparedTablePackFingerprint = "";
    /** @type {ReturnType<typeof setTimeout> | null} */
    let tablePackStatusTimer = null;

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

    loadMessageLocalState();

    function loadMessageLocalState() {
        pinnedMessageIds = loadMessageIdSet(PINNED_MESSAGES_KEY);
        hiddenMessageIds = loadMessageIdSet(HIDDEN_MESSAGES_KEY);
        readMessageIds = loadMessageIdSet(READ_MESSAGES_KEY);
    }

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
        dispatch("refresh");
    }

    function toggleExplorer() {
        messageExplorerMode = !messageExplorerMode;
    }

    function openCompose() {
        dispatch("compose");
    }

    function toggleSubscription() {
        dispatch("subscriptionToggle");
    }

    function openH0xc() {
        dispatch("openH0xc");
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

    $: assetChannelNames = asset ? channelNamesForAsset(asset.name) : [];
    $: filteredMessages = messages.filter((msg) =>
        assetChannelNames.includes(msg.asset_name),
    );

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

    // ---- Table pack panel ----
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
        inboxTablePackBusy = true;
        inboxTablePackError = "";
        inboxTablePackStatus = message;
        await tick();
        await waitForPaint();
    }

    /** @param {string} message */
    function finishTablePackBusy(message = "") {
        inboxTablePackBusy = false;
        inboxTablePackStatus = message;
        if (tablePackStatusTimer) clearTimeout(tablePackStatusTimer);
        if (message) {
            tablePackStatusTimer = setTimeout(() => {
                inboxTablePackStatus = "";
                tablePackStatusTimer = null;
            }, 1400);
        }
    }

    async function prepareShortMessagePack() {
        inboxTablePackStatus = "Preparing text prediction...";
        await tick();
        await waitForPaint();
        try {
            await core.invoke("short_message_prepare_active_table_pack");
            preparedTablePackFingerprint = activePackFingerprint();
        } catch (err) {
            // The next encode request will surface any real issue.
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
            inboxTablePackError = "";
            inboxTablePackStatus = "";
        } catch (err) {
            inboxTablePackError = String(err);
            inboxTablePackStatus = "";
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
            inboxTablePackError = String(err);
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
            inboxTablePackError = String(err);
            finishTablePackBusy();
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

    /** @param {CustomEvent<{ fingerprint?: string }>} event */
    async function handleSelectPack(event) {
        const fingerprint = event.detail?.fingerprint;
        if (!fingerprint) return;
        const pack = tablePacks.find((p) => p.fingerprint_sha256 === fingerprint);
        if (!pack) return;
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
            shortMessageCache = {};
            shortMessagePending.clear();
            await prepareShortMessagePack();
            if (displayedMessages.length > 0) {
                await decodePendingShortMessages(displayedMessages);
            }
            finishTablePackBusy("Table pack loaded.");
        } catch (err) {
            inboxTablePackError = String(err);
            finishTablePackBusy();
        } finally {
            inboxTablePackBusy = false;
        }
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

    function handleDetailTogglePin() {
        if (selectedMessage) togglePin(selectedMessage);
    }

    function handleDetailMarkUnread() {
        if (selectedMessage) {
            markMessageUnread(selectedMessage);
            closeMessageDetail();
        }
    }
</script>

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
                class="h0xc-mini-btn"
                on:click={openH0xc}
                title="Open Hemp0x Community Chat"
            >
                <span class="mini-icon">◈</span> Hemp0x Community Chat
            </button>
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
        <TablePackPanel
            packs={tablePacks}
            selectionFingerprint={tablePackSelectionFingerprint}
            selectedPack={selectedTablePack()}
            busy={inboxTablePackBusy}
            error={inboxTablePackError}
            status={inboxTablePackStatus}
            showExport={false}
            showImport={true}
            showReset={true}
            showDelete={false}
            noteText="Select a pack to re-decode received messages. Both sender and receiver need the same custom pack to read custom-table messages."
            activePackLabel={activePackLabel()}
            activePackFingerprint={activePackFingerprint()}
            activePackFingerprintShort={activePackFingerprintShort()}
            activePackStatusTitle={activePackStatusTitle()}
            on:selectPack={handleSelectPack}
            on:import={importTablePack}
            on:reset={resetTablePack}
        />
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
        <div class="messages-loading">
            <CommanderLoader label="Loading messages" compact={true} />
        </div>
    {:else if displayedMessages.length === 0}
        <div class="messages-empty">
            <div class="empty-big">◈</div>
            <div class="empty-line">
                {#if messageExplorerMode}
                    No messages across all channels.
                {:else}
                    No messages for this asset channel.
                {/if}
            </div>
            <div class="empty-line sub">Messages load from Core message RPCs. Subscribe to the channel to receive updates.</div>
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
        <AssetMessageDetail
            message={selectedMessage}
            decoded={dsm}
            isPinned={isPinned(selectedMessage)}
            status={displayMessageStatus(selectedMessage)}
            on:close={closeMessageDetail}
            on:togglePin={handleDetailTogglePin}
            on:markUnread={handleDetailMarkUnread}
        />
    {/if}
</div>

<style>
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
        padding: 0.35rem 0.55rem;
        color: #8cff9f;
        font-size: 0.58rem;
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
    .h0xc-mini-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.3rem;
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 5px;
        padding: 0.35rem 0.55rem;
        color: var(--color-primary);
        font-size: 0.58rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
        white-space: nowrap;
    }
    .h0xc-mini-btn:hover {
        background: rgba(0, 255, 65, 0.12);
        border-color: rgba(0, 255, 65, 0.4);
    }
    .mini-icon {
        font-size: 0.75rem;
    }
    .messages-actions {
        display: flex;
        gap: 0.35rem;
    }
    .action-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.3rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        padding: 0.35rem 0.55rem;
        color: #aaa;
        font-size: 0.58rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
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
    }
    .action-icon {
        font-size: 0.85rem;
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
    .messages-loading {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1.5rem 0;
    }
    .messages-empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 2rem 1rem;
        gap: 0.3rem;
        text-align: center;
    }
    .empty-big {
        font-size: 1.5rem;
        color: var(--color-primary);
        opacity: 0.3;
        margin-bottom: 0.3rem;
    }
    .empty-line {
        color: #777;
        font-size: 0.68rem;
    }
    .empty-line.sub {
        font-size: 0.6rem;
        color: #555;
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
</style>
