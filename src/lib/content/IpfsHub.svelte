<script>
    import { fade } from "svelte/transition";
    import { ipfsHubSection } from "../stores/contentLibrary.js";
    import ContentLibraryPanel from "./ContentLibraryPanel.svelte";
    import CidViewer from "./CidViewer.svelte";
    import CidImportPanel from "./CidImportPanel.svelte";
    import ProviderSettingsPanel from "./ProviderSettingsPanel.svelte";
    import HelpHitbox from "../ui/HelpHitbox.svelte";

    function setSection(section) {
        $ipfsHubSection = section;
    }

    export let openCid = null;
</script>

<div class="ipfs-hub" in:fade={{ duration: 200 }}>
    <header class="hub-header">
        <div class="hub-tabs">
            <nav class="hub-tab-list" aria-label="IPFS sections">
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
                    SETTINGS
                </button>
            </nav>
            <div class="hub-help-inline">
                <HelpHitbox title="IPFS In Commander">
                    <p>Library packages are local records in Commander. A CID points to content on IPFS and may be fetched through a public gateway.</p>
                    <p>Public gateways can reveal request metadata. Pinning keeps content available on your selected provider; unpinning removes that provider pin but cannot erase data already shared elsewhere.</p>
                </HelpHitbox>
            </div>
        </div>
    </header>

    <div class="hub-body">
        {#key $ipfsHubSection}
            <div in:fade={{ duration: 150 }}>
                {#if $ipfsHubSection === "library"}
                    <ContentLibraryPanel />
                {:else if $ipfsHubSection === "cid-viewer"}
                    {#if openCid}
                        <CidViewer loadCid={openCid} on:created={() => setSection("library")} />
                    {:else}
                        <CidViewer on:created={() => setSection("library")} />
                    {/if}
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
        min-height: 0;
        display: grid;
        grid-template-rows: auto 1fr;
    }
    .hub-header {
        padding: 0.25rem 0;
        margin-bottom: 0.5rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.08);
    }
    .hub-tabs {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        padding: 0.22rem;
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 6px;
        background: rgba(0, 0, 0, 0.34);
    }
    .hub-tab-list {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        flex: 1 1 auto;
        min-width: 0;
        gap: 0.25rem;
    }
    .hub-tab {
        min-width: 0;
        padding: 0.38rem 0.65rem;
        overflow: hidden;
        border: 1px solid transparent;
        border-radius: 4px;
        background: transparent;
        color: rgba(255, 255, 255, 0.48);
        font-size: 0.68rem;
        letter-spacing: 0.75px;
        text-overflow: ellipsis;
        white-space: nowrap;
        cursor: pointer;
        transition: all 0.2s;
        font-weight: 600;
    }
    .hub-tab:hover {
        border-color: rgba(0, 255, 65, 0.16);
        background: rgba(0, 255, 65, 0.025);
        color: rgba(255, 255, 255, 0.78);
        box-shadow: none;
        transform: none;
    }
    .hub-tab.active {
        border-color: rgba(0, 255, 65, 0.32);
        background: rgba(0, 255, 65, 0.07);
        color: var(--color-primary);
    }
    .hub-help-inline {
        flex: 0 0 auto;
        display: inline-flex;
        align-items: center;
        padding-right: 0.15rem;
    }
    @media (max-width: 560px) {
        .hub-tab-list {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }
    }
    .hub-body {
        min-height: 0;
        overflow-y: auto;
        -webkit-overflow-scrolling: touch;
    }
</style>
