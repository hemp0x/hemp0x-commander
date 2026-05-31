<script>
    import { fly } from "svelte/transition";
    import { formatBalance } from "../utils.js";

    export let value = "";
    export let label = "Asset";
    export let id = "asset-picker";
    export let placeholder = "ASSET_NAME";
    export let assets = []; // Array of {name, balance, type?}
    export let disabled = false;

    let showDropdown = false;
    let pickerSearch = "";
    let pickerEl;

    $: filteredAssets = assets.filter(a => {
        if (!pickerSearch.trim()) return true;
        const q = pickerSearch.toLowerCase();
        return a.name.toLowerCase().includes(q);
    });

    function assetType(asset) {
        if (asset.name.endsWith("!")) return "Owner";
        if (asset.name.includes("#")) return "NFT";
        if (asset.name.includes("/")) return "Sub";
        return "Token";
    }

    function selectAsset(asset) {
        value = asset.name;
        showDropdown = false;
        pickerSearch = "";
    }

    function selectCustom() {
        showDropdown = false;
        pickerSearch = "";
    }

    function toggleDropdown(e) {
        e.stopPropagation();
        showDropdown = !showDropdown;
        if (showDropdown) pickerSearch = "";
    }

    function closeDropdown() {
        showDropdown = false;
        pickerSearch = "";
    }

    function handleKeydown(e) {
        if (e.key === "Escape") closeDropdown();
    }
</script>

<svelte:window on:click={showDropdown ? closeDropdown : undefined} on:keydown={handleKeydown} />

<div class="asset-picker" bind:this={pickerEl}>
    <label for={id}>{label}</label>

    <div class="picker-input-row">
        <input
            {id}
            type="text"
            class="cyber-input mono"
            bind:value
            {placeholder}
            {disabled}
            on:focus={() => (showDropdown = false)}
        />
        {#if assets.length > 0}
            <button
                class="picker-trigger"
                type="button"
                on:click={toggleDropdown}
                title="Select owned asset"
                {disabled}
            >
                <span class="picker-arrow" class:open={showDropdown}>▼</span>
            </button>
        {/if}
    </div>

    {#if showDropdown && assets.length > 0}
        <div class="picker-dropdown" transition:fly={{ y: -6, duration: 150 }}>
            <!-- Search inside dropdown -->
            <div class="dropdown-search">
                <input
                    type="text"
                    class="search-input mono"
                    bind:value={pickerSearch}
                    placeholder="Search assets..."
                />
            </div>

            <div class="dropdown-divider"></div>

            <!-- Custom option -->
            <button
                class="dropdown-row custom"
                type="button"
                on:click={selectCustom}
            >
                <span class="row-name">Custom asset</span>
                <span class="row-balance"></span>
                <span class="row-type"></span>
            </button>

            <div class="dropdown-divider"></div>

            <!-- Header -->
            <div class="dropdown-header">
                <span class="header-col">NAME</span>
                <span class="header-col right">BALANCE</span>
                <span class="header-col right">TYPE</span>
            </div>

            <!-- Asset rows -->
            {#if filteredAssets.length === 0}
                <div class="dropdown-empty">No assets match.</div>
            {:else}
                {#each filteredAssets as asset (asset.name)}
                    <button
                        class="dropdown-row"
                        type="button"
                        class:selected={asset.name === value}
                        on:click={() => selectAsset(asset)}
                    >
                        <span class="row-name" title={asset.name}>{asset.name}</span>
                        <span class="row-balance">{formatBalance(asset.balance)}</span>
                        <span class="row-type">{assetType(asset)}</span>
                    </button>
                {/each}
            {/if}
        </div>
    {/if}
</div>

<style>
    .asset-picker {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }

    label {
        color: #888;
        font-size: 0.65rem;
        letter-spacing: 0.5px;
        display: block;
        margin-bottom: 0.15rem;
    }

    .picker-input-row {
        display: flex;
        align-items: center;
        gap: 0.35rem;
    }

    .cyber-input {
        flex: 1;
        padding: 0.45rem 0.6rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        outline: none;
        transition: all 0.2s;
        box-sizing: border-box;
        min-width: 0;
    }
    .cyber-input:focus {
        border-color: var(--color-primary);
    }
    .cyber-input::placeholder {
        color: #555;
    }

    .picker-trigger {
        flex-shrink: 0;
        width: 32px;
        height: 32px;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.55rem;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all 0.15s;
        box-sizing: border-box;
    }
    .picker-trigger:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
    }
    .picker-trigger:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .picker-arrow {
        transition: transform 0.2s;
        display: inline-block;
    }
    .picker-arrow.open {
        transform: rotate(180deg);
    }

    .picker-dropdown {
        position: absolute;
        top: calc(100% + 4px);
        left: 0;
        right: 0;
        z-index: 500;
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        box-shadow: 0 0 30px rgba(0, 0, 0, 0.7);
        max-height: 240px;
        overflow-y: auto;
        padding: 0.3rem;
    }
    .picker-dropdown::-webkit-scrollbar {
        width: 6px;
    }
    .picker-dropdown::-webkit-scrollbar-track {
        background: transparent;
    }
    .picker-dropdown::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.1);
        border-radius: 3px;
    }
    .picker-dropdown::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.3);
    }

    .dropdown-search {
        padding: 0.25rem 0.3rem;
    }
    .search-input {
        width: 100%;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.7rem;
        outline: none;
        box-sizing: border-box;
    }
    .search-input:focus {
        border-color: var(--color-primary);
    }
    .search-input::placeholder {
        color: #555;
    }

    .dropdown-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.15rem 0;
    }

    .dropdown-header {
        display: grid;
        grid-template-columns: 1fr 100px 60px;
        gap: 0.4rem;
        padding: 0.25rem 0.5rem;
        font-size: 0.55rem;
        color: #555;
        letter-spacing: 0.5px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .header-col {
        font-weight: 600;
    }
    .header-col.right {
        text-align: right;
    }

    .dropdown-row {
        display: grid;
        grid-template-columns: 1fr 100px 60px;
        gap: 0.4rem;
        align-items: center;
        padding: 0.4rem 0.5rem;
        background: none;
        border: none;
        border-radius: 4px;
        color: #ccc;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        transition: all 0.1s;
        width: 100%;
    }
    .dropdown-row:hover {
        background: rgba(0, 255, 65, 0.06);
    }
    .dropdown-row.selected {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
    }
    .dropdown-row.custom {
        color: #888;
        font-style: italic;
    }
    .dropdown-row.custom .row-name {
        color: #aaa;
    }

    .row-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: #ddd;
    }
    .row-balance {
        text-align: right;
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.65rem;
    }
    .row-type {
        text-align: right;
        font-size: 0.6rem;
        color: #666;
    }

    .dropdown-empty {
        padding: 0.6rem 0.5rem;
        text-align: center;
        color: #555;
        font-size: 0.7rem;
    }
</style>
