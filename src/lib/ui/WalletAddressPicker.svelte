<script>
    import { createEventDispatcher } from "svelte";
    import { fly } from "svelte/transition";
    import { formatBalance } from "../utils.js";

    export let value = "";
    export let label = "Address";
    export let id = "address-picker";
    export let placeholder = "H...";
    export let addresses = []; // Array of {address, label, balance}
    export let nodeOnline = false;
    export let defaultSortColumn = "label";
    export let defaultSortDirection = "asc";

    const dispatch = createEventDispatcher();

    let showDropdown = false;
    let pickerEl;
    let sortColumn = defaultSortColumn; // 'label', 'balance'
    let sortDirection = defaultSortDirection; // 'asc', 'desc'
    let genLabel = "";
    let showGenForm = false;

    function shortAddr(addr) {
        if (!addr || addr.length < 16) return addr || "...";
        return addr.slice(0, 10) + "..." + addr.slice(-6);
    }

    function displayLabel(lbl) {
        return lbl && lbl.trim() ? lbl : "-";
    }

    function selectAddress(addr) {
        value = addr.address;
        showDropdown = false;
        dispatch("select", addr);
    }

    function selectCustom() {
        showDropdown = false;
    }

    function toggleDropdown(e) {
        e.stopPropagation();
        showDropdown = !showDropdown;
    }

    function closeDropdown() {
        showDropdown = false;
        showGenForm = false;
        genLabel = "";
    }

    function handleKeydown(e) {
        if (e.key === "Escape") closeDropdown();
    }

    function toggleSort(col) {
        if (sortColumn === col) {
            sortDirection = sortDirection === "asc" ? "desc" : "asc";
        } else {
            sortColumn = col;
            sortDirection = col === "balance" ? "desc" : "asc";
        }
    }

    function doGenerate() {
        dispatch("generate", { label: genLabel.trim() || undefined });
        showGenForm = false;
        genLabel = "";
    }

    $: sortedAddresses = [...addresses].sort((a, b) => {
        let valA = a[sortColumn];
        let valB = b[sortColumn];
        if (sortColumn === "balance") {
            valA = Number(valA || 0);
            valB = Number(valB || 0);
        } else {
            valA = displayLabel(valA).toLowerCase();
            valB = displayLabel(valB).toLowerCase();
        }
        if (valA < valB) return sortDirection === "asc" ? -1 : 1;
        if (valA > valB) return sortDirection === "asc" ? 1 : -1;
        return 0;
    });

    $: selectedItem = addresses.find(a => a.address === value);
    $: selectedLabel = selectedItem?.label || "";
    $: selectedBalance = selectedItem?.balance || "";
</script>

<svelte:window on:click={showDropdown ? closeDropdown : undefined} on:keydown={handleKeydown} />

