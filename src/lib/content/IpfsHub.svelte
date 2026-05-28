<script>
    import { fade } from "svelte/transition";
    import { ipfsHubSection } from "../stores/contentLibrary.js";
    import ContentLibraryPanel from "./ContentLibraryPanel.svelte";
    import CidImportPanel from "./CidImportPanel.svelte";
    import ProviderSettingsPanel from "./ProviderSettingsPanel.svelte";

    function setSection(section) {
        $ipfsHubSection = section;
    }
</script>

<div class="ipfs-hub" in:fade={{ duration: 200 }}>
    <header class="hub-header">
        <div class="hub-tabs">
            <button
                class="hub-tab"
                class:active={$ipfsHubSection === "library"}
                on:click={() => setSection("library")}
            >
                LIBRARY
            </button>
            <button
                class="hub-tab"
                class:active={$ipfsHubSection === "cid-viewer"}
                on:click={() => setSection("cid-viewer")}
            >
                CID VIEWER
            </button>
            <button
                class="hub-tab"
                class:active={$ipfsHubSection === "providers"}
                on:click={() => setSection("providers")}
            >
                PROVIDERS
            </button>
        </div>
    </header>

    <div class="hub-body">
        {#key $ipfsHubSection}
            <div in:fade={{ duration: 150 }}>
                {#if $ipfsHubSection === "library"}
                    <ContentLibraryPanel />
                {:else if $ipfsHubSection === "cid-viewer"}
                    <CidImportPanel on:imported={() => setSection("library")} />
                {:else if $ipfsHubSection === "providers"}
                    <ProviderSettingsPanel />
                {/if}
            </div>
        {/key}
    </div>
</div>

<style>
    .ipfs-hub {
        flex: 1;
        display: flex;
        flex-direction: column;
        min-height: 0;
    }
    .hub-header {
        flex-shrink: 0;
        margin-bottom: 0.5rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.08);
    }
    .hub-tabs {
        display: flex;
        gap: 1px;
    }
    .hub-tab {
        background: transparent;
        border: none;
        color: #555;
        padding: 0.5rem 1.2rem;
        font-size: 0.7rem;
        letter-spacing: 1.5px;
        border-bottom: 2px solid transparent;
        cursor: pointer;
        transition: all 0.2s;
        font-weight: 600;
    }
    .hub-tab:hover {
        color: #aaa;
        background: rgba(255, 255, 255, 0.02);
    }
    .hub-tab.active {
        color: var(--color-primary);
        border-bottom-color: var(--color-primary);
        text-shadow: 0 0 8px rgba(0, 255, 65, 0.4);
    }
    .hub-body {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }
</style>
