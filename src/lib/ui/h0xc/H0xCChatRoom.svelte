<script>
    import { onDestroy, onMount, afterUpdate, createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import H0xCChatUserList from "./H0xCChatUserList.svelte";
    import H0xCChatCompose from "./H0xCChatCompose.svelte";
    import H0xCChatSettings from "./H0xCChatSettings.svelte";
    import H0xCUserContextMenu from "./H0xCUserContextMenu.svelte";
    import WalletUnlockModal from "../WalletUnlockModal.svelte";
    import { addNotification } from "../../stores/notifications.js";
    import { deriveRootNameFn, isH0xCChannelAsset } from "../../stores/h0xc.js";

    const dispatch = createEventDispatcher();

    /**
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     * }} AssetMessage
     * @typedef {{ rootName: string, assetName: string, lastSeen: number, messageCount: number }} Participant
     * @typedef {{ is_short_message?: boolean, text?: string, warnings?: string[] }} DecodeResult
     */

    export let identity = "";
    export let isGuest = false;
    export let onSwitchIdentity = null;
    export let onBackToSetup = null;
    export let onClose = null;
    /** @type {Participant[]} */
    export let participants = [];
    export let mutedUsers = [];
    export let blockedUsers = [];
    /** @type {{ messageExpiryDefault: number, autoDiscovery: boolean, pollingIntervalSeconds: number, autoBlockTags: string[], discoveryEnabled: boolean, muteNotifications: boolean, discoveryScanLimit: number }} */
    export let settings = {};
    export let lastScanBlock = 0;
    export let lastSeenMessageKey = "";
    export let lastScanTime = "";

    /** @type {AssetMessage[]} */
    let messages = [];
    let messagesLoading = false;
    let messagesError = "";
    let messagesWarn = "";

    /** @type {Record<string, DecodeResult>} */
    let decodeCache = {};
    /** @type {Set<string>} */
    let decodePending = new Set();

    let discovering = false;
    let discoveryError = "";
    let discoveryResult = "";

    let settingsOpen = false;
    /** @type {ReturnType<typeof setInterval> | null} */
    let pollTimer = null;
    let activePollingIntervalSeconds = 0;

    // Compose
    let composeBusy = false;
    let composeError = "";
    let searchFilter = "";
    let searchOpen = false;
    let pageSize = 200;
    let showCount = pageSize;
    let tagBlockedChannels = new Set();
    let tagLookupStatus = "";
    let tagLookupPending = false;
    /** @type {import("svelte").SvelteComponent | null} */
    let composeRef = null;

    // Unlock
    let showUnlockModal = false;
    let unlockPassword = "";
    let unlocking = false;
    let unlockError = "";
    /** @type {() => void | null} */
    let unlockCallback = null;

    // Scroll
    /** @type {HTMLDivElement | null} */
    let chatMessagesEl = null;
    let isNearBottom = true;
    let showJumpToBottom = false;
    const BOTTOM_THRESHOLD = 80;

    // Tag lookup cache
    let lastTagCheckTime = 0;
    const TAG_CHECK_COOLDOWN_MS = 30000;
    let tagCheckChannelsHash = "";

    // Auto-discovery
    let lastAutoDiscovery = 0;
    const AUTO_DISCOVERY_INTERVAL_MS = 300000; // 5 minutes
    let chatVisible = true;

    // Smart discovery
    /** @type {"idle"|"scanning"|"paused"|"disabled"} */
    let discoveryState = "idle";
    let discoveryAbort = false;
    let discoveryProgress = "";
    let lastDiscoveryRpcTime = 0;
    const DISCOVERY_RPC_COOLDOWN_MS = 3000;
    /** @type {ReturnType<typeof setInterval> | null} */
    let discoveryChunkTimer = null;
    const DISCOVERY_CHUNK_MS = 15000;
    let discoveryChunkRunning = false;

    // Notifications
    /** @type {Set<string>} */
    let seenMessageKeys = new Set();
    let initialLoadDone = false;

    // Resolved addresses for context
    /** @type {Record<string, string>} */
    let resolvedAddresses = {};
    let resolvingAddress = false;

    // Username context menu
    /** @type {string | null} */
    let ctxUser = null;
    let ctxX = 0;
    let ctxY = 0;

    function rootName() {
        return deriveRootNameFn(identity);
    }

    async function loadMessages() {
        if (messagesLoading || !chatVisible) return;
        messagesLoading = true;
        messagesError = "";
        messagesWarn = "";
        try {
            const info = await core.invoke("get_messaging_info");
            if (!info.enabled) {
                messages = [];
                messagesError = "Messaging is not enabled on this node.";
                return;
            }
            const msgs = await core.invoke("view_asset_messages");
            const prevMessages = messages;
            messages = (Array.isArray(msgs) ? msgs : [])
                .filter((/** @type {AssetMessage} */ m) => isH0xCChannelAsset(m.asset_name));
            updateParticipantsFromMessages(messages);
            decodeVisible(messages);

            if (!initialLoadDone) {
                for (const msg of messages) seenMessageKeys.add(messageKey(msg));
                initialLoadDone = true;
            } else if (prevMessages.length > 0) {
                notifyNewMessages(prevMessages, messages);
            }

            const warns = [];
            if (info.warnings && info.warnings.length > 0) warns.push(...info.warnings);
            if (!info.messaging_active) warns.push("Messaging is not fully active.");
            if (!info.caches_available) warns.push("Message caches are unavailable; some messages may be missing.");
            if (warns.length > 0) messagesWarn = warns.join(" ");

            maybeCheckChannelTags();
        } catch (err) {
            messagesError = String(err);
            messages = [];
        } finally {
            messagesLoading = false;
        }
    }

    async function maybeCheckChannelTags() {
        const tags = settings.autoBlockTags || [];
        if (tags.length === 0) {
            tagBlockedChannels = new Set();
            tagLookupStatus = "";
            return;
        }
        if (tagLookupPending) return;
        const channels = [...new Set(messages.map((/** @type {AssetMessage} */ m) => m.asset_name))];
        const channelsHash = channels.sort().join("|") + "#" + tags.sort().join("|");
        const now = Date.now();
        if (channelsHash === tagCheckChannelsHash && now - lastTagCheckTime < TAG_CHECK_COOLDOWN_MS) {
            return;
        }
        tagLookupPending = true;
        tagLookupStatus = "checking tags...";
        try {
            const blocked = await core.invoke("h0xc_filter_tagged_channels", {
                channelNames: channels,
                tagNames: tags,
            });
            tagBlockedChannels = new Set(Array.isArray(blocked) ? blocked : []);
            tagLookupStatus = tagBlockedChannels.size > 0
                ? `${tagBlockedChannels.size} blocked by tag`
                : "";
            lastTagCheckTime = now;
            tagCheckChannelsHash = channelsHash;
        } catch (err) {
            tagBlockedChannels = new Set();
            const text = String(err);
            if (text.includes("Core does not support")) {
                tagLookupStatus = "tag check unavailable";
            } else {
                tagLookupStatus = "";
            }
        } finally {
            tagLookupPending = false;
        }
    }

    /** @param {AssetMessage[]} msgs */
    function updateParticipantsFromMessages(msgs) {
        const seen = new Map();
        for (const p of participants) {
            seen.set(p.assetName, p);
        }
        for (const msg of msgs) {
            const rn = deriveRootNameFn(msg.asset_name);
            if (!rn) continue;
            const existing = seen.get(msg.asset_name);
            if (existing) {
                existing.lastSeen = Math.max(existing.lastSeen, parseTime(msg.time));
                existing.messageCount = Math.max(existing.messageCount, (existing.messageCount || 0) + 1);
            } else {
                seen.set(msg.asset_name, {
                    rootName: rn,
                    assetName: msg.asset_name,
                    lastSeen: parseTime(msg.time),
                    messageCount: 1,
                });
            }
        }
        const list = Array.from(seen.values());
        list.sort((a, b) => a.rootName.localeCompare(b.rootName));
        participants = list;
    }

    /** @param {string|number|undefined} time */
    function parseTime(time) {
        if (!time) return Date.now();
        let d = new Date(time);
        if (!isNaN(d.getTime())) return d.getTime();
        const n = Number(time);
        if (!Number.isNaN(n) && n > 1000000000) return n * 1000;
        return Date.now();
    }

    /** @param {AssetMessage[]} msgs */
    async function decodeVisible(msgs) {
        const toDecode = [];
        for (const msg of msgs) {
            if (decodeCache[msg.message] === undefined && !decodePending.has(msg.message)) {
                toDecode.push(msg.message);
                decodePending.add(msg.message);
            }
        }
        if (toDecode.length === 0) return;
        const promises = toDecode.map(async (hex) => {
            try {
                decodeCache[hex] = await core.invoke("short_message_decode_built_in", { hex });
            } catch {
                decodeCache[hex] = { is_short_message: false };
            } finally {
                decodePending.delete(hex);
            }
        });
        await Promise.all(promises);
        decodeCache = decodeCache;
    }

    function messageKey(/** @type {AssetMessage} */ msg) {
        return `${msg.asset_name}|${msg.time}|${msg.message}`;
    }

    function isNotificationSuppressed(/** @type {string} */ rootName, /** @type {string} */ channelName) {
        if (settings.muteNotifications) return true;
        const upper = rootName.toUpperCase();
        if (blockedUsers.map((u) => u.toUpperCase()).includes(upper)) return true;
        if (mutedUsers.map((u) => u.toUpperCase()).includes(upper)) return true;
        if (tagBlockedChannels.has(channelName)) return true;
        if (isGuest) return true;
        return false;
    }

    function notifyNewMessages(/** @type {AssetMessage[]} */ prev, /** @type {AssetMessage[]} */ current) {
        const prevKeys = new Set(prev.map(messageKey));
        for (const msg of current) {
            const key = messageKey(msg);
            if (prevKeys.has(key) || seenMessageKeys.has(key)) continue;
            seenMessageKeys.add(key);
            lastSeenMessageKey = key;
            const rn = deriveRootNameFn(msg.asset_name);
            if (isNotificationSuppressed(rn, msg.asset_name)) continue;
            const decoded = decodeCache[msg.message];
            const body = decoded?.is_short_message && decoded.text
                ? decoded.text
                : `New message from ${rn}`;
            addNotification({
                type: "message",
                severity: "info",
                title: `H0XC · [${rn.toUpperCase()}]`,
                body,
            });
        }
    }

    /** @type {AssetMessage[]} */
    $: filteredMessages = (() => {
        const bl = new Set(blockedUsers.map((u) => u.toUpperCase()));
        const mu = new Set(mutedUsers.map((u) => u.toUpperCase()));
        let msgs = messages.filter((msg) => {
            const rn = deriveRootNameFn(msg.asset_name).toUpperCase();
            return !bl.has(rn);
        }).filter((msg) => {
            const rn = deriveRootNameFn(msg.asset_name).toUpperCase();
            return !mu.has(rn);
        });
        if (tagBlockedChannels.size > 0) {
            msgs = msgs.filter((msg) => {
                return !tagBlockedChannels.has(msg.asset_name);
            });
        }
        if (searchFilter.trim()) {
            const q = searchFilter.trim().toLowerCase();
            msgs = msgs.filter((msg) => {
                const rn = deriveRootNameFn(msg.asset_name).toLowerCase();
                const dec = decodeCache[msg.message];
                const body = dec?.is_short_message && dec.text ? dec.text.toLowerCase() : "";
                return rn.includes(q) || body.includes(q) || msg.message.toLowerCase().includes(q);
            });
        }
        return msgs;
    })();

    $: pagedMessages = filteredMessages.slice(0, showCount);
    $: hasMore = filteredMessages.length > showCount;

    $: messageRows = pagedMessages.map((msg) => ({
        msg,
        rootName: deriveRootNameFn(msg.asset_name),
        decoded: decodeCache[msg.message],
        time: formatTime(msg.time),
        isMuted: false,
    }));

    /** @param {string|number|undefined} raw */
    function formatTime(raw) {
        if (!raw) return "";
        let d = new Date(raw);
        if (isNaN(d.getTime())) {
            const n = Number(raw);
            if (!Number.isNaN(n) && n > 1000000000) d = new Date(n * 1000);
        }
        if (isNaN(d.getTime())) return String(raw);
        const pad = (/** @type {number} */ n) => String(n).padStart(2, "0");
        return `[${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}]`;
    }

    function refresh() {
        showCount = pageSize;
        loadMessages();
    }

    function loadMore() {
        showCount = showCount + pageSize;
    }

    function clearSearch() {
        searchFilter = "";
        showCount = pageSize;
    }

    /**
     * @param {boolean} [silent]
     * @param {boolean} [large]
     */
    async function discover(silent = false, large = false) {
        if (!settings.discoveryEnabled) {
            discoveryState = "disabled";
            if (!silent) discoveryResult = "Discovery is disabled in settings.";
            return;
        }
        if (discovering) return;

        const now = Date.now();
        if (now - lastDiscoveryRpcTime < DISCOVERY_RPC_COOLDOWN_MS) return;

        discovering = true;
        discoveryAbort = false;
        discoveryState = "scanning";
        if (!silent) {
            discoveryError = "";
            discoveryResult = "";
        }
        discoveryProgress = large ? "Manual scan started..." : "Scanning...";

        try {
            const rpcLimit = large ? Math.min(settings.discoveryScanLimit || 2000, 2000) : 200;
            let raw;
            try {
                raw = await core.invoke("list_network_assets", { pattern: "*.H0XC", verbose: true, limit: rpcLimit });
            } catch {
                raw = await core.invoke("view_message_channels");
            }

            if (discoveryAbort) {
                discoveryState = "paused";
                discoveryProgress = "";
                return;
            }

            let discovered = [];
            if (typeof raw === "string") {
                try {
                    const parsed = JSON.parse(raw);
                    if (Array.isArray(parsed)) {
                        discovered = parsed.map((item) => typeof item === "string" ? item : item?.name || item?.asset_name || "").filter(Boolean);
                    } else if (typeof parsed === "object") {
                        discovered = Object.keys(parsed);
                    }
                } catch {
                    discovered = raw.split(/\n/).map((s) => s.trim()).filter((s) => isH0xCChannelAsset(s));
                }
            } else if (Array.isArray(raw)) {
                discovered = raw.map((item) => typeof item === "string" ? item : item?.name || item?.asset_name || "").filter(Boolean);
            }

            const h0xcDiscovered = discovered.filter((s) => isH0xCChannelAsset(s));
            const existing = new Set(participants.map((p) => p.assetName));
            let added = 0;
            for (const name of h0xcDiscovered) {
                if (discoveryAbort) break;
                if (!existing.has(name)) {
                    participants = [...participants, {
                        rootName: deriveRootNameFn(name),
                        assetName: name,
                        lastSeen: 0,
                        messageCount: 0,
                    }];
                    added++;
                }
            }
            lastDiscoveryRpcTime = Date.now();
            const ts = new Date().toLocaleTimeString();
            lastScanTime = ts;
            if (!silent) {
                discoveryResult = added > 0 ? `Discovered ${added} new participant(s)` : "No new participants found";
            }
            discoveryProgress = "";
            discoveryState = "idle";
            lastScanBlock = 0;
        } catch (err) {
            if (!silent) discoveryError = String(err);
            discoveryState = "idle";
            discoveryProgress = "";
        } finally {
            discovering = false;
        }
    }

    function cancelDiscovery() {
        discoveryAbort = true;
        discoveryState = "paused";
        discoveryProgress = "";
    }

    function startBackgroundDiscovery() {
        if (discoveryChunkTimer) clearInterval(discoveryChunkTimer);
        discoveryChunkTimer = setInterval(() => {
            if (!chatVisible) return;
            if (!settings.autoDiscovery || !settings.discoveryEnabled) return;
            if (discovering || discoveryChunkRunning) return;
            const now = Date.now();
            if (now - lastAutoDiscovery < AUTO_DISCOVERY_INTERVAL_MS) return;
            lastAutoDiscovery = now;
            discoveryChunkRunning = true;
            discover(true).finally(() => { discoveryChunkRunning = false; });
        }, DISCOVERY_CHUNK_MS);
    }

    function stopBackgroundDiscovery() {
        if (discoveryChunkTimer) {
            clearInterval(discoveryChunkTimer);
            discoveryChunkTimer = null;
        }
    }

    function toggleSettings() {
        settingsOpen = !settingsOpen;
    }

    /** @param {CustomEvent} e */
    function handleSaveSettings(e) {
        settings = e.detail.settings;
        settingsOpen = false;
    }

    function toggleMute(/** @type {string} */ rootName) {
        mutedUsers = mutedUsers.includes(rootName)
            ? mutedUsers.filter((u) => u !== rootName)
            : [...mutedUsers, rootName];
    }

    function toggleBlock(/** @type {string} */ rootName) {
        blockedUsers = blockedUsers.includes(rootName)
            ? blockedUsers.filter((u) => u !== rootName)
            : [...blockedUsers, rootName];
        mutedUsers = mutedUsers.filter((u) => u !== rootName);
    }

    async function blockAndUnsubscribe(/** @type {string} */ rootName) {
        if (!blockedUsers.includes(rootName)) {
            blockedUsers = [...blockedUsers, rootName];
        }
        mutedUsers = mutedUsers.filter((u) => u !== rootName);
        const p = participants.find((p) => p.rootName === rootName);
        if (p) {
            try {
                await core.invoke("unsubscribe_from_channel", { channelName: p.assetName });
            } catch {
                // unsubscribe best-effort; block is already applied locally
            }
        }
    }

    async function resolveUserAddress(/** @type {string} */ rootName) {
        const part = participants.find((p) => p.rootName === rootName);
        if (!part) return;
        if (resolvedAddresses[rootName]) return;
        resolvingAddress = true;
        try {
            const addrs = await core.invoke("h0xc_resolve_authority_addresses", { channelName: part.assetName });
            if (Array.isArray(addrs) && addrs.length > 0) {
                resolvedAddresses[rootName] = addrs[0];
                resolvedAddresses = resolvedAddresses;
            }
        } catch {
            // resolution best-effort
        } finally {
            resolvingAddress = false;
        }
    }

    function showUserDetails(/** @type {string} */ rootName) {
        const part = participants.find((p) => p.rootName === rootName);
        const addr = resolvedAddresses[rootName] || "";
        if (part) {
            let text = `Channel: ${part.assetName} | Messages: ${part.messageCount} | Last seen: ${part.lastSeen ? new Date(part.lastSeen).toLocaleString() : "never"}`;
            if (addr) text += ` | Authority: ${addr}`;
            discoveryResult = text;
        }
        resolveUserAddress(rootName);
    }

    function openManageTags() {
        dispatch("manageTags");
    }

    /** @param {CustomEvent} e */
    function handleMute(e) { toggleMute(e.detail.rootName); }

    /** @param {CustomEvent} e */
    function handleBlock(e) { toggleBlock(e.detail.rootName); }

    /** @param {CustomEvent} e */
    function handleViewDetails(e) { showUserDetails(e.detail.rootName); }

    /** @param {MouseEvent} e */
    function openCtx(e, clickedRoot) {
        e.preventDefault();
        e.stopPropagation();
        if (clickedRoot.toUpperCase() === rootName().toUpperCase()) return;
        ctxUser = clickedRoot;
        ctxX = e.clientX;
        ctxY = e.clientY;
    }

    function closeCtx() {
        ctxUser = null;
    }

    function requestWalletUnlock(callback) {
        unlockPassword = "";
        unlockError = "";
        showUnlockModal = true;
        unlockCallback = callback;
    }

    async function doUnlockAndRetry() {
        if (!unlockPassword.trim() || unlocking) return;
        unlocking = true;
        unlockError = "";
        try {
            await core.invoke("wallet_unlock", { password: unlockPassword, duration: 300 });
            showUnlockModal = false;
            unlockPassword = "";
            if (unlockCallback) {
                const cb = unlockCallback;
                unlockCallback = null;
                cb();
            }
        } catch (err) {
            if (/wallet.*locked|passphrase/i.test(String(err))) {
                unlockError = "Wallet unlock failed. Check passphrase.";
            } else {
                unlockError = String(err);
            }
        } finally {
            unlocking = false;
        }
    }

    /** @param {CustomEvent} e */
    async function handleSend(e) {
        const { hex } = e.detail;
        composeBusy = true;
        composeError = "";
        try {
            const channel = identity.replace(/!$/, "");
            const expiry = settings.messageExpiryDefault > 0
                ? Math.floor(Date.now() / 1000) + settings.messageExpiryDefault * 86400
                : null;

            await core.invoke("preview_send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: expiry,
            });

            const txid = await core.invoke("send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: expiry,
            });

            if (composeRef && composeRef.clearCompose) composeRef.clearCompose();

            addNotification({
                type: "message",
                severity: "success",
                title: "H0XC Message Sent",
                body: `Broadcast on ${channel}`,
                action: { label: "Copy TXID", txid },
            });

            setTimeout(() => loadMessages(), 2000);
        } catch (err) {
            const text = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(text)) {
                requestWalletUnlock(() => handleSend(e));
                return;
            }
            composeError = text;
            addNotification({
                type: "message",
                severity: "error",
                title: "H0XC Send Failed",
                body: text,
            });
        } finally {
            composeBusy = false;
        }
    }

    function startPolling() {
        if (pollTimer) clearInterval(pollTimer);
        activePollingIntervalSeconds = settings.pollingIntervalSeconds || 30;
        const interval = activePollingIntervalSeconds * 1000;
        pollTimer = setInterval(() => {
            if (chatVisible) loadMessages();
        }, interval);
    }

    function stopPolling() {
        if (pollTimer) {
            clearInterval(pollTimer);
            pollTimer = null;
        }
        activePollingIntervalSeconds = 0;
    }

    function onVisibilityChange() {
        chatVisible = document.visibilityState === "visible";
        if (chatVisible) {
            loadMessages();
        }
    }

    onMount(() => {
        document.addEventListener("visibilitychange", onVisibilityChange);
        loadMessages();
        startPolling();
        startBackgroundDiscovery();
    });

    onDestroy(() => {
        document.removeEventListener("visibilitychange", onVisibilityChange);
        stopPolling();
        stopBackgroundDiscovery();
        discoveryAbort = true;
    });

    $: if (
        pollTimer
        && typeof settings?.pollingIntervalSeconds === "number"
        && settings.pollingIntervalSeconds !== activePollingIntervalSeconds
    ) {
        startPolling();
    }

    $: if (settings?.autoDiscovery !== undefined || settings?.discoveryEnabled !== undefined) {
        stopBackgroundDiscovery();
        if (settings.autoDiscovery && settings.discoveryEnabled) {
            startBackgroundDiscovery();
        }
        if (!settings.discoveryEnabled) {
            discoveryState = "disabled";
        } else if (discoveryState === "disabled") {
            discoveryState = "idle";
        }
    }

    let lastIdentity = "";
    $: if (identity && identity !== lastIdentity) {
        lastIdentity = identity;
        loadMessages();
    }

    function onScroll() {
        if (!chatMessagesEl) return;
        const { scrollHeight, scrollTop, clientHeight } = chatMessagesEl;
        const dist = scrollHeight - scrollTop - clientHeight;
        isNearBottom = dist < BOTTOM_THRESHOLD;
        showJumpToBottom = !isNearBottom && scrollTop > 0;
    }

    function scrollToBottom() {
        if (chatMessagesEl) {
            chatMessagesEl.scrollTop = chatMessagesEl.scrollHeight;
            showJumpToBottom = false;
            isNearBottom = true;
        }
    }

    afterUpdate(() => {
        if (isNearBottom) scrollToBottom();
    });

    function handleWindowClick() {
        if (ctxUser) closeCtx();
    }
