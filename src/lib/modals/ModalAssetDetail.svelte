<script>
    import { fly, fade } from "svelte/transition";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
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

    export let asset;
    export let metadata;
    export let loading = false;
    export let slideDirection = 0;
    export let hasMultipleAssets = false;
    export let inline = false;
    export let initialActiveTab = "DETAILS";

    let showAlert = false;

    let activeTab = initialActiveTab;

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

    // Short message compose state
    let composeMode = "cid"; // "cid" | "short"
    let composeShortText = "";
    let composeShortResult = null;
    let composeShortEncoding = false;
    let composeShortShowHex = false;
    let composeShortDebounce = null;
    let composeShortTextarea = null;
    let autocompleteEnabled = true;
    let composeShortFocused = false;

    function handleShortSuggestion(suggestion) {
        const nextText = insertSuggestion(composeShortText, suggestion);
        composeShortText = nextText;
        queueEncodeShortMessage();
        return nextText;
    }

    function handleTextareaFocusOut(event) {
        const related = event.relatedTarget;
        const wrap = event.currentTarget;
        if (!wrap.contains(related)) {
            composeShortFocused = false;
        }
    }

    // Short message decode cache for received messages
    let shortMessageCache = {};
    let shortMessagePending = new Set();

    $: if (activeTab) {
        dispatch("tabChange", activeTab);
    }

    $: if (filteredMessages.length > 0) {
        decodePendingShortMessages(filteredMessages);
    }

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
            addNotification({
                type: "message",
                severity: "success",
                title: "Announcement Sent",
                body: `Message sent on channel ${asset.name}`,
                action: { label: "Copy TXID", txid },
            });
        } catch (err) {
            composeError = String(err);
            addNotification({
                type: "message",
                severity: "error",
                title: "Announcement Failed",
                body: String(err),
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
										on:click={() => { composeMode = "cid"; composeShortText = ""; composeShortResult = null; composeIpfsHash = ""; composeError = ""; }}
										disabled={composeBroadcasting || composeSent || !!composePreview}
									>CID / Hash</button>
									<button
										class="mode-btn"
										class:active={composeMode === "short"}
										on:click={() => { composeMode = "short"; composeIpfsHash = ""; composeError = ""; }}
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
												disabled={composeBroadcasting || composeSent || !!composePreview}
											></textarea>
											{#if autocompleteEnabled && !composeBroadcasting && !composeSent && !composePreview}
												<ShortMessageAutocomplete
													text={composeShortText}
													disabled={composeBroadcasting || composeSent || !!composePreview}
													targetElement={composeShortTextarea}
													onAccept={handleShortSuggestion}
													focused={composeShortFocused}
												/>
											{/if}
										</div>
										<label class="autocomplete-toggle">
											<input type="checkbox" bind:checked={autocompleteEnabled} />
											<span>Enable text prediction</span>
										</label>
										{#if composeShortEncoding}
											<div class="short-msg-status">Encoding...</div>
										{:else if composeShortResult}
											<div class="short-msg-status" class:fits={composeShortResult.fits} class:no-fits={!composeShortResult.fits}>
												{#if composeShortResult.fits}
													✓ Fits ({composeShortResult.encoded_payload_len} / 29 bytes used)
												{:else}
													✗ Does not fit ({composeShortResult.encoded_payload_len} / 29 bytes)
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
