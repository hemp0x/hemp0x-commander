<script>
    import { fade } from "svelte/transition";
</script>

<div class="provider-settings" in:fade={{ duration: 150 }}>
    <h3 class="panel-title">PROVIDER SETTINGS</h3>

    <div class="notice-bar">
        Configured publish providers are not enabled yet. This section will allow you to select and configure IPFS providers for publishing content packages.
    </div>

    <div class="provider-grid">
        <div class="provider-card disabled">
            <div class="provider-header">
                <span class="provider-name">Manual CID Import</span>
                <span class="provider-status available">AVAILABLE</span>
            </div>
            <div class="provider-desc">
                Import content by pasting a CID/hash. Creates a local package record for reference and library organization.
            </div>
            <div class="provider-detail">
                Use the CID Viewer section to import CIDs manually.
            </div>
        </div>

        <div class="provider-card disabled">
            <div class="provider-header">
                <span class="provider-name">Installed Kubo (go-ipfs)</span>
                <span class="provider-status planned">PLANNED</span>
            </div>
            <div class="provider-desc">
                Connect to a locally running Kubo/ipfs daemon for full IPFS node functionality including publishing and pinning.
            </div>
            <div class="provider-detail">
                Requires Kubo installed and running. Will be available after v2.1+ prototype evaluation.
            </div>
        </div>

        <div class="provider-card disabled">
            <div class="provider-header">
                <span class="provider-name">Pinata / web3.storage</span>
                <span class="provider-status planned">PLANNED</span>
            </div>
            <div class="provider-desc">
                Upload and pin content through third-party pinning services using API keys.
            </div>
            <div class="provider-detail">
                API-key management will be available after security review of credential storage.
            </div>
        </div>
    </div>

    <div class="privacy-section">
        <h4 class="section-subtitle">PRIVACY NOTES</h4>
        <ul class="privacy-list">
            <li>
                <strong>Public gateway requests</strong> reveal requested CIDs to gateway operators. Use with care.
            </li>
            <li>
                <strong>Local IPFS nodes</strong> may expose your IP address and DHT participation metadata to the IPFS network.
            </li>
            <li>
                <strong>Third-party pinning services</strong> have access to the content you upload and your usage patterns.
            </li>
            <li>
                You are responsible for the content you publish and retrieve. Commander does not censor or filter.
            </li>
        </ul>
    </div>
</div>

<style>
    .provider-settings {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 8px;
        padding: 1rem 1.2rem;
        max-width: 700px;
    }
    .panel-title {
        font-size: 0.8rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        margin: 0 0 0.8rem 0;
    }
    .notice-bar {
        padding: 0.5rem 0.75rem;
        background: rgba(255, 165, 0, 0.08);
        border: 1px solid rgba(255, 165, 0, 0.2);
        color: #cca;
        font-size: 0.65rem;
        border-radius: 4px;
        margin-bottom: 1rem;
        line-height: 1.4;
    }
    .provider-grid {
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
        margin-bottom: 1.2rem;
    }
    .provider-card {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 6px;
        padding: 0.75rem 1rem;
    }
    .provider-card.disabled {
        opacity: 0.75;
    }
    .provider-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.35rem;
    }
    .provider-name {
        font-size: 0.75rem;
        font-weight: 600;
        color: #ccc;
    }
    .provider-status {
        font-size: 0.55rem;
        padding: 2px 8px;
        border-radius: 4px;
        letter-spacing: 1px;
        font-weight: 600;
    }
    .provider-status.available {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
    }
    .provider-status.planned {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #666;
    }
    .provider-desc {
        font-size: 0.65rem;
        color: #888;
        line-height: 1.4;
        margin-bottom: 0.25rem;
    }
    .provider-detail {
        font-size: 0.6rem;
        color: #555;
    }
    .privacy-section {
        padding: 0.8rem 0;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .section-subtitle {
        color: #666;
        font-size: 0.65rem;
        letter-spacing: 1px;
        margin: 0 0 0.6rem 0;
        text-transform: uppercase;
    }
    .privacy-list {
        margin: 0;
        padding-left: 1.2rem;
        font-size: 0.6rem;
        color: #777;
        line-height: 1.6;
    }
    .privacy-list li {
        margin: 0.3rem 0;
    }
    .privacy-list li strong {
        color: #aaa;
    }
</style>