<div class="wallet-picker" bind:this={pickerEl}>
    <label for={id}>{label}</label>

    <div class="picker-input-row">
        <input
            {id}
            type="text"
            class="cyber-input mono"
            bind:value
            {placeholder}
            on:focus={() => (showDropdown = false)}
        />
        {#if addresses.length > 0}
            <button
                class="picker-trigger"
                type="button"
                on:click={toggleDropdown}
                title="Select wallet address"
            >
                <span class="picker-arrow" class:open={showDropdown}>▼</span>
            </button>
        {/if}
    </div>

    {#if showDropdown && addresses.length > 0}
        <div class="picker-dropdown" transition:fly={{ y: -6, duration: 150 }}>
            <!-- Custom option -->
            <button
                class="dropdown-row custom"
                type="button"
                on:click={selectCustom}
                class:selected={!addresses.some(a => a.address === value)}
            >
                <span class="row-label">Custom address</span>
                <span class="row-addr mono">{shortAddr(value) || "Type your own"}</span>
                <span class="row-balance"></span>
            </button>

            <div class="dropdown-divider"></div>

            <!-- Sortable header -->
            <div class="dropdown-header">
                <button
                    class="sort-btn"
                    class:active={sortColumn === "label"}
                    on:click={(e) => { e.stopPropagation(); toggleSort("label"); }}
                    type="button"
                >
                    LABEL {sortColumn === "label" ? (sortDirection === "asc" ? "▲" : "▼") : ""}
                </button>
                <span class="header-spacer">ADDRESS</span>
                <button
                    class="sort-btn"
                    class:active={sortColumn === "balance"}
                    class:right={true}
                    on:click={(e) => { e.stopPropagation(); toggleSort("balance"); }}
                    type="button"
                >
                    {sortColumn === "balance" ? (sortDirection === "asc" ? "▲" : "▼") : ""} BALANCE
                </button>
            </div>

            <!-- Address rows -->
            {#each sortedAddresses as addr}
                <button
                    class="dropdown-row"
                    type="button"
                    class:selected={addr.address === value}
                    on:click={() => selectAddress(addr)}
                >
                    <span class="row-label">{displayLabel(addr.label)}</span>
                    <span class="row-addr mono">{shortAddr(addr.address)}</span>
                    <span class="row-balance">{formatBalance(addr.balance)}</span>
                </button>
            {/each}

            <div class="dropdown-divider"></div>

            <!-- Generate new address -->
            {#if showGenForm}
                <div class="gen-form">
                    <input
                        type="text"
                        class="gen-input mono"
                        bind:value={genLabel}
                        placeholder="Label (optional)"
                        on:keydown={(e) => e.key === "Enter" && doGenerate()}
                    />
                    <div class="gen-actions">
                        <button class="gen-btn primary" type="button" on:click={(e) => { e.stopPropagation(); doGenerate(); }} disabled={!nodeOnline}>
                            Generate
                        </button>
                        <button class="gen-btn" type="button" on:click={(e) => { e.stopPropagation(); showGenForm = false; genLabel = ""; }}>
                            Cancel
                        </button>
                    </div>
                </div>
            {:else}
                <button
                    class="gen-toggle"
                    type="button"
                    on:click={(e) => { e.stopPropagation(); showGenForm = true; }}
                    disabled={!nodeOnline}
                >
                    + Generate New Address
                </button>
            {/if}
        </div>
    {/if}
</div>

<style>
    .wallet-picker {
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
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        outline: none;
        transition: all 0.2s;
        box-sizing: border-box;
        min-width: 0;
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.03);
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
        border-bottom: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.55rem;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: all 0.15s;
        box-sizing: border-box;
        box-shadow: inset 0 0 0 1px rgba(0, 255, 65, 0.08);
    }
    .picker-trigger:hover {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
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
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.7);
        max-height: 220px;
        overflow-y: auto;
        padding: 0.4rem 0.55rem 0.55rem;
        scrollbar-gutter: stable;
    }
    .picker-dropdown::-webkit-scrollbar {
        width: 8px;
    }
    .picker-dropdown::-webkit-scrollbar-track {
        background: transparent;
    }
    .picker-dropdown::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.12);
        border-radius: 4px;
    }
    .picker-dropdown::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.3);
    }

    .dropdown-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.25rem 0;
    }

    .dropdown-header {
        display: grid;
        grid-template-columns: 90px 1fr 80px;
        align-items: center;
        gap: 0.5rem;
        padding: 0.25rem 0.5rem;
        font-size: 0.55rem;
        color: #555;
        letter-spacing: 0.5px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
        margin-bottom: 0.15rem;
    }
    .sort-btn {
        background: transparent;
        border: none;
        color: #555;
        font-size: 0.55rem;
        letter-spacing: 0.5px;
        cursor: pointer;
        padding: 0;
        text-align: left;
        transition: color 0.15s;
    }
    .sort-btn.right {
        text-align: right;
    }
    .sort-btn:hover {
        color: #888;
    }
    .sort-btn.active {
        color: var(--color-primary);
    }
    .header-spacer {
        text-align: center;
    }

    .dropdown-row {
        width: 100%;
        display: grid;
        grid-template-columns: 90px 1fr 80px;
        align-items: center;
        gap: 0.4rem;
        padding: 0.35rem 0.45rem;
        background: transparent;
        border: 1px solid transparent;
        border-radius: 6px;
        color: #aaa;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        transition: all 0.12s;
    }
    .dropdown-row:hover {
        background: rgba(0, 255, 65, 0.05);
        border-color: rgba(0, 255, 65, 0.15);
    }
    .dropdown-row.selected {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.25);
        color: #fff;
    }
    .dropdown-row.custom {
        grid-template-columns: 100px 1fr 80px;
    }
    .dropdown-row.custom.selected {
        background: rgba(255, 170, 0, 0.08);
        border-color: rgba(255, 170, 0, 0.25);
    }

    .row-label {
        font-weight: 600;
        color: #ccc;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .row-addr {
        font-size: 0.65rem;
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .row-balance {
        font-family: var(--font-mono);
        font-size: 0.65rem;
        color: var(--color-primary);
        text-align: right;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .gen-toggle {
        width: 100%;
        padding: 0.4rem 0.5rem;
        background: rgba(0, 255, 65, 0.05);
        border: 1px dashed rgba(0, 255, 65, 0.2);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.65rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        text-align: center;
        transition: all 0.15s;
    }
    .gen-toggle:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
    }
    .gen-toggle:disabled {
        opacity: 0.4;
        cursor: not-allowed;
        color: #555;
        border-color: #333;
    }

    .gen-form {
        display: flex;
        flex-direction: column;
        gap: 0.35rem;
        padding: 0.35rem 0.2rem;
    }
    .gen-input {
        width: 100%;
        padding: 0.4rem 0.5rem;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.75rem;
        outline: none;
        box-sizing: border-box;
    }
    .gen-input:focus {
        border-color: var(--color-primary);
    }
    .gen-actions {
        display: flex;
        gap: 0.35rem;
        justify-content: flex-end;
    }
    .gen-btn {
        padding: 0.35rem 0.7rem;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #aaa;
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 0.5px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .gen-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .gen-btn.primary {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .gen-btn.primary:hover {
        background: var(--color-primary);
        color: #000;
    }
    .gen-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    @media (max-width: 600px) {
        .dropdown-row,
        .dropdown-header {
            grid-template-columns: 70px 1fr 70px;
            gap: 0.3rem;
            padding: 0.3rem 0.35rem;
        }
        .picker-dropdown {
            max-height: 200px;
        }
    }
</style>
