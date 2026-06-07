<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import "../../components.css";
    import ModalAlert from "./ModalAlert.svelte";
    import { cidViewerTarget } from "../stores/contentLibrary.js";
    import AssetDetailDetailsTab from "../ui/asset-detail/AssetDetailDetailsTab.svelte";
    import AssetMessageCompose from "../ui/asset-messages/AssetMessageCompose.svelte";
    import AssetMessageInbox from "../ui/asset-messages/AssetMessageInbox.svelte";
    import H0xCChatPanel from "../ui/h0xc/H0xCChatPanel.svelte";

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
    let h0xcOpen = false;
    let currentAssetName = "";

    const dispatch = createEventDispatcher();

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

    function openMessagesTab() {
        activeTab = "MESSAGES";
        loadMessages();
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

    function openCompose() {
        composeOpen = true;
    }

    function cancelCompose() {
        composeOpen = false;
    }

    function openH0xC() {
        h0xcOpen = true;
    }

    function closeH0xC() {
        h0xcOpen = false;
    }

    function handleCreateH0xC() {
        closeH0xC();
        dispatch("createH0xC", asset);
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

    async function toggleSubscription() {
        if (!asset) return;
        const names = channelNamesForAsset(asset.name);
        const channelName = names.find((name) => channels.includes(name))
            ?? names.find((name) => name.endsWith('!'))
            ?? names[0]
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

    $: if (activeTab) {
        dispatch("tabChange", activeTab);
    }

    $: assetChannelNames = asset ? channelNamesForAsset(asset.name) : [];

    $: assetComposeChannelName = assetChannelNames.find((name) => name.endsWith("!"))
        ?? assetChannelNames[0]
        ?? "";

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
                    <AssetMessageCompose
                        channelName={assetComposeChannelName}
                        on:close={cancelCompose}
                        on:sent={loadMessages}
                    />
                {:else if h0xcOpen}
                    <H0xCChatPanel
                        inline
                        show={true}
                        on:close={closeH0xC}
                        on:createH0xC={handleCreateH0xC}
                    />
                {:else}
                    <AssetMessageInbox
                        {asset}
                        {messages}
                        {messagesInfo}
                        {messagesLoading}
                        {messagesError}
                        {isSubscribed}
                        on:compose={openCompose}
                        on:refresh={loadMessages}
                        on:subscriptionToggle={toggleSubscription}
                        on:openH0xc={openH0xC}
                    />
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

    <ModalAlert
        isOpen={showAlert}
        title="Coming Soon"
        message="Top 100 Holders list requires 'assetindex=1' node configuration. This feature is deferred."
        on:close={() => (showAlert = false)}
    />
{/if}

<style>
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
