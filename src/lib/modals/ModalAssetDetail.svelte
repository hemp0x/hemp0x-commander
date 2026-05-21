<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { formatBalance } from "../utils.js";
    import "../../components.css";
    import Tooltip from "../ui/Tooltip.svelte";
    import ModalAlert from "./ModalAlert.svelte";

    const dispatch = createEventDispatcher();

    export let asset;
    export let metadata;
    export let loading = false;
    export let slideDirection = 0;
    export let hasMultipleAssets = false;

    let showAlert = false;

    let activeTab = "DETAILS";

    let messagesInfo = null;
    let messages = [];
    let channels = [];
    let messagesLoading = false;
    let messagesError = "";
    let isSubscribed = false;

    let composeOpen = false;
    let composeIpfsHash = "";
    let composeExpireTime = "";
    let composePreview = null;
    let composeError = "";
    let composePreviewing = false;
    let composeBroadcasting = false;
    let composeSent = false;
    let currentAssetName = "";

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
            const [info, msgs, chans] = await Promise.all([
                core.invoke("get_messaging_info"),
                core.invoke("view_asset_messages"),
                core.invoke("view_message_channels"),
            ]);
            messagesInfo = info;
            messages = msgs;
            channels = chans;
        } catch (err) {
            messagesError = String(err);
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

    async function toggleSubscription() {
        if (!asset) return;
        const channelName = asset.name;
        try {
            if (isSubscribed) {
                await core.invoke("unsubscribe_from_channel", { channelName });
            } else {
                await core.invoke("subscribe_to_channel", { channelName });
            }
            await loadMessages();
        } catch (err) {
            messagesError = String(err);
        }
    }

    async function previewAnnouncement() {
        if (!composeIpfsHash.trim()) {
            composeError = "IPFS hash is required";
            return;
        }
        const expireTime = parseComposeExpireTime();
        if (expireTime === undefined) return;
        composePreviewing = true;
        composeError = "";
        composePreview = null;
        try {
            const channelName = asset.name;
            composePreview = await core.invoke("preview_send_announcement", {
                channelName,
                ipfsHash: composeIpfsHash.trim(),
                expireTime,
            });
        } catch (err) {
            composeError = String(err);
        } finally {
            composePreviewing = false;
        }
    }

    async function broadcastAnnouncement() {
        if (!composePreview) return;
        const expireTime = composePreview.expire_time ?? null;
        composeBroadcasting = true;
        composeError = "";
        try {
            const channelName = asset.name;
            const txid = await core.invoke("send_announcement", {
                channelName,
                ipfsHash: composeIpfsHash.trim(),
                expireTime,
            });
            composeSent = true;
            composePreview = null;
            composeIpfsHash = "";
            composeExpireTime = "";
        } catch (err) {
            composeError = String(err);
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

{#if asset}
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
        class="modal-overlay"
        transition:fade={{ duration: 150 }}
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        role="button"
        tabindex="0"
    >
        <div class="modal-container">
            <!-- Navigation Arrows -->
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
                    out:fly={{
                        x: slideDirection * -40,
                        duration: 120,
                        opacity: 0,
                    }}
                    on:click|stopPropagation
                >
                    <button class="modal-close" on:click={close}>×</button>

                    <div class="detail-header">
                        <div class="detail-icon">◈</div>
                        <div class="detail-title">
                            {asset.name}
                        </div>
                    </div>

                    <div class="detail-body">
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
                                    {asset.isSubAsset
                                        ? "SUB-ASSET"
                                        : asset.type || "STANDARD"}
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
                                        : "Status"}
                                    on:click={onGovernance}
                                    on:keydown={(e) =>
                                        e.key === "Enter" && onGovernance()}
                                >
                                    {asset.hasOwner ? "👑 OWNER" : "🔒 LOCKED"}
                                </div>
                            </div>
                            <div class="detail-stat">
                                <div class="stat-label">DECIMALS</div>
                                <div class="stat-value">
                                    {metadata?.units ?? asset.units ?? 0}
                                </div>
                            </div>
                        </div>

                        <!-- Enhanced Metadata Section -->
                        {#if loading}
                            <div class="metadata-section">
                                <div class="meta-loading">
                                    Loading metadata...
                                </div>
                            </div>
                        {:else if metadata}
                            <div class="metadata-section">
                                <div class="meta-row">
                                    <span class="meta-label">TOTAL SUPPLY</span>
                                    <Tooltip
                                        text="Top 100 Holders (Coming Soon)"
                                    >
                                        <span
                                            class="meta-value clickable"
                                            role="button"
                                            tabindex="0"
                                            on:click={() => (showAlert = true)}
                                            on:keydown={(e) =>
                                                e.key === "Enter" &&
                                                (showAlert = true)}
                                            >{metadata.amount.toLocaleString()}</span
                                        >
                                    </Tooltip>
                                </div>
                                <div class="meta-row">
                                    <span class="meta-label">REISSUABLE</span>
                                    <span
                                        class="meta-value"
                                        class:yes={metadata.reissuable}
                                    >
                                        {metadata.reissuable ? "YES" : "NO"}
                                    </span>
                                </div>
                                {#if metadata.has_ipfs && metadata.ipfs_hash}
                                    <div class="meta-row">
                                        <span class="meta-label">IPFS</span>
                                        <span class="meta-value mono ipfs-hash"
                                            >{metadata.ipfs_hash}</span
                                        >
                                    </div>
                                {/if}
                                <div class="meta-row">
                                    <span class="meta-label"
                                        >CREATED AT BLOCK</span
                                    >
                                    <span class="meta-value"
                                        >{metadata.block_height.toLocaleString()}</span
                                    >
                                </div>
                            </div>
                        {/if}

                        <div class="detail-actions">
                            <button
                                class="action-btn primary"
                                on:click={onTransfer}
                            >
                                <span class="action-icon">→</span> TRANSFER
                            </button>
                            <button
                                class="action-btn"
                                class:disabled={!metadata?.reissuable}
                                on:click={onReissue}
                                disabled={!metadata?.reissuable}
                                title={!metadata?.reissuable
                                    ? "Asset supply is locked"
                                    : "Reissue Asset"}
                            >
                                <span class="action-icon">↻</span> REISSUE
                            </button>
                        </div>
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
                                    Messaging is disabled on this node. Enable it by removing -disablemessaging or waiting for BIP9 activation.
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
                                        <div class="message-entry" class:unread={msg.status === 'UNREAD'}>
                                            <div class="message-channel">{msg.asset_name}</div>
                                            <div class="message-hash" title={msg.message}>
                                                IPFS: {msg.message}
                                            </div>
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

                        {#if composeOpen}
                            <div class="compose-overlay" transition:fade={{ duration: 150 }}>
                                <div class="compose-panel">
                                    <div class="compose-header">
                                        <span>Send Announcement on {asset.name}</span>
                                        <button class="modal-close-sub" on:click={cancelCompose}>×</button>
                                    </div>

                                    <div class="compose-body">
                                        <div class="compose-field">
                                            <label for="compose-ipfs">IPFS Hash</label>
                                            <input
                                                id="compose-ipfs"
                                                type="text"
                                                bind:value={composeIpfsHash}
                                                placeholder="Qm..."
                                                disabled={composeBroadcasting || composeSent || composePreview}
                                            />
                                        </div>
                                        <div class="compose-field">
                                            <label for="compose-expire">Expire Time (UTC timestamp, optional)</label>
                                            <input
                                                id="compose-expire"
                                                type="text"
                                                bind:value={composeExpireTime}
                                                placeholder="e.g. 1737500000"
                                                disabled={composeBroadcasting || composeSent || composePreview}
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
                                                    <span>IPFS:</span> {composePreview.ipfs_hash}
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
                                    </div>

                                    <div class="compose-actions">
                                        {#if !composePreview && !composeSent}
                                            <button
                                                class="action-btn primary"
                                                on:click={previewAnnouncement}
                                                disabled={composePreviewing || !composeIpfsHash.trim()}
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
                        {/if}
                        {/if}
                    </div>
                </div>
            {/key}

            <!-- Right Arrow -->
            {#if hasMultipleAssets}
                <button
                    class="nav-arrow nav-next"
                    on:click|stopPropagation={next}
                    title="Next Asset">»</button
                >
            {/if}
        </div>
    </div>

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
        border-radius: 16px;
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
    .detail-header {
        padding: 1rem 2rem 0.8rem;
        text-align: center;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.75rem;
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
    .detail-body {
        padding: 1rem 1.5rem 1.5rem;
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
    }
    .owner-actions {
        margin-top: 0.8rem;
    }
    .action-btn {
        flex: 1;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 10px;
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
        margin-top: 0.5rem;
        padding: 0.5rem 0.75rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 8px;
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
    .meta-value.mono {
        font-family: var(--font-mono);
    }
    .ipfs-hash {
        font-size: 0.55rem;
        max-width: 120px;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    /* Tabs */
    .detail-tabs {
        display: flex;
        gap: 0;
        margin-bottom: 1rem;
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

    /* Compose Overlay */
    .compose-overlay {
        position: absolute;
        inset: 0;
        background: rgba(0, 0, 0, 0.9);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10;
        border-radius: 16px;
    }
    .compose-panel {
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 12px;
        padding: 1rem;
        width: 90%;
        max-width: 400px;
    }
    .compose-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        font-size: 0.7rem;
        font-weight: 600;
        color: var(--color-primary);
        letter-spacing: 1px;
        margin-bottom: 0.75rem;
    }
    .modal-close-sub {
        background: transparent;
        border: none;
        color: #555;
        font-size: 1.2rem;
        cursor: pointer;
    }
    .modal-close-sub:hover {
        color: #fff;
    }
    .compose-body {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .compose-field {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .compose-field label {
        font-size: 0.55rem;
        color: #666;
        letter-spacing: 0.5px;
    }
    .compose-field input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.4rem 0.5rem;
        color: #fff;
        font-size: 0.65rem;
        font-family: var(--font-mono);
    }
    .compose-field input:focus {
        outline: none;
        border-color: var(--color-primary);
    }
    .compose-error {
        font-size: 0.55rem;
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
        margin-top: 0.75rem;
    }
    .compose-actions .action-btn {
        flex: none;
        padding: 0.4rem 0.8rem;
    }
</style>
