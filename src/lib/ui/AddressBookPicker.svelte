<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { fly } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { addNotification } from "../stores/notifications.js";

    export let value = "";
    export let label = "Address";
    export let id = "ab-picker";
    export let placeholder = "H...";

    const dispatch = createEventDispatcher();

    let showDropdown = false;
    let pickerEl;
    let searchQuery = "";
    let loading = false;
    let error = "";
    let favorites = [];

    // Inline edit state
    let editingAddress = null;
    let editLabelText = "";
    let showAddForm = false;
    let newAddrInput = "";
    let newLabelInput = "";

    function shortAddr(addr) {
        if (!addr || addr.length < 16) return addr || "...";
        return addr.slice(0, 10) + "..." + addr.slice(-6);
    }

    function displayLabel(lbl) {
        return lbl && lbl.trim() ? lbl : "—";
    }

    $: filtered = filterFavorites(favorites, searchQuery);

    function filterFavorites(list, query) {
        let res = [...list];
        if (query.trim()) {
            const q = query.toLowerCase();
            res = res.filter(
                (f) =>
                    (f.label && f.label.toLowerCase().includes(q)) ||
                    (f.address && f.address.toLowerCase().includes(q)),
            );
        }
        return res;
    }

    function selectEntry(entry) {
        value = entry.address;
        showDropdown = false;
        searchQuery = "";
        editingAddress = null;
        dispatch("select", entry);
    }

    function selectCustom() {
        showDropdown = false;
        searchQuery = "";
        editingAddress = null;
    }

    function toggleDropdown(e) {
        e.stopPropagation();
        showDropdown = !showDropdown;
        if (showDropdown) {
            searchQuery = "";
            editingAddress = null;
            loadFavorites();
        }
    }

    function closeDropdown() {
        showDropdown = false;
        searchQuery = "";
        editingAddress = null;
        showAddForm = false;
    }

    function handleKeydown(e) {
        if (e.key === "Escape") closeDropdown();
    }

    async function loadFavorites() {
        loading = true;
        error = "";
        try {
            favorites = await core.invoke("load_address_book");
        } catch (err) {
            error = String(err);
            favorites = [];
        } finally {
            loading = false;
        }
    }

    async function saveFavorites() {
        try {
            await core.invoke("save_address_book", { entries: favorites });
        } catch (err) {
            console.error("Failed to save address book:", err);
        }
    }

    async function quickSave() {
        if (!value.trim()) {
            addNotification({ type: "system", severity: "error", title: "No Address", body: "Enter an address to save." });
            return;
        }
        const addr = value.trim();
        if (favorites.some((f) => f.address === addr)) {
            addNotification({ type: "system", severity: "info", title: "Already Saved", body: "This address is already in your address book." });
            return;
        }
        favorites.push({ label: "Unlabeled", address: addr, locked: false, date: Date.now() });
        favorites = favorites;
        await saveFavorites();
        addNotification({ type: "system", severity: "success", title: "Address Saved", body: "Address added to your address book." });
    }

    function startEdit(entry) {
        editingAddress = entry.address;
        editLabelText = entry.label || "";
    }

    function saveEdit(entry) {
        if (editLabelText.trim()) {
            const idx = favorites.findIndex((f) => f.address === entry.address);
            if (idx >= 0) {
                favorites[idx].label = editLabelText.trim();
                favorites = favorites;
                saveFavorites();
            }
        }
        editingAddress = null;
    }

    function deleteEntry(entry) {
        if (entry.locked) {
            addNotification({ type: "system", severity: "error", title: "Locked", body: "Unlock this address before deleting it." });
            return;
        }
        favorites = favorites.filter((f) => f.address !== entry.address);
        saveFavorites();
    }

    function toggleLock(entry) {
        const idx = favorites.findIndex((f) => f.address === entry.address);
        if (idx >= 0) {
            favorites[idx].locked = !favorites[idx].locked;
            favorites = favorites;
            saveFavorites();
        }
    }

    function openAddForm() {
        showAddForm = true;
        newAddrInput = "";
        newLabelInput = "";
    }

    function cancelAddForm() {
        showAddForm = false;
        newAddrInput = "";
        newLabelInput = "";
    }

    async function submitNewAddress() {
        if (!newAddrInput.trim()) return;
        const addr = newAddrInput.trim();
        if (favorites.some((f) => f.address === addr)) {
            cancelAddForm();
            return;
        }
        try {
            const res = await core.invoke("run_cli_args", { args: ["validateaddress", addr] });
            const json = JSON.parse(res);
            if (!json.isvalid) {
                addNotification({ type: "system", severity: "error", title: "Invalid Address", body: "The entered address is not valid." });
                return;
            }
        } catch {
            addNotification({ type: "system", severity: "error", title: "Validation Failed", body: "Could not validate address." });
            return;
        }
        favorites.push({ label: newLabelInput.trim() || "Unlabeled", address: addr, locked: false, date: Date.now() });
        favorites = favorites;
        await saveFavorites();
        cancelAddForm();
    }

    onMount(() => {
        loadFavorites();
    });