</script>

<svelte:window on:click={handleWindowClick} />

<div class="h0xc-chat-room" transition:fade={{ duration: 120 }}>
    <div class="chat-header">
        <div class="header-left">
            <span class="header-icon">◈</span>
            <span class="header-title">H0XC</span>
            {#if isGuest}
                <span class="header-badge guest">GUEST</span>
            {:else}
                <span class="header-badge identity">[{rootName().toUpperCase()}]</span>
            {/if}
        </div>
        <div class="header-right">
            {#if onSwitchIdentity || (isGuest && onBackToSetup)}
                <button class="header-btn switch" on:click={() => isGuest ? onBackToSetup?.() : onSwitchIdentity?.()} title={isGuest ? "Choose identity" : "Switch identity"}>
                    ⇄
                </button>
            {/if}
            <button class="header-btn" on:click={() => { refresh(); discover(false, true); }} disabled={messagesLoading || discovering} title="Refresh messages & discover participants">
                {discovering ? "..." : "↻"}
            </button>
            <button class="header-btn" class:active={searchOpen} on:click={() => { searchOpen = !searchOpen; if (!searchOpen) clearSearch(); }} title="Filter messages">
                ⌕
            </button>
            <button class="header-btn" on:click={toggleSettings} title="Settings">
                ⚙
            </button>
            {#if onClose}
                <button class="header-btn" on:click={onClose} title="Close chat">
                    ✕
                </button>
            {/if}
        </div>
    </div>
    <div class="chat-status-bar">
        {#if messagesLoading}
            <span class="status-pill">Loading...</span>
        {/if}
        {#if tagLookupPending}
            <span class="status-pill">Tags...</span>
        {/if}
        {#if discovering}
            <span class="status-pill">Scanning...{discoveryProgress ? ` ${discoveryProgress}` : ""}</span>
            <button class="status-cancel" on:click={cancelDiscovery} title="Cancel scan">✕</button>
        {/if}
        {#if !discovering && discoveryState === "idle" && lastScanTime}
            <span class="status-pill dim">Last scan: {lastScanTime}</span>
        {/if}
        {#if !settings.discoveryEnabled}
            <span class="status-pill warn">Discovery off</span>
        {/if}
        {#if discoveryResult}
            <span class="status-pill ok">{discoveryResult}</span>
        {/if}
        {#if discoveryError}
            <span class="status-pill err" title={discoveryError}>Discovery error</span>
        {/if}
        {#if tagLookupStatus}
            <span class="status-pill warn">{tagLookupStatus}</span>
        {/if}
        {#if messagesError}
            <span class="status-pill err">{messagesError}</span>
        {/if}
    </div>

    {#if searchOpen}
        <div class="chat-search-bar" transition:fade={{ duration: 100 }}>
            <input
                class="search-input"
                type="text"
                bind:value={searchFilter}
                on:input={() => { showCount = pageSize; }}
                placeholder="Filter messages..."
                aria-label="Filter messages"
            />
            {#if searchFilter}
                <button class="search-clear" on:click={clearSearch} title="Clear filter">✕</button>
            {/if}
        </div>
    {/if}

    {#if messagesWarn}
        <div class="chat-status warn">{messagesWarn}</div>
    {/if}
    {#if messagesError}
        <div class="chat-status error">{messagesError}</div>
    {/if}

    <div class="chat-body-wrap">
        <div class="chat-messages" class:loading={messagesLoading} bind:this={chatMessagesEl} on:scroll={onScroll}>
            {#if messagesLoading && messages.length === 0}
                <div class="skeleton-wrap">
                    <div class="skeleton-line"></div>
                    <div class="skeleton-line short"></div>
                    <div class="skeleton-line"></div>
                    <div class="skeleton-line short"></div>
                    <div class="skeleton-line"></div>
                </div>
            {:else if filteredMessages.length === 0}
                <div class="chat-empty">
                    <div class="empty-big">◈</div>
                    <div class="empty-line">No messages in H0XC yet.</div>
                    <div class="empty-line sub">Messages load from subscribed channels via Core. Use Discover to find new H0XC participants.</div>
                    {#if settings.discoveryEnabled}
                        <button class="empty-discover" on:click={() => discover(false, true)}>Discover</button>
                    {:else}
                        <div class="empty-line sub">Discovery is disabled. Enable it in Settings to find participants.</div>
                    {/if}
                </div>
            {:else}
                {#each messageRows as row (row.msg.asset_name + row.msg.time + row.msg.message)}
                    <div class="chat-msg" class:me={row.rootName.toUpperCase() === rootName().toUpperCase()}>
                        <span class="msg-time">{row.time}</span>
                        <span
                            class="msg-user"
                            class:me={row.rootName.toUpperCase() === rootName().toUpperCase()}
                            on:click|preventDefault={(e) => openCtx(e, row.rootName)}
                            role="button"
                            tabindex="0"
                            on:keydown={(e) => e.key === "Enter" && openCtx(e, row.rootName)}
                        >[{row.rootName.toUpperCase()}]</span>
                        <span class="msg-body">
                            {#if row.decoded?.is_short_message && row.decoded.text}
                                <span class="msg-short-text">{row.decoded.text}</span>
                                {#if row.decoded.warnings && row.decoded.warnings.length > 0}
                                    <span class="msg-decode-warn" title={row.decoded.warnings.join(", ")}>⚠</span>
                                {/if}
                            {:else if row.decoded && !row.decoded.is_short_message}
                                <span class="msg-needs-pack" title="This message requires a matching table pack to decode. Raw hex: {row.msg.message.slice(0, 16)}...">[needs matching table pack]</span>
                            {:else}
                                <span class="msg-hex">{row.msg.message.slice(0, 10)}...{row.msg.message.slice(-6)}</span>
                            {/if}
                        </span>
                    </div>
                {/each}

                {#if hasMore}
                    <button class="show-more-btn" on:click={loadMore}>
                        Show {pageSize} more ({filteredMessages.length - showCount} remaining)
                    </button>
                {/if}
            {/if}

            {#if messagesLoading && messages.length > 0}
                <div class="chat-loading-more">Refreshing...</div>
            {/if}

            {#if showJumpToBottom}
                <button class="jump-btn" on:click={scrollToBottom} transition:fade={{ duration: 100 }}>
                    Jump to bottom ↓
                </button>
            {/if}
        </div>

        <H0xCChatUserList
            {participants}
            {mutedUsers}
            {blockedUsers}
            selectedIdentity={identity}
            tagBlockedChannels={tagBlockedChannels}
            {resolvedAddresses}
            on:mute={handleMute}
            on:block={handleBlock}
            on:viewDetails={handleViewDetails}
            on:blockAndUnsub={(e) => { blockAndUnsubscribe(e.detail.rootName); }}
            on:manageTags={() => openManageTags()}
        />
    </div>

    <H0xCChatCompose
        bind:this={composeRef}
        {isGuest}
        busy={composeBusy}
        error={composeError}
        on:send={handleSend}
        on:requestIdentity={() => onBackToSetup?.()}
    />

    <H0xCChatSettings
        show={settingsOpen}
        {settings}
        {blockedUsers}
        {discoveryState}
        {lastScanTime}
        on:close={() => (settingsOpen = false)}
        on:save={handleSaveSettings}
        on:unblock={(e) => { blockedUsers = blockedUsers.filter((u) => u !== e.detail.rootName); }}
    />

    <H0xCUserContextMenu
        x={ctxX}
        y={ctxY}
        user={ctxUser || ""}
        muted={ctxUser ? mutedUsers.includes(ctxUser) : false}
        blocked={ctxUser ? blockedUsers.includes(ctxUser) : false}
        resolvedAddress={ctxUser ? resolvedAddresses[ctxUser] || "" : ""}
        on:viewDetails={(e) => { showUserDetails(e.detail.rootName); closeCtx(); }}
        on:mute={(e) => { toggleMute(e.detail.rootName); closeCtx(); }}
        on:block={(e) => { toggleBlock(e.detail.rootName); closeCtx(); }}
        on:blockAndUnsub={(e) => { blockAndUnsubscribe(e.detail.rootName); closeCtx(); }}
        on:manageTags={(e) => { openManageTags(); closeCtx(); }}
    />

    <WalletUnlockModal
        show={showUnlockModal}
        bind:password={unlockPassword}
        {unlocking}
        error={unlockError}
        title="UNLOCK WALLET"
        body="Wallet unlock required to send community chat messages."
        confirmLabel="UNLOCK AND SEND"
        on:cancel={() => { showUnlockModal = false; unlockPassword = ""; unlockError = ""; unlockCallback = null; }}
        on:confirm={doUnlockAndRetry}
    />
</div>

<style>
    .h0xc-chat-room {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
        position: relative;
    }
    .chat-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.35rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
        gap: 0.4rem;
        flex-wrap: wrap;
    }
    .header-left {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .header-icon {
        font-size: 0.8rem;
        color: var(--color-primary);
    }
    .header-title {
        font-size: 0.6rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }
    .header-badge {
        font-size: 0.5rem;
        letter-spacing: 0.5px;
        padding: 0.12rem 0.35rem;
        border-radius: 999px;
        font-weight: 600;
    }
    .header-badge.identity {
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
    }
    .header-badge.guest {
        color: #888;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.12);
    }
    .header-right {
        display: flex;
        align-items: center;
        gap: 0.25rem;
    }
    .header-btn {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 5px;
        padding: 0.2rem 0.35rem;
        color: #888;
        font-size: 0.7rem;
        cursor: pointer;
        transition: all 0.15s;
        line-height: 1;
    }
    .header-btn:hover:not(:disabled) {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .header-btn:disabled { opacity: 0.4; cursor: not-allowed; }
    .header-btn.active { border-color: var(--color-primary); color: var(--color-primary); background: rgba(0, 255, 65, 0.08); }
    .header-btn.switch { font-size: 0.85rem; }
    .chat-status {
        font-size: 0.55rem;
        padding: 0.3rem 0.6rem;
        color: #ffaa00;
        background: rgba(255, 170, 0, 0.06);
        border-bottom: 1px solid rgba(255, 170, 0, 0.15);
    }
    .chat-status.error {
        color: #ff5555;
        background: rgba(255, 85, 85, 0.06);
        border-color: rgba(255, 85, 85, 0.15);
    }
    .chat-status.warn {
        color: #ffcc00;
        background: rgba(255, 204, 0, 0.06);
        border-color: rgba(255, 204, 0, 0.15);
    }
    .chat-status-bar {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: 0.25rem;
        padding: 0.25rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        background: rgba(0, 0, 0, 0.15);
        min-height: 1.4rem;
    }
    .status-pill {
        font-size: 0.48rem;
        color: #888;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 999px;
        padding: 0.1rem 0.4rem;
        white-space: nowrap;
    }
    .status-pill.ok {
        color: #8cff9f;
        border-color: rgba(0, 255, 65, 0.15);
        background: rgba(0, 255, 65, 0.05);
    }
    .status-pill.warn {
        color: #ffaa88;
        border-color: rgba(255, 170, 0, 0.15);
        background: rgba(255, 170, 0, 0.05);
    }
    .status-pill.err {
        color: #ff8888;
        border-color: rgba(255, 85, 85, 0.15);
        background: rgba(255, 85, 85, 0.05);
    }
    .status-pill.dim {
        color: #555;
        border-color: rgba(255, 255, 255, 0.05);
        background: transparent;
    }
    .status-cancel {
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.2);
        border-radius: 999px;
        color: #ff8888;
        font-size: 0.48rem;
        cursor: pointer;
        padding: 0.08rem 0.3rem;
        line-height: 1;
        transition: all 0.15s;
    }
    .status-cancel:hover {
        background: rgba(255, 85, 85, 0.2);
        color: #ff5555;
    }
    .chat-body-wrap {
        display: flex;
        flex: 1;
        min-height: 0;
        position: relative;
    }
    .chat-messages {
        flex: 1;
        min-width: 0;
        overflow-y: auto;
        padding: 0.3rem;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.25) transparent;
        font-family: var(--font-mono);
        position: relative;
    }
    .chat-messages::-webkit-scrollbar { width: 5px; }
    .chat-messages::-webkit-scrollbar-track { background: transparent; }
    .chat-messages::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.25); border-radius: 3px; }
    .chat-messages.loading { opacity: 0.6; }
    .chat-empty {
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
    .empty-discover {
        margin-top: 0.5rem;
        padding: 0.4rem 0.8rem;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.62rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .empty-discover:hover {
        background: var(--color-primary);
        color: #000;
    }
    .chat-loading-more {
        text-align: center;
        font-size: 0.5rem;
        color: #555;
        padding: 0.5rem;
    }
    .chat-msg {
        display: flex;
        gap: 0.45rem;
        padding: 0.15rem 0.3rem;
        font-size: 0.62rem;
        line-height: 1.4;
        transition: background 0.1s;
        border-left: 2px solid transparent;
        contain: content;
    }
    .chat-msg:hover {
        background: rgba(0, 255, 65, 0.03);
    }
    .chat-msg.me {
        background: rgba(0, 255, 65, 0.03);
        border-left-color: rgba(0, 255, 65, 0.25);
    }
    .msg-time {
        color: var(--color-primary-dim);
        flex-shrink: 0;
        font-size: 0.52rem;
        min-width: 4.5rem;
    }
    .msg-user {
        color: var(--color-primary);
        font-weight: 700;
        flex-shrink: 0;
        letter-spacing: 0.3px;
        font-size: 0.58rem;
        cursor: pointer;
        user-select: none;
    }
    .msg-user:hover {
        text-decoration: underline;
    }
    .msg-user.me {
        text-shadow: 0 0 8px rgba(0, 255, 65, 0.3);
    }
    .msg-body {
        color: #ccc;
        word-break: break-word;
        min-width: 0;
    }
    .msg-short-text {
        color: #ddd;
    }
    .msg-hex {
        color: #666;
        font-size: 0.52rem;
    }
    .msg-needs-pack {
        color: #ffaa00;
        font-style: italic;
        font-size: 0.52rem;
    }
    .msg-decode-warn {
        color: #ffaa00;
        font-size: 0.6rem;
        cursor: help;
        margin-left: 0.2rem;
    }
    .jump-btn {
        position: absolute;
        bottom: 8px;
        left: 50%;
        transform: translateX(-50%);
        padding: 0.25rem 0.6rem;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        border-radius: 999px;
        color: var(--color-primary);
        font-size: 0.55rem;
        font-weight: 600;
        cursor: pointer;
        letter-spacing: 0.5px;
        font-family: var(--font-mono);
        transition: all 0.15s;
        z-index: 2;
    }
    .jump-btn:hover {
        background: var(--color-primary);
        color: #000;
    }
    .skeleton-wrap {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        padding: 0.8rem 0.4rem;
    }
    .skeleton-line {
        height: 0.7rem;
        background: linear-gradient(90deg, rgba(255,255,255,0.03) 25%, rgba(255,255,255,0.07) 50%, rgba(255,255,255,0.03) 75%);
        background-size: 200% 100%;
        border-radius: 3px;
        animation: skeleton-pulse 1.2s infinite ease-in-out;
    }
    .skeleton-line.short {
        width: 60%;
    }
    @keyframes skeleton-pulse {
        0% { background-position: 200% 0; }
        100% { background-position: -200% 0; }
    }
    .chat-search-bar {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        padding: 0.25rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        background: rgba(0, 0, 0, 0.15);
    }
    .search-input {
        flex: 1;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        padding: 0.2rem 0.4rem;
        color: #ccc;
        font-size: 0.55rem;
        font-family: var(--font-mono);
        outline: none;
    }
    .search-input:focus {
        border-color: var(--color-primary);
    }
    .search-input::placeholder { color: #444; }
    .search-clear {
        background: none;
        border: none;
        color: #666;
        font-size: 0.7rem;
        cursor: pointer;
        padding: 0.1rem 0.25rem;
    }
    .search-clear:hover { color: #fff; }
    .show-more-btn {
        display: block;
        width: 100%;
        background: rgba(0, 255, 65, 0.04);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.52rem;
        font-weight: 600;
        padding: 0.25rem;
        cursor: pointer;
        margin: 0.2rem 0;
        text-align: center;
        transition: all 0.15s;
    }
    .show-more-btn:hover {
        background: rgba(0, 255, 65, 0.1);
    }
</style>
