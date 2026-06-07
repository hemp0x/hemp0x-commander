<script>
    import { formatBalance } from "../../utils.js";
    import "../../../components.css";
    import Tooltip from "../Tooltip.svelte";
    import IpfsReference from "../IpfsReference.svelte";

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
     */

    /** @type {AssetDetail | null} */
    export let asset = null;
    /** @type {AssetMetadata | null} */
    export let metadata = null;
    export let loading = false;

    /** @type {() => void} */
    export let onGovernance = () => {};
    /** @type {() => void} */
    export let onTransfer = () => {};
    /** @type {() => void} */
    export let onReissue = () => {};
    /** @type {() => void} */
    export let onManageTags = () => {};
    /** @type {() => void} */
    export let onSubAsset = () => {};
    /** @type {() => void} */
    export let onNft = () => {};
    /** @type {() => void} */
    export let openCidViewer = () => {};
    /** @type {() => void} */
    export let onShowAlert = () => {};
</script>

<div class="detail-grid">
    <div class="detail-stat">
        <div class="stat-label">YOUR BALANCE</div>
        <div class="stat-value neon-text">
            {formatBalance(asset?.balance ?? 0)}
        </div>
    </div>
    <div class="detail-stat">
        <div class="stat-label">TYPE</div>
        <div class="stat-value">
            {asset?.name.includes("#")
                    ? "NFT"
                    : asset?.isSubAsset
                        ? "SUB-ASSET"
                        : asset?.type || "TOKEN"}
        </div>
    </div>
    <div class="detail-stat">
        <div class="stat-label">STATUS</div>
        <div
            class="stat-value"
            class:owner-yes={asset?.hasOwner}
            class:clickable={asset?.hasOwner}
            role="button"
            tabindex={asset?.hasOwner ? 0 : -1}
            title={asset?.hasOwner
                ? "Manage Governance"
                : "Holder — no owner token"}
            on:click={onGovernance}
            on:keydown={(e) =>
                e.key === "Enter" && onGovernance()}
        >
            {asset?.hasOwner ? "👑 OWNER" : "HOLDER"}
        </div>
    </div>
    <div class="detail-stat">
        <div class="stat-label">DECIMALS</div>
        <div class="stat-value">
            {metadata?.units ?? asset?.units ?? 0}
        </div>
    </div>
</div>

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
                    on:click={onShowAlert}
                    on:keydown={(e) =>
                        e.key === "Enter" && onShowAlert()}
                    >{metadata.amount?.toLocaleString() ?? "--"}</span>
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
                <span class="meta-value">{metadata.block_height?.toLocaleString() ?? "--"}</span>
            </div>
        </div>

        {#if metadata.has_ipfs && metadata.ipfs_hash}
            <div class="meta-card ipfs-card">
                <div class="ipfs-header">
                    <span class="meta-label">METADATA CID / HASH</span>
                    <div class="ipfs-header-actions">
                        {#if asset?.hasOwner && metadata?.reissuable}
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
                    {#if asset?.hasOwner && metadata?.reissuable}
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
    {#if asset?.hasOwner}
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
{#if asset?.hasOwner && asset?.name.startsWith("#")}
    <div class="detail-actions owner-actions">
        <button
            class="action-btn"
            on:click={onManageTags}
        >
            <span class="action-icon">🏷</span> MANAGE TAGS
        </button>
    </div>
{/if}
{#if asset?.hasOwner && !asset?.name.includes("#")}
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

<style>
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
    .stat-value.owner-yes { color: gold; }

    .detail-actions { display: flex; gap: 0.8rem; margin: 0 2rem; }
    .owner-actions { margin-top: 0.8rem; }

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
    .action-btn.disabled { opacity: 0.5; cursor: not-allowed; filter: grayscale(1); }
    .action-btn.disabled:hover { background: rgba(255, 255, 255, 0.05); color: #fff; transform: none; box-shadow: none; }
    .action-icon { font-size: 1rem; }

    .metadata-section { margin-top: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem; }
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
    .meta-row { display: flex; flex-direction: column; align-items: center; padding: 0.25rem 0; }
    .meta-label { font-size: 0.45rem; color: #555; letter-spacing: 0.5px; margin-bottom: 0.1rem; }
    .meta-value { font-size: 0.7rem; color: #aaa; }
    .meta-value.yes { color: var(--color-primary); }
    .meta-value.clickable {
        cursor: pointer;
        transition: all 0.2s;
        border: 1px solid transparent;
        border-radius: 4px;
        padding: 0 4px;
    }
    .meta-value.clickable:hover {
        background: rgba(255, 215, 0, 0.15);
        border-color: rgba(255, 215, 0, 0.3);
        transform: scale(1.05);
    }

    .ipfs-card { grid-template-columns: 1fr; gap: 0.4rem; }
    .ipfs-card.empty { display: flex; flex-direction: column; gap: 0.4rem; }
    .ipfs-header { display: flex; justify-content: space-between; align-items: center; }
    .ipfs-header-actions { display: flex; gap: 0.4rem; align-items: center; }
    .ipfs-value { font-size: 0.65rem; }
    .ipfs-value :global(.ipfs-ref) { font-size: 0.65rem; }
    .ipfs-placeholder { font-size: 0.65rem; color: #555; font-style: italic; }

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
    .meta-update-btn:hover { background: var(--color-primary); color: #000; }
    .meta-update-btn .action-icon { font-size: 0.7rem; }
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
    .meta-view-btn:hover { background: rgba(0, 255, 65, 0.12); border-color: var(--color-primary); }
    .meta-view-btn .action-icon { font-size: 0.7rem; }
</style>