</script>

<svelte:window on:click={showDropdown ? closeDropdown : undefined} on:keydown={handleKeydown} />

<div class="ab-picker" bind:this={pickerEl}>
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
        <button
            class="picker-trigger star-btn"
            type="button"
            on:click={(e) => { e.stopPropagation(); quickSave(); }}
            title="Save address to book"
            class:active={favorites.some((f) => f.address === value)}
        >
            ★
        </button>
        <button
            class="picker-trigger book-btn"
            type="button"
            on:click={toggleDropdown}
            title="Open address book"
        >
            📒
        </button>
    </div>

    {#if showDropdown}
        <div class="picker-dropdown" transition:fly={{ y: -6, duration: 150 }}>
            <!-- Search -->
            <div class="dropdown-search">
                <input
                    type="text"
                    class="search-input mono"
                    bind:value={searchQuery}
                    placeholder="Search address book..."
                />
            </div>

            <div class="dropdown-divider"></div>

            <!-- Custom option -->
            <button
                class="dropdown-row custom"
                type="button"
                on:click={selectCustom}
                class:selected={!favorites.some((f) => f.address === value)}
            >
                <span class="row-label">Custom address</span>
                <span class="row-addr mono">{shortAddr(value) || "Type your own"}</span>
            </button>

            <div class="dropdown-divider"></div>

            <!-- Header -->
            <div class="dropdown-header">
                <span class="header-col">LABEL</span>
                <span class="header-col">ADDRESS</span>
                <span class="header-col right"></span>
            </div>

            <!-- Rows -->
            {#if error}
                <div class="dropdown-empty error">{error}</div>
            {:else if loading}
                <div class="dropdown-empty">Loading...</div>
            {:else if filtered.length === 0}
                <div class="dropdown-empty">
                    {searchQuery.trim() ? "No matches." : "Address book empty."}
                </div>
            {:else}
                {#each filtered as entry (entry.address)}
                    <div class="dropdown-row-wrap" class:selected={entry.address === value}>
                        <button
                            class="dropdown-row-main"
                            type="button"
                            on:click={() => selectEntry(entry)}
                        >
                            {#if editingAddress === entry.address}
                                <input
                                    type="text"
                                    class="edit-input"
                                    bind:value={editLabelText}
                                    on:keydown={(e) => e.key === "Enter" && saveEdit(entry)}
                                    on:blur={() => saveEdit(entry)}
                                />
                            {:else}
                                <span class="row-label" title={entry.label}>{displayLabel(entry.label)}</span>
                            {/if}
                            <span class="row-addr mono" title={entry.address}>{shortAddr(entry.address)}</span>
                        </button>
                        <div class="row-actions">
                            <button
                                class="row-action lock"
                                type="button"
                                title={entry.locked ? "Unlock" : "Lock"}
                                on:click={(e) => { e.stopPropagation(); toggleLock(entry); }}
                            >
                                {entry.locked ? "🔒" : "🔓"}
                            </button>
                            <button
                                class="row-action edit"
                                type="button"
                                title="Edit label"
                                on:click={(e) => { e.stopPropagation(); startEdit(entry); }}
                            >
                                ✎
                            </button>
                            <button
                                class="row-action delete"
                                type="button"
                                title="Delete"
                                on:click={(e) => { e.stopPropagation(); deleteEntry(entry); }}
                            >
                                🗑
                            </button>
                        </div>
                    </div>
                {/each}
            {/if}

            <div class="dropdown-divider"></div>

            <!-- Add new address -->
            {#if showAddForm}
                <div class="add-form">
                    <input
                        type="text"
                        class="add-input mono"
                        bind:value={newAddrInput}
                        placeholder="Address..."
                        on:keydown={(e) => e.key === "Enter" && submitNewAddress()}
                    />
                    <input
                        type="text"
                        class="add-input"
                        bind:value={newLabelInput}
                        placeholder="Label (optional)"
                        on:keydown={(e) => e.key === "Enter" && submitNewAddress()}
                    />
                    <div class="add-actions">
                        <button class="add-btn primary" type="button" on:click={(e) => { e.stopPropagation(); submitNewAddress(); }}>SAVE</button>
                        <button class="add-btn" type="button" on:click={(e) => { e.stopPropagation(); cancelAddForm(); }}>CANCEL</button>
                    </div>
                </div>
            {:else}
                <button
                    class="add-toggle"
                    type="button"
                    on:click={(e) => { e.stopPropagation(); openAddForm(); }}
                >
                    + Add New Address
                </button>
            {/if}
        </div>
    {/if}
</div>

<style>
    .ab-picker {
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
    .picker-trigger:hover {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
    }
    .picker-trigger.star-btn {
        font-size: 0.85rem;
        color: #888;
        border-color: rgba(255, 255, 255, 0.1);
        background: rgba(255, 255, 255, 0.05);
    }
    .picker-trigger.star-btn:hover {
        color: #ffbd2e;
        border-color: #ffbd2e;
        background: rgba(255, 189, 46, 0.1);
    }
    .picker-trigger.star-btn.active {
        color: #ffbd2e;
        border-color: #ffbd2e;
        text-shadow: 0 0 5px #ffbd2e;
    }
    .picker-trigger.book-btn {
        font-size: 0.75rem;
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
        box-shadow: 0 0 30px rgba(0, 0, 0, 0.7);
        max-height: 260px;
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
        grid-template-columns: 1fr 1fr 60px;
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
        grid-template-columns: 1fr 1fr 60px;
        gap: 0.4rem;
        align-items: center;
        padding: 0.4rem 0.5rem;
        background: transparent;
        border: 1px solid transparent;
        border-radius: 6px;
        color: #aaa;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        transition: all 0.12s;
        width: 100%;
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
        color: #888;
        font-style: italic;
    }
    .dropdown-row.custom.selected {
        background: rgba(255, 170, 0, 0.08);
        border-color: rgba(255, 170, 0, 0.25);
    }

    .dropdown-row-wrap {
        display: flex;
        align-items: center;
        gap: 0.2rem;
        padding: 0.2rem 0.3rem;
        border-radius: 6px;
        border: 1px solid transparent;
        transition: all 0.12s;
    }
    .dropdown-row-wrap:hover {
        background: rgba(0, 255, 65, 0.04);
        border-color: rgba(0, 255, 65, 0.1);
    }
    .dropdown-row-wrap.selected {
        background: rgba(0, 255, 65, 0.08);
        border-color: rgba(0, 255, 65, 0.2);
    }

    .dropdown-row-main {
        flex: 1;
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.4rem;
        align-items: center;
        padding: 0.2rem 0.3rem;
        background: transparent;
        border: none;
        color: #aaa;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        min-width: 0;
    }
    .dropdown-row-main:hover .row-label {
        color: var(--color-primary);
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

    .row-actions {
        display: flex;
        gap: 0.1rem;
        align-items: center;
    }
    .row-action {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 0.75rem;
        padding: 0.15rem;
        border-radius: 4px;
        opacity: 0.5;
        transition: all 0.15s;
        line-height: 1;
    }
    .row-action:hover {
        opacity: 1;
        background: rgba(255, 255, 255, 0.05);
    }
    .row-action.lock:hover {
        color: var(--color-primary);
    }
    .row-action.edit:hover {
        color: var(--color-primary);
    }
    .row-action.delete:hover {
        color: #ff4444;
    }

    .edit-input {
        width: 100%;
        padding: 0.2rem 0.3rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid var(--color-primary);
        border-radius: 4px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.7rem;
        outline: none;
    }

    .add-toggle {
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
    .add-toggle:hover {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
    }

    .add-form {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
        padding: 0.3rem;
    }
    .add-input {
        width: 100%;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.75rem;
        outline: none;
        box-sizing: border-box;
    }
    .add-input:focus {
        border-color: var(--color-primary);
    }
    .add-actions {
        display: flex;
        gap: 0.35rem;
        justify-content: flex-end;
    }
    .add-btn {
        padding: 0.3rem 0.6rem;
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
    .add-btn:hover {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .add-btn.primary {
        background: rgba(0, 255, 65, 0.1);
        border-color: rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .add-btn.primary:hover {
        background: var(--color-primary);
        color: #000;
    }

    .dropdown-empty {
        padding: 0.6rem 0.5rem;
        text-align: center;
        color: #555;
        font-size: 0.7rem;
    }
    .dropdown-empty.error {
        color: #ff5555;
    }

    @media (max-width: 600px) {
        .dropdown-header,
        .dropdown-row-main {
            grid-template-columns: 1fr 1fr;
        }
        .row-actions {
            display: none;
        }
        .picker-dropdown {
            max-height: 240px;
        }
    }
</style>
