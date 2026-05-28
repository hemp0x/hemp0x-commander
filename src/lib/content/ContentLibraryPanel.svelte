<script>
    import { createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { contentLibrary, libraryLoading, activePanel, sortByUpdatedDesc } from "../stores/contentLibrary.js";
    import PackageCard from "./PackageCard.svelte";
    import PackageComposer from "./PackageComposer.svelte";

    const dispatch = createEventDispatcher();
    let errorMsg = "";

    $: packages = sortByUpdatedDesc($contentLibrary);

    onMount(async () => {
        await refresh();
    });

    async function refresh() {
        $libraryLoading = true;
        errorMsg = "";
        try {
            $contentLibrary = await core.invoke("content_library_list");
        } catch (err) {
            errorMsg = String(err);
        }
        $libraryLoading = false;
    }

    function showCreate() {
        $activePanel = "create";
    }

    function showBrowse() {
        $activePanel = "browse";
    }

    function showDetail(id) {
        $activePanel = "detail";
    }

    function onPackageSaved() {
        showBrowse();
        refresh();
    }
</script>

<div class="content-library">
    <header class="library-header">
        <div class="header-left">
            <span class="header-title">CONTENT LIBRARY</span>
            <span class="header-count mono">{$contentLibrary.length} packages</span>
        </div>
        <div class="header-actions">
            {#if $activePanel !== "browse"}
                <button class="header-btn" on:click={showBrowse}>
                    BACK TO LIBRARY
                </button>
            {/if}
            {#if $activePanel === "browse"}
                <button class="header-btn create-btn" on:click={showCreate}>
                    + NEW PACKAGE
                </button>
            {/if}
            <button class="header-btn refresh-btn" on:click={refresh} disabled={$libraryLoading}>
                {$libraryLoading ? "LOADING..." : "REFRESH"}
            </button>
        </div>
    </header>

    <div class="library-body">
        {#if errorMsg}
            <div class="error-bar">{errorMsg}</div>
        {/if}

        {#if $activePanel === "browse"}
            {#if $libraryLoading}
                <div class="empty-state">Loading packages...</div>
            {:else if packages.length === 0}
                <div class="empty-state">
                    <div class="empty-icon">◈</div>
                    <div class="empty-text">No packages in library</div>
                    <button class="cyber-btn" on:click={showCreate}>CREATE YOUR FIRST PACKAGE</button>
                </div>
            {:else}
                <div class="package-grid">
                    {#each packages as pkg (pkg.id)}
                        <div in:fly={{ y: 10, duration: 200 }}>
                            <PackageCard {pkg} on:refresh={refresh} on:edit={() => { $activePanel = pkg.id; }} />
                        </div>
                    {/each}
                </div>
            {/if}
        {:else if $activePanel === "create"}
            <PackageComposer on:saved={onPackageSaved} on:cancel={showBrowse} />
        {:else}
            {#each packages as pkg (pkg.id)}
                {#if $activePanel === pkg.id}
                    <PackageComposer editPackage={pkg} on:saved={onPackageSaved} on:cancel={showBrowse} />
                {/if}
            {/each}
        {/if}
    </div>
</div>

<style>
    .content-library {
        flex: 1;
        display: flex;
        flex-direction: column;
        min-height: 0;
    }
    .library-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0 0 0.75rem 0;
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        flex-shrink: 0;
    }
    .header-left {
        display: flex;
        align-items: baseline;
        gap: 0.75rem;
    }
    .header-title {
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 2px;
    }
    .header-count {
        font-size: 0.7rem;
        color: #555;
    }
    .header-actions {
        display: flex;
        gap: 0.5rem;
    }
    .header-btn {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        padding: 0.4rem 0.8rem;
        font-size: 0.65rem;
        font-weight: 600;
        letter-spacing: 1px;
        border-radius: 6px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .header-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.2);
    }
    .header-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .library-body {
        flex: 1;
        overflow-y: auto;
        padding: 1rem 0;
    }
    .package-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
        gap: 1rem;
    }
    .empty-state {
        text-align: center;
        padding: 3rem 2rem;
        color: #555;
    }
    .empty-icon {
        font-size: 2.5rem;
        margin-bottom: 0.75rem;
        opacity: 0.3;
    }
    .empty-text {
        font-size: 0.9rem;
        margin-bottom: 1.5rem;
    }
    .error-bar {
        padding: 0.5rem 1rem;
        background: rgba(255, 68, 68, 0.1);
        border: 1px solid rgba(255, 68, 68, 0.3);
        color: #ff6666;
        font-size: 0.75rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        font-family: var(--font-mono);
    }
    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.6rem 1.2rem;
        letter-spacing: 1px;
        font-weight: bold;
        font-size: 0.75rem;
        cursor: pointer;
        text-transform: uppercase;
        transition: all 0.2s;
    }
    .cyber-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
    }
</style>
