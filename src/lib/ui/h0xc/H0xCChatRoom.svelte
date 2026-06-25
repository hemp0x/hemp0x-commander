<script>
    import { onDestroy, onMount, afterUpdate, createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import H0xCChatUserList from "./H0xCChatUserList.svelte";
    import H0xCChatCompose from "./H0xCChatCompose.svelte";
    import H0xCChatSettings from "./H0xCChatSettings.svelte";
    import H0xCUserContextMenu from "./H0xCUserContextMenu.svelte";
    import WalletUnlockModal from "../WalletUnlockModal.svelte";
    import {
        loadWalletPinStatus,
        defaultUnlockMode,
        pinRequiresPassphrase,
        unlockWalletWithPin,
        unlockRuntimeWalletWithPassphrase,
        forgotWalletPin,
        isValidPin,
    } from "../../walletPinUnlock.js";
    import HelpHitbox from "../HelpHitbox.svelte";
    import { ensureNodeSyncedForBroadcast } from "../../utils/nodeSync.js";
    import { addNotification } from "../../stores/notifications.js";
    import { deriveRootNameFn, isH0xCAsset } from "../../stores/h0xc.js";
    import {
        interpretControlMessages,
        isControlCommandMessage,
        isDecodedLeaveCommandMessage,
        buildDeleteCommandHex,
        buildLeaveCommandHex,
        buildStatusCommandHex,
        buildReportMessageCommandHex,
        buildReportChannelCommandHex,
        canonicalMessageChannel,
        STATUS_LABELS,
        STATUS_ICONS,
        REASON_LABELS,
        SEVERITY_LABELS,
        hideMessageLocally,
        hideChannelLocally,
        unhideMessageLocally,
        unhideChannelLocally,
        isMessageModerationHidden,
        isChannelModerationHidden,
        aggregateReports,
        shouldAutoHide,
        setCommunityHidden,
        getCommunityHidden,
        addOverrideHidden,
    } from "../../stores/h0xc-control.js";

    const dispatch = createEventDispatcher();

    /**
     * @typedef {{
     *   asset_name: string;
     *   message: string;
     *   time?: string|number;
     *   block_height?: string|number;
     *   status?: string;
     *   expire_time?: string|number|null;
     *   expire_utc_time?: string|number|null;
     *   txid?: string;
     *   channel?: string;
     *   authority_asset?: string;
     *   authority_address?: string;
     *   block_hash?: string;
     *   sender_address?: string;
     * }} AssetMessage
     * @typedef {{ rootName: string, assetName: string, lastSeen: number, messageCount: number, joinedAt?: number }} Participant
     * @typedef {{ is_short_message?: boolean, text?: string, warnings?: string[], is_h0xc_chat_message?: boolean }} DecodeResult
     */

    /**
     * H0XC identity is channel/authority based, not a permanent person account.
     * The selected identity is the channel asset (e.g., ROOT/H0XC) that this
     * user broadcasts under. If the authority asset is transferred to another
     * wallet, the new holder can speak under the same channel name. If the
     * authority is burned or no spendable authority remains, no new messages
     * can be sent from that channel. Old messages remain historical records.
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
    /** @type {{ messageExpiryDefault: number, autoDiscovery: boolean, pollingIntervalSeconds: number, autoBlockTags: string[], discoveryEnabled: boolean, muteNotifications: boolean, discoveryScanLimit: number, historyDays: number, showExpired: boolean, hideStaleUsers: boolean, staleUserDays: number }} */
    export let settings = {};
    export let lastScanBlock = 0;
    export let lastSeenMessageKey = "";
    export let lastScanTime = "";

    /** @type {AssetMessage[]} */
    let messages = [];
    let messagesLoading = false;
    let messagesError = "";
    let messagesWarn = "";
    let showRefreshIndicator = false;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let refreshIndicatorTimer = null;

    /** @type {Record<string, DecodeResult>} */
    let decodeCache = {};
    /** @type {Set<string>} */
    let decodePending = new Set();

    /** @type {Set<string>} */
    let hiddenTxids = new Set();
    /** @type {Set<string>} */
    export let leftChannels = new Set();

    // Status
    /** @type {Map<string, {value: number, expiryMode: number, expiryValue: number, expiryTs: number, commandTime: number, commandKey: string}>} */
    let statusByChannel = new Map();
    let statusOpen = false;
    let statusBusy = false;
    let statusError = "";

    // Reports
    /** @type {Array<{target: string, targetType: number, reason: number, severity: number, durationDays: number, channel: string, timeSec: number, commandKey: string}>} */
    let reportCommands = [];
    let reportOpen = false;
    let reportBusy = false;
    let reportError = "";
    let reportTargetType = 0;
    let reportTargetId = "";
    let reportTargetLabel = "";
    let reportReason = 1;
    let reportSeverity = 2;
    let reportDurationDays = 30;

    // Status picker state
    let selectedStatus = 0;
    let selectedExpiryMode = 0;
    let selectedExpiryHours = 4;
    let selectedExpiryDateTime = "";

    // Moderation lists for settings view
    import { getLocalHiddenMessages, getLocalHiddenChannels } from "../../stores/h0xc-control.js";
    $: localHiddenMessagesList = getLocalHiddenMessages();
    $: localHiddenChannelsList = getLocalHiddenChannels();
    $: communityHiddenList = getCommunityHidden();

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
    // Runtime Wallet PIN unlock (slice 76b)
    let walletPinStatus = null;
    let walletUnlockMode = "passphrase";
    let unlockPin = "";
    $: walletPinUsable = !!walletPinStatus?.pin_configured && !pinRequiresPassphrase(walletPinStatus);
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

    // Message detail panel
    /** @type {null | { msg: AssetMessage, rootName: string, decoded: DecodeResult|undefined, time: string }} */
    let msgDetail = null;

    // Broadcast preview
    /** @type {null | { hex: string, text: string, channel: string, expiry: number|null, warnings: string[], feeEstimate: string|null }} */
    let broadcastPreview = null;
    let broadcastBusy = false;
    let broadcastError = "";

    // Optimistic pending messages
    /** @type {Array<{id: string, hex: string, text: string, assetName: string, rootName: string, timeMs: number, status: "pending"|"failed", error?: string, txid?: string}>} */
    let pendingMessages = [];
    let pendingIdCounter = 0;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let pendingTimeout = null;
    const PENDING_TIMEOUT_MS = 60000;

    // Username context menu
    /** @type {string | null} */
    let ctxUser = null;
    let ctxX = 0;
    let ctxY = 0;

    $: ctxParticipant = ctxUser ? participants.find((p) => p.rootName === ctxUser) : null;
    $: ctxIsSelf = ctxUser ? ctxUser.toUpperCase() === rootName().toUpperCase() : false;

    $: myStatusEntry = (() => {
        const myChannel = identity ? canonicalMessageChannel({ asset_name: identity }) : "";
        return myChannel ? statusByChannel.get(myChannel) : null;
    })();
    $: myStatusValue = myStatusEntry ? myStatusEntry.value : undefined;

    $: {
        if (messagesLoading && messages.length > 0 && !showRefreshIndicator) {
            if (refreshIndicatorTimer) clearTimeout(refreshIndicatorTimer);
            refreshIndicatorTimer = setTimeout(() => {
                showRefreshIndicator = true;
            }, 600);
        } else if (!messagesLoading) {
            showRefreshIndicator = false;
            if (refreshIndicatorTimer) {
                clearTimeout(refreshIndicatorTimer);
                refreshIndicatorTimer = null;
            }
        }
    }

    function rootName() {
        return deriveRootNameFn(identity);
    }

    function uniqueH0xCChannels() {
        const seen = new Set();
        const result = [];
        const add = (ch) => {
            const norm = ch.replace(/!$/, "").trim();
            if (!norm || seen.has(norm.toUpperCase())) return;
            seen.add(norm.toUpperCase());
            result.push(norm);
        };
        if (identity) add(identity);
        for (const p of participants) {
            if (p.assetName) add(p.assetName);
        }
        return result;
    }

    async function loadChannelMessages(channel) {
        try {
            return await core.invoke("view_channel_messages", { channel });
        } catch {
            return null;
        }
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
            const channels = uniqueH0xCChannels();
            let allMsgs = [];
            let anyChannelLoaded = false;
            // Guests have no identity channel of their own and rely on the global
            // H0xC feed. Per-channel fetches are keyed off the discovery cache, which
            // for a fresh/incomplete guest omits not-yet-discovered channels (e.g. a
            // GRIDSHADE/H0XC channel). Always use the global view for guests so public
            // H0xC messages are visible even before discovery populates participants.
            if (!isGuest && channels.length > 0) {
                const MAX_CONCURRENT = 4;
                for (let i = 0; i < channels.length; i += MAX_CONCURRENT) {
                    const batch = channels.slice(i, i + MAX_CONCURRENT);
                    const results = await Promise.allSettled(
                        batch.map((ch) => loadChannelMessages(ch))
                    );
                    for (const r of results) {
                        if (r.status === "fulfilled" && Array.isArray(r.value)) {
                            allMsgs.push(...r.value);
                            anyChannelLoaded = true;
                        }
                    }
                }
            }
            if (isGuest || !anyChannelLoaded) {
                try {
                    allMsgs = await core.invoke("view_asset_messages");
                } catch {
                    allMsgs = [];
                }
            }
            const prevMessages = messages;
            const rawMessageCount = Array.isArray(allMsgs) ? allMsgs.length : 0;
            let feedDiagnostic = "";
            messages = (Array.isArray(allMsgs) ? allMsgs : [])
                .filter((/** @type {AssetMessage} */ m) => isH0xCAsset(m.asset_name));
            if (rawMessageCount === 0) {
                feedDiagnostic = "Core returned no asset messages. If this node is new or recovering, wait for message indexes to catch up and refresh.";
            } else if (messages.length === 0) {
                feedDiagnostic = `Core returned ${rawMessageCount} asset message record(s), but none were H0xC channels.`;
            }
            const seenKeys = new Set();
            messages = messages.filter((/** @type {AssetMessage} */ m) => {
                const key = m.txid
                    ? `txid:${m.txid}`
                    : `${m.asset_name}|${m.message}|${m.time}|${m.block_height}`;
                if (seenKeys.has(key)) return false;
                seenKeys.add(key);
                return true;
            });
            const controlResult = await interpretControlMessages(messages);
            hiddenTxids = controlResult.hiddenTxids;
            const leaveMessageKeys = controlResult.leaveMessageKeys || new Set();
            // Rejoin: the most recent message per channel wins.
            // If a channel's latest loaded message is a leave command, it is left.
            // If the latest loaded message is a normal message, it is not left.
            const lastMessageType = new Map(); // canonical channel -> { time: number, isLeave: boolean }
            for (const msg of messages) {
                const ch = canonicalMessageChannel(msg);
                if (!ch) continue;
                const t = parseTime(msg.time);
                const existing = lastMessageType.get(ch);
                if (!existing || t > existing.time) {
                    const isLeave = isDecodedLeaveCommandMessage(msg, leaveMessageKeys);
                    if (isControlCommandMessage(msg) && !isLeave) {
                        // Delete and other non-leave control commands do not affect leave state.
                        // Preserve whatever entry is already stored for this channel.
                        continue;
                    }
                    lastMessageType.set(ch, { time: t, isLeave });
                }
            }
            const nextLeft = new Set();
            for (const [ch, info] of lastMessageType) {
                if (info.isLeave) nextLeft.add(ch);
            }
            // Preserve channels that were left but have no messages in the current load
            for (const ch of leftChannels) {
                if (!lastMessageType.has(ch)) nextLeft.add(ch);
            }
            leftChannels = nextLeft;

            statusByChannel = controlResult.statusByChannel || new Map();
            reportCommands = controlResult.reportCommands || [];
            updateCommunityHiddenFromReports();

            await decodeVisible(messages);
            updateParticipantsFromMessages(messages);
            reconcilePending();

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
            if (feedDiagnostic) warns.push(feedDiagnostic);
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
        const byAsset = new Map();
        for (const p of participants) {
            byAsset.set(p.assetName, {
                rootName: p.rootName,
                assetName: p.assetName,
                lastSeen: p.lastSeen,
                messageCount: 0,
                joinedAt: p.joinedAt,
            });
        }
        for (const msg of msgs) {
            if (isControlCommandMessage(msg)) continue;
            if (!isDisplayableH0xCMessage(msg)) continue;
            const rn = deriveRootNameFn(msg.asset_name);
            if (!rn) continue;
            const t = parseTime(msg.time);
            const entry = byAsset.get(msg.asset_name);
            if (entry) {
                entry.messageCount += 1;
                if (t > entry.lastSeen) entry.lastSeen = t;
                if (!entry.joinedAt || t < entry.joinedAt) entry.joinedAt = t;
            } else {
                byAsset.set(msg.asset_name, {
                    rootName: rn,
                    assetName: msg.asset_name,
                    lastSeen: t,
                    messageCount: 1,
                    joinedAt: t,
                });
            }
        }
        const list = Array.from(byAsset.values());
        list.sort((a, b) => a.rootName.localeCompare(b.rootName));
        participants = list;
    }

    function updateCommunityHiddenFromReports() {
        if (!settings.communityReportAutoHide) {
            setCommunityHidden([]);
            communityHiddenList = [];
            return;
        }

        const blockedReporters = new Set();
        const blockedRoots = new Set(blockedUsers.map((u) => u.toUpperCase()));
        const mutedRoots = new Set(mutedUsers.map((u) => u.toUpperCase()));
        for (const p of participants) {
            const root = (p.rootName || "").toUpperCase();
            const channel = canonicalMessageChannel({ asset_name: p.assetName });
            if (!channel) continue;
            if (blockedRoots.has(root) || mutedRoots.has(root) || leftChannels.has(channel) || tagBlockedChannels.has(p.assetName) || tagBlockedChannels.has(channel)) {
                blockedReporters.add(channel);
            }
        }

        const windowDays = Math.max(1, Math.min(365, Number(settings.communityReportWindowDays) || 30));
        const minReports = Math.max(1, Math.min(20, Number(settings.communityReportMinReports) || 3));
        const minRatio = Math.max(0.1, Math.min(1, Number(settings.communityReportMinRatio) || 0.4));
        const recentParticipants = Math.max(1, participants.filter((p) => {
            const channel = canonicalMessageChannel({ asset_name: p.assetName });
            return channel && !blockedReporters.has(channel);
        }).length);
        const aggregated = aggregateReports(reportCommands, blockedReporters, windowDays);
        const now = Math.floor(Date.now() / 1000);
        const next = [];

        for (const [key, entry] of aggregated) {
            if (!shouldAutoHide(entry, minReports, minRatio, recentParticipants)) continue;
            const sep = key.indexOf(":");
            const targetType = Number(key.slice(0, sep));
            const target = key.slice(sep + 1);
            const durationDays = entry.maxDuration || 30;
            next.push({
                target,
                targetType,
                count: entry.channels.size,
                channels: Array.from(entry.channels),
                maxSeverity: entry.maxSeverity,
                expiryTs: Math.max(entry.latestTime, now) + durationDays * 86400,
            });
        }

        setCommunityHidden(next);
        communityHiddenList = next;
    }

    /**
     * Parse a message timestamp to milliseconds since epoch.
     * Handles: numeric Unix seconds, numeric Unix milliseconds, Core-style
     * "YYYY-MM-DD HH:MM:SS" (interpreted as UTC), ISO 8601 strings, and
     * browser-native Date-parseable strings.
     * @param {string|number|undefined|null} raw
     * @returns {number}
     */
    function parseTime(raw) {
        if (raw == null || raw === "") return Date.now();
        if (typeof raw === "number" || (typeof raw === "string" && /^\d{10,}$/.test(raw.trim()))) {
            const n = typeof raw === "number" ? raw : parseInt(raw, 10);
            if (Number.isNaN(n)) return Date.now();
            if (n > 1e12) return n;
            if (n > 1e9) return n * 1000;
            return Date.now();
        }
        if (typeof raw === "string") {
            const coreMatch = raw.match(/^(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})$/);
            if (coreMatch) {
                const [, y, mo, d, h, mi, s] = coreMatch;
                const ms = Date.UTC(+y, +mo - 1, +d, +h, +mi, +s);
                return Number.isNaN(ms) ? Date.now() : ms;
            }
            const d = new Date(raw);
            if (!isNaN(d.getTime())) return d.getTime();
        }
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
            if (decoded?.is_short_message && !decoded?.is_h0xc_chat_message) continue;
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

    function isDisplayableH0xCMessage(/** @type {AssetMessage} */ msg) {
        const dec = decodeCache[msg.message];
        if (dec?.is_h0xc_chat_message === true) return true;
        return isH0xCAsset(msg.asset_name) && dec?.is_short_message === true && !!dec.text;
    }

    let searchUserFilter = "";
    let searchChannelFilter = "";
    let historyOverride = false;

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
        }).filter((msg) => {
            return !isMessageModerationHidden(msg, hiddenTxids).hidden;
        }).filter((msg) => {
            return !isChannelModerationHidden(
                msg.asset_name,
                leftChannels,
                bl,
                tagBlockedChannels
            ).hidden;
        }).filter((msg) => {
            return !isControlCommandMessage(msg);
        }).filter((msg) => {
            return isDisplayableH0xCMessage(msg);
        });
        if (tagBlockedChannels.size > 0) {
            msgs = msgs.filter((msg) => {
                return !tagBlockedChannels.has(msg.asset_name);
            });
        }
        if (searchUserFilter.trim()) {
            const q = searchUserFilter.trim().toLowerCase();
            msgs = msgs.filter((msg) => {
                const rn = deriveRootNameFn(msg.asset_name).toLowerCase();
                return rn.includes(q);
            });
        }
        if (searchChannelFilter.trim()) {
            const q = searchChannelFilter.trim().toLowerCase();
            msgs = msgs.filter((msg) => {
                return msg.asset_name.toLowerCase().includes(q);
            });
        }
        if (searchFilter.trim()) {
            const q = searchFilter.trim().toLowerCase();
            msgs = msgs.filter((msg) => {
                const rn = deriveRootNameFn(msg.asset_name).toLowerCase();
                const dec = decodeCache[msg.message];
                const body = dec?.is_short_message && dec.text ? dec.text.toLowerCase() : "";
                const assetMatch = msg.asset_name.toLowerCase().includes(q);
                return rn.includes(q) || body.includes(q) || msg.message.toLowerCase().includes(q) || assetMatch;
            });
        }
        if (!settings.showExpired) {
            const now = Date.now();
            msgs = msgs.filter((msg) => {
                if (msg.expire_time == null && msg.expire_utc_time == null) return true;
                const exp = parseTime(msg.expire_utc_time ?? msg.expire_time);
                return exp > now;
            });
        }
        if (!historyOverride && settings.historyDays > 0) {
            const cutoff = Date.now() - settings.historyDays * 86400000;
            msgs = msgs.filter((msg) => parseTime(msg.time) >= cutoff);
        }
        msgs.sort((a, b) => parseTime(a.time) - parseTime(b.time));
        return msgs;
    })();

    $: pagedMessages = filteredMessages.length <= showCount
        ? filteredMessages
        : filteredMessages.slice(filteredMessages.length - showCount);
    $: hasMore = filteredMessages.length > showCount;
    $: olderCount = hasMore ? filteredMessages.length - showCount : 0;

    $: messageRows = pagedMessages.map((msg) => ({
        msg,
        rootName: deriveRootNameFn(msg.asset_name),
        decoded: decodeCache[msg.message],
        time: formatTime(msg.time),
        isMuted: false,
    }));

    /** @param {string|number|undefined} raw */
    function formatTime(raw) {
        const ms = parseTime(raw);
        if (ms === Date.now() && (raw == null || raw === "")) return "";
        const d = new Date(ms);
        const pad = (/** @type {number} */ n) => String(n).padStart(2, "0");
        if (Date.now() - ms > 86400000) {
            return `[${pad(d.getMonth() + 1)}/${pad(d.getDate())}]`;
        }
        return `[${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}]`;
    }

    /** @param {string|number|undefined|null} raw */
    function formatFullTime(raw) {
        const ms = parseTime(raw);
        if (ms === Date.now() && (raw == null || raw === "")) return "";
        return new Date(ms).toLocaleString();
    }

    function refresh() {
        showCount = pageSize;
        loadMessages();
    }

    function loadMore() {
        if (!historyOverride && settings.historyDays > 0 && olderCount > 0) {
            historyOverride = true;
            showCount = pageSize;
        } else {
            showCount = showCount + pageSize;
        }
    }

    function clearSearch() {
        searchFilter = "";
        searchUserFilter = "";
        searchChannelFilter = "";
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
                    discovered = raw.split(/\n/).map((s) => s.trim()).filter((s) => isH0xCAsset(s));
                }
            } else if (Array.isArray(raw)) {
                discovered = raw.map((item) => typeof item === "string" ? item : item?.name || item?.asset_name || "").filter(Boolean);
            }

            const h0xcDiscovered = discovered.filter((s) => isH0xCAsset(s));
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
    function requestWalletUnlock(callback) {
        unlockPassword = "";
        unlockPin = "";
        unlockError = "";
        walletUnlockMode = "passphrase";
        walletPinStatus = null;
        showUnlockModal = true;
        unlockCallback = callback;
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

    async function doUnlockAndRetry() {
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
                showUnlockModal = false;
                unlockPin = "";
                if (unlockCallback) {
                    const cb = unlockCallback;
                    unlockCallback = null;
                    cb();
                }
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
        const { hex, text } = e.detail;
        const channel = identity.replace(/!$/, "");
        const expiry = settings.messageExpiryDefault > 0
            ? Math.floor(Date.now() / 1000) + settings.messageExpiryDefault * 86400
            : null;

        try {
            await ensureNodeSyncedForBroadcast();
        } catch (err) {
            composeError = String(err?.message || err);
            return;
        }

        if (settings.showBroadcastPreview) {
            let feeEstimate = null;
            try {
                feeEstimate = await core.invoke("estimate_announcement_fee");
            } catch { /* fee estimation unavailable */ }
            broadcastPreview = { hex, text, channel, expiry, warnings: [], feeEstimate };
            broadcastError = "";
            return;
        }

        doBroadcast(hex, text, channel, expiry);
    }

    function cancelBroadcastPreview() {
        broadcastPreview = null;
        broadcastError = "";
    }

    function confirmBroadcastPreview() {
        if (!broadcastPreview) return;
        const { hex, text, channel, expiry } = broadcastPreview;
        broadcastPreview = null;
        broadcastError = "";
        doBroadcast(hex, text, channel, expiry);
    }

    /** @param {string} hex @param {string} text @param {string} channel @param {number|null} expiry */
    async function doBroadcast(hex, text, channel, expiry) {
        composeBusy = true;
        composeError = "";

        try {
            await ensureNodeSyncedForBroadcast();
        } catch (err) {
            composeError = String(err?.message || err);
            composeBusy = false;
            return;
        }

        const pendingId = `pending-${++pendingIdCounter}-${Date.now()}`;
        const pendingEntry = {
            id: pendingId,
            hex,
            text,
            assetName: channel,
            rootName: rootName(),
            timeMs: Date.now(),
            status: "pending",
        };
        pendingMessages = [...pendingMessages, pendingEntry];

        try {
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
            pendingMessages = pendingMessages.map((p) =>
                p.id === pendingId ? { ...p, txid: String(txid || "") } : p
            );

            if (composeRef && composeRef.clearCompose) composeRef.clearCompose();

            addNotification({
                type: "message",
                severity: "success",
                title: "H0XC Message Sent",
                body: `Broadcast on ${channel}`,
                action: { label: "Copy TXID", txid },
            });

            schedulePendingTimeout();
            setTimeout(() => loadMessages(), 1500);
            setTimeout(() => loadMessages(), 4000);
        } catch (err) {
            const errorText = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(errorText)) {
                pendingMessages = pendingMessages.filter((p) => p.id !== pendingId);
                requestWalletUnlock(() => doBroadcast(hex, text, channel, expiry));
                return;
            }
            pendingMessages = pendingMessages.map((p) =>
                p.id === pendingId ? { ...p, status: "failed", error: errorText } : p
            );
            composeError = errorText;
            addNotification({
                type: "message",
                severity: "error",
                title: "H0XC Send Failed",
                body: errorText,
            });
        } finally {
            composeBusy = false;
        }
    }

    function schedulePendingTimeout() {
        if (pendingTimeout) clearTimeout(pendingTimeout);
        if (!pendingMessages.some((p) => p.status === "pending")) {
            pendingTimeout = null;
            return;
        }
        pendingTimeout = setTimeout(() => {
            pendingTimeout = null;
            const now = Date.now();
            let changed = false;
            pendingMessages = pendingMessages.map((p) => {
                if (p.status === "pending" && now - p.timeMs > PENDING_TIMEOUT_MS) {
                    changed = true;
                    return { ...p, status: "failed", error: "Message was not found on-chain after waiting. Check again, or restore the text and send it again." };
                }
                return p;
            });
            if (changed) pendingMessages = pendingMessages;
            schedulePendingTimeout();
        }, 5000);
    }

    /** @param {string} pendingId */
    async function retryPending(pendingId) {
        const entry = pendingMessages.find((p) => p.id === pendingId);
        if (!entry || entry.status !== "failed") return;
        pendingMessages = pendingMessages.map((p) =>
            p.id === pendingId
                ? { ...p, status: "pending", error: "Checking for on-chain confirmation..." }
                : p
        );
        await loadMessages();
        if (pendingMessages.some((p) => p.id === pendingId)) {
            pendingMessages = pendingMessages.map((p) =>
                p.id === pendingId
                    ? { ...p, status: "failed", error: "Still not found on-chain. Check the transaction journal before resending." }
                    : p
            );
        }
    }

    /** @param {string} pendingId */
    function dismissPending(pendingId) {
        pendingMessages = pendingMessages.filter((p) => p.id !== pendingId);
    }

    /** @param {string} pendingId */
    function restorePendingToComposer(pendingId) {
        const entry = pendingMessages.find((p) => p.id === pendingId);
        if (!entry) return;
        composeRef?.restoreCompose?.(entry.text);
        pendingMessages = pendingMessages.filter((p) => p.id !== pendingId);
    }

    function reconcilePending() {
        if (pendingMessages.length === 0) return;
        const remaining = [];
        for (const p of pendingMessages) {
            const found = messages.some((m) => pendingMatchesMessage(p, m));
            if (found) {
                addNotification({
                    type: "message",
                    severity: "success",
                    title: "H0XC Message Confirmed",
                    body: `"${p.text.slice(0, 40)}${p.text.length > 40 ? "..." : ""}" confirmed on-chain`,
                });
            } else {
                remaining.push(p);
            }
        }
        if (remaining.length !== pendingMessages.length) {
            pendingMessages = remaining;
        }
    }

    /**
     * Match optimistic rows against messages returned by Core.
     * Txid is preferred when present. The time window fallback handles older
     * Core builds or cached entries that may not expose txid yet.
     * @param {{txid?: string, assetName: string, hex: string, timeMs: number}} pending
     * @param {AssetMessage} msg
     */
    function pendingMatchesMessage(pending, msg) {
        if (pending.txid && msg.txid && String(msg.txid) === pending.txid) return true;
        if (msg.asset_name !== pending.assetName) return false;
        if (msg.message !== pending.hex) return false;
        const msgTime = parseTime(msg.time);
        return msgTime >= pending.timeMs - 60000 && msgTime <= pending.timeMs + 5 * 60000;
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
        if (refreshIndicatorTimer) {
            clearTimeout(refreshIndicatorTimer);
            refreshIndicatorTimer = null;
        }
        if (pendingTimeout) {
            clearTimeout(pendingTimeout);
            pendingTimeout = null;
        }
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

    function openMsgDetail(e, row) {
        e.preventDefault();
        e.stopPropagation();
        msgDetail = row;
        if (!resolvedAddresses[row.rootName]) {
            resolveUserAddress(row.rootName);
        }
    }

    function openUserContext(e, clickedRoot) {
        e.preventDefault();
        e.stopPropagation();
        ctxUser = clickedRoot;
        ctxX = e.clientX;
        ctxY = e.clientY;
        resolveUserAddress(clickedRoot);
    }

    function closeUserContext() {
        ctxUser = null;
    }

    function closeMsgDetail() {
        msgDetail = null;
    }

    function filterByUser(rootName) {
        searchUserFilter = rootName;
        searchFilter = "";
        searchChannelFilter = "";
        searchOpen = true;
        msgDetail = null;
        showCount = pageSize;
    }

    function filterByChannel(assetName) {
        searchChannelFilter = assetName;
        searchFilter = "";
        searchUserFilter = "";
        searchOpen = true;
        msgDetail = null;
        showCount = pageSize;
    }

    async function copyText(text) {
        try { await navigator.clipboard.writeText(text); } catch {}
    }

    function isOwnMessage(msg) {
        const msgRoot = deriveRootNameFn(msg.asset_name);
        return msgRoot.toUpperCase() === rootName().toUpperCase();
    }

    function canDeleteMessage(msg) {
        if (isGuest || !msg.txid) return false;
        const identityChannel = canonicalMessageChannel({
            asset_name: identity,
        });
        const msgChannel = canonicalMessageChannel(msg);
        return identityChannel !== "" && msgChannel !== "" && identityChannel === msgChannel;
    }

    async function handleDeleteMessage() {
        if (!msgDetail?.msg || !canDeleteMessage(msgDetail.msg)) return;
        await ensureNodeSyncedForBroadcast();
        const deletedTxid = msgDetail.msg.txid;
        const hex = await buildDeleteCommandHex(deletedTxid);
        const channel = identity.replace(/!$/, "");
        await core.invoke("preview_send_announcement", {
            channelName: channel,
            ipfsHash: hex,
            expireTime: null,
        });
        const txid = await core.invoke("send_announcement", {
            channelName: channel,
            ipfsHash: hex,
            expireTime: null,
        });
        addNotification({
            type: "message",
            severity: "success",
            title: "Delete Command Sent",
            body: `Delete broadcast on ${channel}`,
        });
        hiddenTxids = new Set([...hiddenTxids, deletedTxid]);
        hideMessageLocally(deletedTxid, 0);
        msgDetail = null;
        setTimeout(() => loadMessages(), 2000);
    }

    function requestDeleteMessage() {
        if (!msgDetail?.msg?.txid) return;
        const doDelete = () => handleDeleteMessage().catch((err) => {
            const text = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(text)) {
                requestWalletUnlock(doDelete);
                return;
            }
            addNotification({
                type: "message",
                severity: "error",
                title: "Delete Failed",
                body: text,
            });
        });
        doDelete();
    }

    // Leave / Rejoin
    let showLeaveConfirm = false;
    let leaveBusy = false;
    let leaveError = "";

    function openLeaveConfirm() {
        showLeaveConfirm = true;
        leaveError = "";
    }

    function closeLeaveConfirm() {
        showLeaveConfirm = false;
        leaveError = "";
    }

    async function handleLeaveChat() {
        if (isGuest || !identity) {
            leaveError = "Cannot leave as guest.";
            return;
        }
        leaveBusy = true;
        leaveError = "";
        try {
            await ensureNodeSyncedForBroadcast();
            const hex = await buildLeaveCommandHex();
            const channel = identity.replace(/!$/, "");
            await core.invoke("preview_send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });
            const txid = await core.invoke("send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });
            addNotification({
                type: "message",
                severity: "success",
                title: "Leave Command Sent",
                body: `Leave broadcast on ${channel}`,
            });
            showLeaveConfirm = false;
            setTimeout(() => loadMessages(), 2000);
        } catch (err) {
            const text = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(text)) {
                requestWalletUnlock(() => handleLeaveChat());
                return;
            }
            leaveError = text;
            addNotification({
                type: "message",
                severity: "error",
                title: "Leave Failed",
                body: text,
            });
        } finally {
            leaveBusy = false;
        }
    }

    // --- Status control ---

    function toggleStatus() {
        statusOpen = !statusOpen;
        statusError = "";
    }

    function closeStatus() {
        statusOpen = false;
        statusError = "";
    }

    async function sendStatus(status, expiryMode, expiryValue) {
        if (isGuest || !identity) {
            statusError = "Cannot set status as guest.";
            return;
        }
        statusBusy = true;
        statusError = "";
        try {
            await ensureNodeSyncedForBroadcast();
            const hex = await buildStatusCommandHex(status, expiryMode, expiryValue);
            const channel = identity.replace(/!$/, "");
            await core.invoke("preview_send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });
            await core.invoke("send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });
            addNotification({
                type: "message",
                severity: "success",
                title: "Status Updated",
                body: `Status broadcast on ${channel}`,
            });
            statusOpen = false;
            setTimeout(() => loadMessages(), 2000);
        } catch (err) {
            const text = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(text)) {
                requestWalletUnlock(() => sendStatus(status, expiryMode, expiryValue));
                return;
            }
            statusError = text;
        } finally {
            statusBusy = false;
        }
    }

    function selectedStatusExpiryValue() {
        if (selectedExpiryMode === 1) {
            return Math.max(1, Math.min(2160, Number(selectedExpiryHours) || 4));
        }
        if (selectedExpiryMode === 2) {
            const selectedMs = selectedExpiryDateTime ? new Date(selectedExpiryDateTime).getTime() : NaN;
            if (!Number.isFinite(selectedMs)) {
                statusError = "Choose a valid status expiry date and time.";
                return null;
            }
            const now = Date.now();
            const max = now + 90 * 86400000;
            if (selectedMs <= now) {
                statusError = "Status expiry must be in the future.";
                return null;
            }
            if (selectedMs > max) {
                statusError = "Status expiry cannot be more than 90 days out.";
                return null;
            }
            return Math.floor(selectedMs / 1000);
        }
        return 0;
    }

    function defaultStatusDateTime() {
        const d = new Date(Date.now() + 24 * 3600000);
        const pad = (n) => String(n).padStart(2, "0");
        return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
    }

    // --- Report control ---

    function openReportMessage(txid, label) {
        reportTargetType = 1;
        reportTargetId = txid;
        reportTargetLabel = label || txid?.slice(0, 16) + "...";
        reportReason = 1;
        reportSeverity = 2;
        reportDurationDays = 30;
        reportOpen = true;
        reportError = "";
    }

    function openReportChannel(channel, label) {
        reportTargetType = 2;
        reportTargetId = channel;
        reportTargetLabel = label || channel;
        reportReason = 1;
        reportSeverity = 2;
        reportDurationDays = 30;
        reportOpen = true;
        reportError = "";
    }

    function closeReport() {
        reportOpen = false;
        reportError = "";
    }

    async function sendReport() {
        if (isGuest || !identity) {
            reportError = "Cannot report as guest.";
            return;
        }
        if (!reportTargetId) {
            reportError = "No target selected.";
            return;
        }
        reportBusy = true;
        reportError = "";
        try {
            await ensureNodeSyncedForBroadcast();
            let hex;
            if (reportTargetType === 1) {
                hex = await buildReportMessageCommandHex(
                    reportTargetId,
                    reportReason,
                    reportSeverity,
                    reportDurationDays
                );
            } else {
                hex = await buildReportChannelCommandHex(
                    reportTargetId,
                    reportReason,
                    reportSeverity,
                    reportDurationDays
                );
            }
            const channel = identity.replace(/!$/, "");
            await core.invoke("preview_send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });
            await core.invoke("send_announcement", {
                channelName: channel,
                ipfsHash: hex,
                expireTime: null,
            });

            // Hide locally immediately
            if (reportTargetType === 1) {
                hideMessageLocally(reportTargetId, reportReason);
            } else {
                hideChannelLocally(reportTargetId, reportReason);
            }

            addNotification({
                type: "message",
                severity: "success",
                title: "Report Sent",
                body: `Report broadcast on ${channel}`,
            });
            reportOpen = false;
            setTimeout(() => loadMessages(), 2000);
        } catch (err) {
            const text = String(err);
            if (/wallet.*locked|passphrase|unlock/i.test(text)) {
                requestWalletUnlock(() => sendReport());
                return;
            }
            reportError = text;
        } finally {
            reportBusy = false;
        }
    }

    function handleUnhideMessage(txid) {
        unhideMessageLocally(txid);
    }

    function handleUnhideChannel(channel) {
        unhideChannelLocally(channel);
    }

    function handleAllowCommunityHidden(target) {
        addOverrideHidden(target);
        updateCommunityHiddenFromReports();
    }

    /** @param {{ rootName: string, address: string, tag: string }} detail */
    function handleAddTag(detail) {
        const tag = detail.tag.startsWith("#") ? detail.tag : `#${detail.tag}`;
        const currentTags = settings.autoBlockTags || [];
        if (!currentTags.includes(tag)) {
            settings.autoBlockTags = [...currentTags, tag];
            settings = settings;
            addNotification({
                type: "message",
                severity: "info",
                title: "Tag Added",
                body: `Added "${tag}" to auto-block tags`,
            });
            setTimeout(() => maybeCheckChannelTags(), 500);
        }
    }
</script>

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
            <HelpHitbox title="H0XC Community Chat" right={true}>
                <p><strong>H0XC</strong> is a public, on-chain community chat system. Every message is broadcast as an asset message on the Hemp0x blockchain.</p>
                <p><strong>How it works:</strong></p>
                <ul>
                    <li>Each participant owns a <code>YOURROOT/H0XC</code> sub-asset. The wallet holding this asset's authority can broadcast messages.</li>
                    <li>Messages are stored <strong>permanently</strong> on-chain. They are <strong>not encrypted</strong> and are visible to anyone.</li>
                    <li>Control frames (e.g., delete commands) are hidden from chat. Expired messages are hidden by default but can be shown in settings.</li>
                    <li>User moderation is local-only: mute, block, or auto-block by tags. These affect your view only.</li>
                    <li>Discovery scans the network for <code>*.H0XC</code> assets. Background discovery keeps the participant list up to date.</li>
                </ul>
                <p><strong>For private messaging, visit <a href="https://hemp0x.social" target="_blank" rel="noopener" style="color:var(--color-primary)">hemp0x.social</a>.</strong></p>
            </HelpHitbox>
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
            <button class="header-btn" class:active={statusOpen} on:click={toggleStatus} title="Set status">
                {#if myStatusValue !== undefined && myStatusValue !== 4}
                    <span class="status-active-icon">{STATUS_ICONS[myStatusValue] || "●"}</span>
                {:else}
                    ◉
                {/if}
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
        {#if tagLookupPending}
            <span class="status-pill">Tags...</span>
        {/if}
        {#if discovering}
            <span class="status-pill">
                <span class="inline-spinner"></span>
                Scanning{discoveryProgress ? ` ${discoveryProgress}` : ""}
            </span>
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
                placeholder="Search text..."
                aria-label="Search messages by text"
            />
            <input
                class="search-input search-user"
                type="text"
                bind:value={searchUserFilter}
                on:input={() => { showCount = pageSize; }}
                placeholder="User..."
                aria-label="Filter by user"
            />
            <input
                class="search-input search-channel"
                type="text"
                bind:value={searchChannelFilter}
                on:input={() => { showCount = pageSize; }}
                placeholder="Channel..."
                aria-label="Filter by channel"
            />
            {#if searchFilter || searchUserFilter || searchChannelFilter}
                <button class="search-clear" on:click={clearSearch} title="Clear all filters">✕</button>
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
                    {#if messages.length > 0 && filteredMessages.length === 0}
                        <div class="empty-line sub">Core loaded {messages.length} H0xC message record(s), but none decoded as displayable chat under the current filters.</div>
                    {:else}
                        <div class="empty-line sub">Messages load from Core's local message index. Use Discover to find H0XC participants.</div>
                    {/if}
                    {#if settings.discoveryEnabled}
                        <button class="empty-discover" on:click={() => discover(false, true)}>Discover</button>
                    {:else}
                        <div class="empty-line sub">Discovery is disabled. Enable it in Settings to find participants.</div>
                    {/if}
                </div>
            {:else}
                {#if hasMore}
                    <button class="show-more-btn load-older" on:click={loadMore}>
                        {#if !historyOverride && settings.historyDays > 0}
                            Load older messages ({olderCount} beyond {settings.historyDays}-day window)
                        {:else}
                            Load older messages ({olderCount} remaining)
                        {/if}
                    </button>
                {/if}
                {#each messageRows as row (row.msg.asset_name + row.msg.time + row.msg.message)}
                    <div class="chat-msg" class:me={row.rootName.toUpperCase() === rootName().toUpperCase()}>
                        <span class="msg-time">{row.time}</span>
                        <span
                            class="msg-user"
                            class:me={row.rootName.toUpperCase() === rootName().toUpperCase()}
                            on:click|preventDefault={(e) => openUserContext(e, row.rootName)}
                            on:contextmenu|preventDefault={(e) => openUserContext(e, row.rootName)}
                            role="button"
                            tabindex="0"
                            on:keydown={(e) => e.key === "Enter" && openUserContext(e, row.rootName)}
                        >[{row.rootName.toUpperCase()}]</span>
                        <span
                            class="msg-body"
                            role="button"
                            tabindex="0"
                            title="Open message details"
                            on:click={(e) => openMsgDetail(e, row)}
                            on:keydown={(e) => e.key === "Enter" && openMsgDetail(e, row)}
                        >
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

                {#each pendingMessages as pm (pm.id)}
                    {#if pm.status === "pending"}
                        <div class="chat-msg me pending-msg">
                            <span class="msg-time">{formatTime(pm.timeMs)}</span>
                            <span class="msg-user me">[{pm.rootName.toUpperCase()}]</span>
                            <span class="msg-body">
                                <span class="msg-short-text">{pm.text}</span>
                                <span class="pending-dot"></span>
                            </span>
                        </div>
                    {:else}
                        <div class="chat-msg me pending-msg failed">
                            <span class="msg-time">{formatTime(pm.timeMs)}</span>
                            <span class="msg-user me">[{pm.rootName.toUpperCase()}]</span>
                            <span class="msg-body">
                                <span class="msg-short-text">{pm.text}</span>
                                <span class="failed-badge" title={pm.error || "Send failed"}>
                                    <span class="failed-icon">✕</span>
                                    <span class="failed-label">not found</span>
                                </span>
                            </span>
                            <div class="pending-actions">
                                <button class="pending-btn retry" on:click={() => retryPending(pm.id)} title="Check on-chain again">↻</button>
                                <button class="pending-btn restore" on:click={() => restorePendingToComposer(pm.id)} title="Restore text to composer">↥</button>
                                <button class="pending-btn dismiss" on:click={() => dismissPending(pm.id)} title="Dismiss">✕</button>
                            </div>
                        </div>
                    {/if}
                {/each}
            {/if}

            {#if showRefreshIndicator}
                <div class="chat-loading-more">
                    <span class="inline-spinner"></span>
                    <span>Refreshing messages...</span>
                </div>
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
            hideStaleUsers={settings.hideStaleUsers !== false}
            staleUserDays={settings.staleUserDays || 90}
            {leftChannels}
            {statusByChannel}
            on:mute={handleMute}
            on:block={handleBlock}
            on:viewDetails={handleViewDetails}
            on:blockAndUnsub={(e) => { blockAndUnsubscribe(e.detail.rootName); }}
            on:manageTags={() => openManageTags()}
            on:filterByUser={(e) => { filterByUser(e.detail.rootName); }}
            on:addTag={(e) => { handleAddTag(e.detail); }}
            on:leave={openLeaveConfirm}
        />
    </div>

    {#if showLeaveConfirm}
        <div class="leave-confirm-bar" transition:fade={{ duration: 100 }}>
            <div class="leave-confirm-body">
                <span class="leave-confirm-title">LEAVE H0XC CHAT</span>
                <p class="leave-confirm-text">
                    This broadcasts an on-chain control message. It only hides you from Commander user lists.
                    Old messages remain on-chain. You can rejoin by sending a new normal message later.
                </p>
                {#if leaveError}
                    <span class="leave-confirm-error">{leaveError}</span>
                {/if}
                <div class="leave-confirm-actions">
                    <button class="leave-btn cancel" on:click={closeLeaveConfirm} disabled={leaveBusy}>Cancel</button>
                    <button class="leave-btn confirm" on:click={handleLeaveChat} disabled={leaveBusy}>
                        {leaveBusy ? "Broadcasting..." : "Leave chat"}
                    </button>
                </div>
            </div>
        </div>
    {/if}

    {#if broadcastPreview}
        <div class="preview-bar" transition:fade={{ duration: 100 }}>
            <div class="preview-body">
                <span class="preview-title">BROADCAST PREVIEW</span>
                {#if broadcastError}
                    <span class="preview-error">{broadcastError}</span>
                {/if}
                <div class="preview-row">
                    <span class="preview-label">CHANNEL</span>
                    <span class="preview-val mono">{broadcastPreview.channel}</span>
                </div>
                <div class="preview-row">
                    <span class="preview-label">TEXT</span>
                    <span class="preview-val">{broadcastPreview.text}</span>
                </div>
                <div class="preview-row">
                    <span class="preview-label">HEX</span>
                    <span class="preview-val mono hex-preview">{broadcastPreview.hex.length > 48 ? broadcastPreview.hex.slice(0, 24) + "…" + broadcastPreview.hex.slice(-24) : broadcastPreview.hex}</span>
                </div>
                {#if broadcastPreview.expiry}
                    <div class="preview-row">
                        <span class="preview-label">EXPIRES</span>
                        <span class="preview-val">{new Date(broadcastPreview.expiry * 1000).toLocaleString()}</span>
                    </div>
                {/if}
                {#if broadcastPreview.feeEstimate}
                    <div class="preview-row">
                        <span class="preview-label">FEE (EST)</span>
                        <span class="preview-val mono">{broadcastPreview.feeEstimate} HEMP</span>
                    </div>
                {/if}
                <div class="preview-warning">
                    <span class="preview-warn-icon">⚠</span>
                    <span class="preview-warn-text">This creates an irreversible on-chain transaction. Fee is determined by Core at broadcast time.</span>
                </div>
                <div class="preview-note">
                    Broadcast previews can be turned off in H0XC chat settings.
                </div>
                {#if broadcastPreview.warnings.length > 0}
                    {#each broadcastPreview.warnings as w}
                        <div class="preview-warning">
                            <span class="preview-warn-icon">ℹ</span>
                            <span class="preview-warn-text">{w}</span>
                        </div>
                    {/each}
                {/if}
                <div class="preview-actions">
                    <button class="preview-btn cancel" on:click={cancelBroadcastPreview} disabled={broadcastBusy}>Cancel</button>
                    <button class="preview-btn broadcast" on:click={confirmBroadcastPreview} disabled={broadcastBusy}>
                        {broadcastBusy ? "Broadcasting..." : "Broadcast"}
                    </button>
                </div>
            </div>
        </div>
    {/if}

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
        localHiddenMessages={localHiddenMessagesList}
        localHiddenChannels={localHiddenChannelsList}
        communityHidden={communityHiddenList}
        on:close={() => (settingsOpen = false)}
        on:save={handleSaveSettings}
        on:unblock={(e) => { blockedUsers = blockedUsers.filter((u) => u !== e.detail.rootName); }}
        on:unhideMessage={(e) => handleUnhideMessage(e.detail.txid)}
        on:unhideChannel={(e) => handleUnhideChannel(e.detail.channel)}
        on:allowCommunityHidden={(e) => handleAllowCommunityHidden(e.detail.target)}
    />

    {#if statusOpen}
        <div class="status-overlay" role="dialog" aria-modal="true" on:click={closeStatus} on:keydown={(e) => e.key === "Escape" && closeStatus()} tabindex="0" transition:fade={{ duration: 80 }}>
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div class="status-panel" on:click|stopPropagation on:keydown|stopPropagation role="document">
                <div class="status-header">
                    <span class="status-title">SET STATUS</span>
                    <button class="status-close" on:click={closeStatus}>&times;</button>
                </div>
                <div class="status-body">
                    {#if statusError}
                        <div class="status-error">{statusError}</div>
                    {/if}
                    <div class="status-options">
                        {#each Object.entries(STATUS_LABELS) as [val, label]}
                            {@const v = parseInt(val)}
                            {#if v !== 4}
                                <button
                                    class="status-opt"
                                    class:active={selectedStatus === v}
                                    on:click={() => { selectedStatus = v; }}
                                    disabled={statusBusy}
                                >
                                    <span class="status-opt-icon">{STATUS_ICONS[v]}</span>
                                    <span class="status-opt-label">{label}</span>
                                </button>
                            {/if}
                        {/each}
                    </div>
                    <div class="status-expiry-section">
                        <span class="status-section-label">EXPIRY</span>
                        <div class="status-expiry-row">
                            <button class="status-expiry-btn" class:active={selectedExpiryMode === 0} on:click={() => { selectedExpiryMode = 0; }} disabled={statusBusy}>24 Hours</button>
                            <button class="status-expiry-btn" class:active={selectedExpiryMode === 1} on:click={() => { selectedExpiryMode = 1; selectedExpiryHours = 4; }} disabled={statusBusy}>Custom</button>
                            <button class="status-expiry-btn" class:active={selectedExpiryMode === 2} on:click={() => { selectedExpiryMode = 2; if (!selectedExpiryDateTime) selectedExpiryDateTime = defaultStatusDateTime(); }} disabled={statusBusy}>Until Date</button>
                            <button class="status-expiry-btn" class:active={selectedExpiryMode === 3} on:click={() => { selectedExpiryMode = 3; }} disabled={statusBusy}>Until Changed</button>
                        </div>
                        {#if selectedExpiryMode === 1}
                            <div class="status-hours-row">
                                <span class="status-hours-label">Hours:</span>
                                <input type="number" class="cyber-input" bind:value={selectedExpiryHours} min="1" max="2160" disabled={statusBusy} />
                            </div>
                        {:else if selectedExpiryMode === 2}
                            <div class="status-hours-row">
                                <span class="status-hours-label">Until:</span>
                                <input type="datetime-local" class="cyber-input" bind:value={selectedExpiryDateTime} disabled={statusBusy} />
                            </div>
                        {/if}
                    </div>
                    <div class="status-actions">
                        <button class="status-btn cancel" on:click={closeStatus} disabled={statusBusy}>Cancel</button>
                        <button class="status-btn clear" on:click={() => sendStatus(4, 0, 0)} disabled={statusBusy}>Clear Status</button>
                        <button class="status-btn set" on:click={() => {
                            const expiryValue = selectedStatusExpiryValue();
                            if (expiryValue !== null) sendStatus(selectedStatus, selectedExpiryMode, expiryValue);
                        }} disabled={statusBusy}>
                            {statusBusy ? "Sending..." : "Set Status"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    {/if}

    {#if reportOpen}
        <div class="report-overlay" role="dialog" aria-modal="true" on:click={closeReport} on:keydown={(e) => e.key === "Escape" && closeReport()} tabindex="0" transition:fade={{ duration: 80 }}>
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div class="report-panel" on:click|stopPropagation on:keydown|stopPropagation role="document">
                <div class="report-header">
                    <span class="report-title">REPORT {reportTargetType === 1 ? "MESSAGE" : "CHANNEL"}</span>
                    <button class="report-close" on:click={closeReport}>&times;</button>
                </div>
                <div class="report-body">
                    {#if reportError}
                        <div class="report-error">{reportError}</div>
                    {/if}
                    <div class="report-target">
                        <span class="report-target-label">Target:</span>
                        <span class="report-target-val">{reportTargetLabel}</span>
                    </div>
                    <div class="report-section">
                        <span class="report-section-label">REASON</span>
                        <div class="report-options">
                            {#each Object.entries(REASON_LABELS) as [val, label]}
                                {@const v = parseInt(val)}
                                <button
                                    class="report-opt"
                                    class:active={reportReason === v}
                                    on:click={() => { reportReason = v; }}
                                    disabled={reportBusy}
                                >{label}</button>
                            {/each}
                        </div>
                    </div>
                    <div class="report-section">
                        <span class="report-section-label">SEVERITY</span>
                        <div class="report-options">
                            {#each Object.entries(SEVERITY_LABELS) as [val, label]}
                                {@const v = parseInt(val)}
                                <button
                                    class="report-opt"
                                    class:active={reportSeverity === v}
                                    on:click={() => { reportSeverity = v; }}
                                    disabled={reportBusy}
                                >{label}</button>
                            {/each}
                        </div>
                    </div>
                    <div class="report-section">
                        <span class="report-section-label">DURATION</span>
                        <div class="report-options">
                            <button class="report-opt" class:active={reportDurationDays === 7} on:click={() => { reportDurationDays = 7; }} disabled={reportBusy}>7 Days</button>
                            <button class="report-opt" class:active={reportDurationDays === 30} on:click={() => { reportDurationDays = 30; }} disabled={reportBusy}>30 Days</button>
                            <button class="report-opt" class:active={reportDurationDays === 90} on:click={() => { reportDurationDays = 90; }} disabled={reportBusy}>90 Days</button>
                            <button class="report-opt" class:active={reportDurationDays === 180} on:click={() => { reportDurationDays = 180; }} disabled={reportBusy}>180 Days</button>
                        </div>
                    </div>
                    <div class="report-actions">
                        <button class="report-btn cancel" on:click={closeReport} disabled={reportBusy}>Cancel</button>
                        <button class="report-btn send" on:click={sendReport} disabled={reportBusy}>
                            {reportBusy ? "Sending..." : "Send Report"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    {/if}

    <H0xCUserContextMenu
        x={ctxX}
        y={ctxY}
        user={ctxUser || ""}
        muted={ctxUser ? mutedUsers.includes(ctxUser) : false}
        blocked={ctxUser ? blockedUsers.includes(ctxUser) : false}
        resolvedAddress={ctxUser ? resolvedAddresses[ctxUser] || "" : ""}
        channelAsset={ctxParticipant?.assetName || ""}
        lastSeen={ctxParticipant?.lastSeen || 0}
        joinedAt={ctxParticipant?.joinedAt || 0}
        messageCount={ctxParticipant?.messageCount || 0}
        isSelf={ctxIsSelf}
        isLeft={ctxIsSelf && leftChannels.has(identity?.replace(/!$/, "").trim().toUpperCase())}
        on:viewDetails={(e) => { handleViewDetails(e); closeUserContext(); }}
        on:mute={(e) => { handleMute(e); closeUserContext(); }}
        on:block={(e) => { handleBlock(e); closeUserContext(); }}
        on:blockAndUnsub={(e) => { blockAndUnsubscribe(e.detail.rootName); closeUserContext(); }}
        on:manageTags={() => { openManageTags(); closeUserContext(); }}
        on:filterByUser={(e) => { filterByUser(e.detail.rootName); closeUserContext(); }}
        on:addTag={(e) => { handleAddTag(e.detail); closeUserContext(); }}
        on:leave={() => { openLeaveConfirm(); closeUserContext(); }}
        on:reportChannel={(e) => { openReportChannel(e.detail.channel, e.detail.rootName); closeUserContext(); }}
        on:close={closeUserContext}
    />

    <WalletUnlockModal
        show={showUnlockModal}
        mode={walletUnlockMode}
        bind:password={unlockPassword}
        bind:pin={unlockPin}
        {unlocking}
        error={unlockError}
        title={walletUnlockMode === "pin" && walletPinUsable ? "UNLOCK WITH DEVICE PIN" : "UNLOCK WALLET"}
        body={walletUnlockMode === "pin" && walletPinUsable ? "Enter this device's 6-digit PIN to unlock the local Core wallet for signing." : "Wallet unlock required to send community chat messages."}
        confirmLabel={walletUnlockMode === "pin" && walletPinUsable ? "UNLOCK" : "UNLOCK AND SEND"}
        pinConfigured={walletPinUsable}
        pinRequiresPassphrase={walletUnlockMode === "pin" && pinRequiresPassphrase(walletPinStatus)}
        pinRequiresPassphraseReason={walletPinStatus?.reason || ""}
        lockoutRemainingSecs={walletPinStatus?.lockout_remaining_secs || 0}
        on:cancel={() => { showUnlockModal = false; unlockPassword = ""; unlockPin = ""; unlockError = ""; unlockCallback = null; }}
        on:confirm={doUnlockAndRetry}
        on:usepassphrase={switchWalletUnlockToPassphrase}
        on:usepin={switchWalletUnlockToPin}
        on:forgotpin={forgotWalletPinUnlock}
    />

    {#if msgDetail}
        <div class="msg-detail-overlay" on:click={closeMsgDetail} on:keydown={(e) => e.key === "Escape" && closeMsgDetail()} role="dialog" aria-modal="true" tabindex="-1" transition:fade={{ duration: 80 }}>
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div class="msg-detail-panel" on:click|stopPropagation on:keydown|stopPropagation role="document">
                <div class="msg-detail-header">
                    <span class="msg-detail-title">MESSAGE DETAILS</span>
                    <button class="msg-detail-close" on:click={closeMsgDetail}>&times;</button>
                </div>
                <div class="msg-detail-body">
                    {#if msgDetail.decoded?.is_h0xc_chat_message}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">TYPE</span>
                            <span class="msg-detail-val chat-badge">H0XC chat message</span>
                        </div>
                    {/if}
                    <div class="msg-detail-row">
                        <span class="msg-detail-label">TEXT</span>
                        <span class="msg-detail-val">{msgDetail.decoded?.is_short_message && msgDetail.decoded.text ? msgDetail.decoded.text : "(not decoded)"}</span>
                        {#if msgDetail.decoded?.is_short_message && msgDetail.decoded.text}
                            <button class="msg-detail-copy" on:click={() => copyText(msgDetail.decoded.text)} title="Copy text">Copy</button>
                        {/if}
                    </div>
                    <div class="msg-detail-row">
                        <span class="msg-detail-label">RAW HEX</span>
                        <span class="msg-detail-val mono">{msgDetail.msg.message}</span>
                        <button class="msg-detail-copy" on:click={() => copyText(msgDetail.msg.message)} title="Copy hex">Copy</button>
                    </div>
                    {#if msgDetail.msg.txid}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">TXID</span>
                            <span class="msg-detail-val mono">{msgDetail.msg.txid}</span>
                            <button class="msg-detail-copy" on:click={() => copyText(msgDetail.msg.txid)} title="Copy txid">Copy</button>
                        </div>
                    {/if}
                    {#if msgDetail.msg.block_hash}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">BLOCK HASH</span>
                            <span class="msg-detail-val mono">{msgDetail.msg.block_hash}</span>
                            <button class="msg-detail-copy" on:click={() => copyText(msgDetail.msg.block_hash)} title="Copy block hash">Copy</button>
                        </div>
                    {/if}
                    <div class="msg-detail-row">
                        <span class="msg-detail-label">CHANNEL</span>
                        <span class="msg-detail-val mono">{msgDetail.msg.asset_name}</span>
                        <button class="msg-detail-copy" on:click={() => copyText(msgDetail.msg.asset_name)} title="Copy channel">Copy</button>
                        <button class="msg-detail-action" on:click={() => filterByChannel(msgDetail.msg.asset_name)} title="Filter by this channel">Filter</button>
                    </div>
                    <div class="msg-detail-row">
                        <span class="msg-detail-label">SENDER</span>
                        <span class="msg-detail-val">[{msgDetail.rootName.toUpperCase()}]</span>
                        <button class="msg-detail-copy" on:click={() => copyText(msgDetail.rootName)} title="Copy sender">Copy</button>
                        <button class="msg-detail-action" on:click={() => filterByUser(msgDetail.rootName)} title="Filter by this user">Filter</button>
                    </div>
                    {#if resolvedAddresses[msgDetail.rootName]}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">AUTHORITY</span>
                            <span class="msg-detail-val mono">{resolvedAddresses[msgDetail.rootName]}</span>
                            <button class="msg-detail-copy" on:click={() => copyText(resolvedAddresses[msgDetail.rootName])} title="Copy address">Copy</button>
                        </div>
                    {:else}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">AUTHORITY</span>
                            <span class="msg-detail-val dim">Resolving...</span>
                        </div>
                    {/if}
                    <div class="msg-detail-row">
                        <span class="msg-detail-label">TIME</span>
                        <span class="msg-detail-val">{msgDetail.time}</span>
                    </div>
                    {#if msgDetail.msg.block_height}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">BLOCK</span>
                            <span class="msg-detail-val mono">{msgDetail.msg.block_height}</span>
                        </div>
                    {/if}
                    {#if msgDetail.msg.expire_time || msgDetail.msg.expire_utc_time}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">EXPIRES</span>
                            <span class="msg-detail-val">{formatFullTime(msgDetail.msg.expire_utc_time ?? msgDetail.msg.expire_time)}</span>
                        </div>
                    {/if}
                    {#if msgDetail.msg.status}
                        <div class="msg-detail-row">
                            <span class="msg-detail-label">STATUS</span>
                            <span class="msg-detail-val">{msgDetail.msg.status}</span>
                        </div>
                    {/if}
                    {#if !isGuest && msgDetail.msg.txid}
                        <div class="msg-detail-row msg-detail-actions">
                            {#if canDeleteMessage(msgDetail.msg)}
                                <button class="msg-detail-delete" on:click={requestDeleteMessage} title="Send a local delete command for this message">Delete message</button>
                            {/if}
                            <button class="msg-detail-action" on:click={() => openReportMessage(msgDetail.msg.txid, msgDetail.msg.asset_name)} title="Report this message">Report message</button>
                        </div>
                    {/if}
                </div>
            </div>
        </div>
    {/if}
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
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
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
        overflow: hidden;
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
    .chat-messages.loading { /* messages stay visible during refresh */ }
    .inline-spinner {
        width: 10px;
        height: 10px;
        border: 2px solid rgba(0, 255, 65, 0.15);
        border-top-color: var(--color-primary);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
        display: inline-block;
        vertical-align: middle;
        margin-right: 0.3rem;
        flex-shrink: 0;
    }
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
        font-size: 0.55rem;
        color: #777;
        padding: 0.5rem;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.4rem;
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
        min-width: 5.5rem;
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
        color: var(--color-primary);
    }
    .msg-body {
        color: #ccc;
        word-break: break-word;
        min-width: 0;
        cursor: pointer;
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
    .search-user { max-width: 7rem; }
    .search-channel { max-width: 10rem; }
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
        font-size: 0.55rem;
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
    .load-older {
        border-color: rgba(0, 255, 65, 0.15);
        margin-bottom: 0.3rem;
    }

    .msg-detail-overlay {
        position: absolute;
        inset: 0;
        z-index: 200;
        display: flex;
        align-items: stretch;
        justify-content: stretch;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(4px);
        padding: 0.5rem;
    }
    .msg-detail-panel {
        width: 100%;
        height: 100%;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 8px;
        box-shadow: 0 16px 48px rgba(0, 0, 0, 0.85);
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .msg-detail-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.85rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
        background: rgba(0, 0, 0, 0.25);
        flex-shrink: 0;
    }
    .msg-detail-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.2px;
    }
    .msg-detail-close {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .msg-detail-close:hover { color: #fff; }
    .msg-detail-body {
        padding: 0.6rem 0.85rem;
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        overflow-y: auto;
        flex: 1 1 0%;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .msg-detail-body::-webkit-scrollbar { width: 6px; }
    .msg-detail-body::-webkit-scrollbar-track { background: transparent; }
    .msg-detail-body::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.35); border-radius: 3px; }
    .msg-detail-row {
        display: flex;
        align-items: flex-start;
        gap: 0.5rem;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 5px;
        flex-wrap: wrap;
    }
    .msg-detail-label {
        color: #555;
        font-size: 0.5rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        min-width: 4.5rem;
        flex-shrink: 0;
        padding-top: 0.05rem;
    }
    .msg-detail-val {
        color: #ccc;
        font-size: 0.6rem;
        line-height: 1.45;
        flex: 1;
        min-width: 0;
        word-break: break-all;
    }
    .msg-detail-val.mono { font-family: var(--font-mono); }
    .msg-detail-val.dim { color: #666; font-style: italic; }
    .msg-detail-val.chat-badge {
        color: var(--color-primary);
        font-size: 0.55rem;
        font-weight: 600;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        padding: 0.1rem 0.4rem;
        letter-spacing: 0.3px;
    }
    .msg-detail-copy, .msg-detail-action {
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.52rem;
        font-weight: 600;
        padding: 0.15rem 0.35rem;
        cursor: pointer;
        flex-shrink: 0;
        letter-spacing: 0.3px;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .msg-detail-copy:hover, .msg-detail-action:hover {
        background: rgba(0, 255, 65, 0.15);
    }
    .msg-detail-action {
        border-color: rgba(68, 136, 255, 0.2);
        color: #6688cc;
        background: rgba(68, 136, 255, 0.06);
    }
    .msg-detail-action:hover {
        background: rgba(68, 136, 255, 0.15);
    }
    .msg-detail-actions {
        justify-content: flex-end;
        padding-top: 0.3rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        margin-top: 0.15rem;
    }
    .msg-detail-delete {
        background: rgba(255, 60, 60, 0.08);
        border: 1px solid rgba(255, 60, 60, 0.25);
        border-radius: 4px;
        color: #cc4444;
        font-size: 0.52rem;
        font-weight: 600;
        padding: 0.15rem 0.5rem;
        cursor: pointer;
        letter-spacing: 0.3px;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .msg-detail-delete:hover {
        background: rgba(255, 60, 60, 0.18);
        border-color: rgba(255, 60, 60, 0.4);
    }

    .leave-confirm-bar {
        flex-shrink: 0;
        padding: 0.4rem 0.5rem;
        background: rgba(255, 60, 60, 0.04);
        border-top: 1px solid rgba(255, 60, 60, 0.15);
        border-bottom: 1px solid rgba(255, 60, 60, 0.15);
    }
    .leave-confirm-body {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    .leave-confirm-title {
        font-size: 0.55rem;
        font-weight: 700;
        color: #ff8888;
        letter-spacing: 0.5px;
    }
    .leave-confirm-text {
        margin: 0;
        font-size: 0.52rem;
        color: #aaa;
        line-height: 1.45;
    }
    .leave-confirm-error {
        font-size: 0.52rem;
        color: #ff5555;
    }
    .leave-confirm-actions {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
        margin-top: 0.2rem;
    }
    .leave-btn {
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        font-size: 0.52rem;
        font-weight: 600;
        letter-spacing: 0.3px;
        cursor: pointer;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .leave-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .leave-btn.cancel:hover:not(:disabled) {
        border-color: #ff5555;
        color: #ff5555;
    }
    .leave-btn.confirm {
        background: rgba(255, 60, 60, 0.08);
        border: 1px solid rgba(255, 60, 60, 0.25);
        color: #cc4444;
    }
    .leave-btn.confirm:hover:not(:disabled) {
        background: rgba(255, 60, 60, 0.18);
        border-color: rgba(255, 60, 60, 0.4);
    }
    .leave-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .status-active-icon {
        font-size: 0.65rem;
    }

    /* Status overlay */
    .status-overlay {
        position: absolute;
        inset: 0;
        z-index: 200;
        display: flex;
        align-items: stretch;
        justify-content: stretch;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(4px);
        padding: 0.5rem;
    }
    .status-panel {
        width: 100%;
        height: 100%;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 8px;
        box-shadow: 0 16px 48px rgba(0, 0, 0, 0.85);
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .status-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.85rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.12);
        background: rgba(0, 0, 0, 0.25);
        flex-shrink: 0;
    }
    .status-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1.2px;
    }
    .status-close {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .status-close:hover { color: #fff; }
    .status-body {
        padding: 0.8rem;
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
        overflow-y: auto;
        flex: 1;
    }
    .status-error {
        font-size: 0.55rem;
        color: #ff5555;
        padding: 0.3rem 0.5rem;
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.2);
        border-radius: 5px;
    }
    .status-options {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
        gap: 0.35rem;
    }
    .status-opt {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.4rem 0.55rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        color: #888;
        font-size: 0.58rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .status-opt:hover { border-color: rgba(255, 255, 255, 0.2); color: #aaa; }
    .status-opt.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.35);
        color: var(--color-primary);
    }
    .status-opt:disabled { opacity: 0.5; cursor: not-allowed; }
    .status-opt-icon { font-size: 0.7rem; }
    .status-opt-label { font-size: 0.55rem; }
    .status-section-label {
        font-size: 0.55rem;
        font-weight: 600;
        color: #888;
        letter-spacing: 0.5px;
    }
    .status-expiry-section {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .status-expiry-row {
        display: flex;
        gap: 0.3rem;
    }
    .status-expiry-btn {
        flex: 1;
        padding: 0.3rem 0.25rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        color: #888;
        font-size: 0.55rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.15s;
    }
    .status-expiry-btn:hover { border-color: rgba(255, 255, 255, 0.2); color: #aaa; }
    .status-expiry-btn.active {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.35);
        color: var(--color-primary);
    }
    .status-expiry-btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .status-hours-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        margin-top: 0.2rem;
    }
    .status-hours-label {
        font-size: 0.52rem;
        color: #888;
    }
    .cyber-input {
        width: 80px;
        padding: 0.25rem 0.4rem;
        background: rgba(0, 0, 0, 0.45);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        color: #ddd;
        font-family: var(--font-mono);
        font-size: 0.55rem;
        outline: none;
    }
    .cyber-input:focus { border-color: var(--color-primary); }
    .status-actions {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
        margin-top: 0.5rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .status-btn {
        padding: 0.35rem 0.65rem;
        border-radius: 5px;
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .status-btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .status-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .status-btn.cancel:hover:not(:disabled) { border-color: #ff5555; color: #ff5555; }
    .status-btn.clear {
        background: rgba(255, 170, 0, 0.06);
        border: 1px solid rgba(255, 170, 0, 0.2);
        color: #ffaa00;
    }
    .status-btn.clear:hover:not(:disabled) {
        background: rgba(255, 170, 0, 0.12);
    }
    .status-btn.set {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
    }
    .status-btn.set:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
    }

    /* Report overlay */
    .report-overlay {
        position: absolute;
        inset: 0;
        z-index: 200;
        display: flex;
        align-items: stretch;
        justify-content: stretch;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(4px);
        padding: 0.5rem;
    }
    .report-panel {
        width: 100%;
        height: 100%;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 8px;
        box-shadow: 0 16px 48px rgba(0, 0, 0, 0.85);
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .report-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.85rem;
        border-bottom: 1px solid rgba(255, 85, 85, 0.15);
        background: rgba(0, 0, 0, 0.25);
        flex-shrink: 0;
    }
    .report-title {
        font-size: 0.72rem;
        font-weight: 700;
        color: #ff8888;
        letter-spacing: 1.2px;
    }
    .report-close {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .report-close:hover { color: #fff; }
    .report-body {
        padding: 0.8rem;
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
        overflow-y: auto;
        flex: 1;
    }
    .report-error {
        font-size: 0.55rem;
        color: #ff5555;
        padding: 0.3rem 0.5rem;
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.2);
        border-radius: 5px;
    }
    .report-target {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.4rem 0.55rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.04);
        border-radius: 5px;
    }
    .report-target-label {
        font-size: 0.5rem;
        color: #888;
        font-weight: 600;
    }
    .report-target-val {
        font-size: 0.55rem;
        color: #ccc;
        font-family: var(--font-mono);
    }
    .report-section {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .report-section-label {
        font-size: 0.55rem;
        font-weight: 600;
        color: #888;
        letter-spacing: 0.5px;
    }
    .report-options {
        display: flex;
        flex-wrap: wrap;
        gap: 0.3rem;
    }
    .report-opt {
        padding: 0.3rem 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 5px;
        color: #888;
        font-size: 0.52rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .report-opt:hover { border-color: rgba(255, 255, 255, 0.2); color: #aaa; }
    .report-opt.active {
        background: rgba(255, 85, 85, 0.1);
        border-color: rgba(255, 85, 85, 0.35);
        color: #ff8888;
    }
    .report-opt:disabled { opacity: 0.5; cursor: not-allowed; }
    .report-actions {
        display: flex;
        gap: 0.4rem;
        justify-content: flex-end;
        margin-top: 0.5rem;
        padding-top: 0.5rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .report-btn {
        padding: 0.35rem 0.65rem;
        border-radius: 5px;
        font-size: 0.55rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .report-btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .report-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .report-btn.cancel:hover:not(:disabled) { border-color: #ff5555; color: #ff5555; }
    .report-btn.send {
        background: rgba(255, 60, 60, 0.08);
        border: 1px solid rgba(255, 60, 60, 0.25);
        color: #cc4444;
    }
    .report-btn.send:hover:not(:disabled) {
        background: rgba(255, 60, 60, 0.18);
        border-color: rgba(255, 60, 60, 0.4);
    }

    /* Broadcast preview bar */
    .preview-bar {
        position: absolute;
        left: 0.6rem;
        right: 0.6rem;
        bottom: 0.75rem;
        z-index: 45;
        max-height: min(42vh, 15rem);
        overflow-y: auto;
        padding: 0.55rem 0.65rem;
        background: rgba(6, 8, 5, 0.98);
        border: 1px solid rgba(255, 170, 0, 0.28);
        border-radius: 6px;
        box-shadow: 0 16px 34px rgba(0, 0, 0, 0.72), 0 0 18px rgba(255, 170, 0, 0.08);
        scrollbar-width: thin;
        scrollbar-color: rgba(255, 170, 0, 0.35) transparent;
    }
    .preview-bar::-webkit-scrollbar { width: 5px; }
    .preview-bar::-webkit-scrollbar-track { background: transparent; }
    .preview-bar::-webkit-scrollbar-thumb { background: rgba(255, 170, 0, 0.35); border-radius: 3px; }
    .preview-body {
        display: flex;
        flex-direction: column;
        gap: 0.24rem;
    }
    .preview-title {
        font-size: 0.62rem;
        font-weight: 700;
        color: #ffaa00;
        letter-spacing: 0.8px;
    }
    .preview-error {
        font-size: 0.62rem;
        color: #ff5555;
        padding: 0.15rem 0.3rem;
        background: rgba(255, 85, 85, 0.06);
        border-radius: 3px;
    }
    .preview-row {
        display: flex;
        align-items: flex-start;
        gap: 0.4rem;
        padding: 0.15rem 0.3rem;
        background: rgba(0, 0, 0, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 3px;
    }
    .preview-label {
        color: #555;
        font-size: 0.54rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        min-width: 3.5rem;
        flex-shrink: 0;
        padding-top: 0.05rem;
    }
    .preview-val {
        color: #ccc;
        font-size: 0.64rem;
        line-height: 1.35;
        flex: 1;
        min-width: 0;
        word-break: break-all;
    }
    .preview-val.mono { font-family: var(--font-mono); }
    .hex-preview { color: #777; font-size: 0.58rem; }
    .preview-warning {
        display: flex;
        align-items: flex-start;
        gap: 0.3rem;
        padding: 0.15rem 0.3rem;
    }
    .preview-warn-icon {
        color: #ffaa00;
        font-size: 0.6rem;
        flex-shrink: 0;
        margin-top: 0.02rem;
    }
    .preview-warn-text {
        font-size: 0.58rem;
        color: #999;
        line-height: 1.35;
    }
    .preview-note {
        color: #777;
        font-size: 0.56rem;
        line-height: 1.35;
        padding: 0.05rem 0.3rem;
    }
    .preview-actions {
        display: flex;
        gap: 0.35rem;
        justify-content: flex-end;
        margin-top: 0.15rem;
    }
    .preview-btn {
        padding: 0.2rem 0.45rem;
        border-radius: 4px;
        font-size: 0.5rem;
        font-weight: 600;
        letter-spacing: 0.3px;
        cursor: pointer;
        transition: all 0.15s;
        font-family: var(--font-mono);
    }
    .preview-btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .preview-btn.cancel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #888;
    }
    .preview-btn.cancel:hover:not(:disabled) { border-color: #ff5555; color: #ff5555; }
    .preview-btn.broadcast {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: var(--color-primary);
    }
    .preview-btn.broadcast:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
    }

    /* Pending message rows */
    .pending-msg {
        border-left-color: rgba(255, 200, 0, 0.4) !important;
        background: rgba(255, 200, 0, 0.03) !important;
        animation: pending-pulse 1.5s ease-in-out infinite;
    }
    @keyframes pending-pulse {
        0%, 100% { background: rgba(255, 200, 0, 0.03); }
        50% { background: rgba(255, 200, 0, 0.06); }
    }
    .pending-msg.failed {
        border-left-color: rgba(255, 85, 85, 0.3) !important;
        background: rgba(255, 85, 85, 0.02) !important;
        animation: none;
    }
    .pending-dot {
        display: inline-block;
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: #ffcc00;
        margin-left: 0.35rem;
        vertical-align: middle;
        animation: dot-pulse 1.2s ease-in-out infinite;
    }
    @keyframes dot-pulse {
        0%, 100% { opacity: 0.3; }
        50% { opacity: 1; }
    }
    .failed-badge {
        display: inline-flex;
        align-items: center;
        gap: 0.2rem;
        margin-left: 0.35rem;
        vertical-align: middle;
        padding: 0.05rem 0.3rem;
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.2);
        border-radius: 3px;
    }
    .failed-icon {
        font-size: 0.5rem;
        color: #ff5555;
        font-weight: 700;
    }
    .failed-label {
        font-size: 0.45rem;
        color: #ff5555;
    }
    .pending-actions {
        display: flex;
        gap: 0.2rem;
        flex-shrink: 0;
        margin-left: 0.2rem;
    }
    .pending-btn {
        width: 1.1rem;
        height: 1.1rem;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 3px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        background: rgba(0, 0, 0, 0.3);
        color: #888;
        font-size: 0.5rem;
        cursor: pointer;
        transition: all 0.15s;
    }
    .pending-btn.retry {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.2);
    }
    .pending-btn.retry:hover { background: rgba(0, 255, 65, 0.1); }
    .pending-btn.restore {
        color: #ffcc00;
        border-color: rgba(255, 204, 0, 0.24);
    }
    .pending-btn.restore:hover { background: rgba(255, 204, 0, 0.1); }
    .pending-btn.dismiss:hover { border-color: #ff5555; color: #ff5555; }
</style>
