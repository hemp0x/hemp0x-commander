<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import Tooltip from "./Tooltip.svelte";
    import "../../components.css";
    import { addNotification } from "../stores/notifications.js";

    const dispatch = createEventDispatcher();

    export let hash = "";
    export let label = "";
    export let compact = false;

    let validation = null;
    let validating = false;
    let showOpenConfirm = false;
    let gatewayUrl = "";
    let gatewayError = "";
    let copying = false;
    let lastValidationInput = "";

    $: if (hash !== lastValidationInput) {
        validateHash(hash);
    }

    async function validateHash(h) {
        lastValidationInput = h;
        gatewayUrl = "";
        gatewayError = "";
        showOpenConfirm = false;
        if (!h || !h.trim()) {
            validation = null;
            return;
        }
        validating = true;
        try {
            const result = await core.invoke("ipfs_validate", { hash: h });
            if (h === lastValidationInput) {
                validation = result;
            }
        } catch (err) {
            if (h === lastValidationInput) {
                validation = null;
            }
        } finally {
            if (h === lastValidationInput) {
                validating = false;
            }
        }
    }

    function truncatedHash() {
        if (!hash) return "...";
        if (hash.length <= 16) return hash;
        return hash.substring(0, 8) + "..." + hash.substring(hash.length - 8);
    }

    function displayHash() {
        if (compact) return truncatedHash();
        return hash;
    }

    async function copyHash() {
        if (!hash) return;
        copying = true;
        try {
            await navigator.clipboard.writeText(hash);
        } finally {
            setTimeout(() => (copying = false), 1200);
        }
    }

    async function confirmOpenGateway() {
        if (!validation) return;
        gatewayError = "";
        try {
            gatewayUrl = await core.invoke("ipfs_gateway_url", {
                hash: validation.normalized,
                gatewayBase: null,
            });
            showOpenConfirm = true;
        } catch (err) {
            gatewayError = String(err);
            addNotification({
                type: "ipfs",
                severity: "error",
                title: "IPFS Gateway Error",
                body: String(err),
            });
        }
    }

    function openGatewayUrl() {
        if (gatewayUrl) {
            window.open(gatewayUrl, "_blank", "noopener,noreferrer");
            showOpenConfirm = false;
        }
    }

    function cancelOpen() {
        showOpenConfirm = false;
        gatewayUrl = "";
    }
</script>

<div class="ipfs-ref" class:compact class:has-label={!!label}>
    {#if label}
        <span class="ipfs-label">{label}</span>
    {/if}
    <span
        class="ipfs-hash mono"
        class:invalid={validation === null && !validating && !!hash}
        title={hash}
    >
        {displayHash()}
    </span>
    <div class="ipfs-actions">
        {#if hash}
            <Tooltip text={copying ? "Copied" : "Copy hash"}>
                <button
                    class="ipfs-btn copy-btn"
                    on:click={copyHash}
                    aria-label="Copy IPFS hash"
                >
                    {#if copying}
                        ✓
                    {:else}
                        ⎘
                    {/if}
                </button>
            </Tooltip>
            <Tooltip text="Open in local IPFS gateway (127.0.0.1:8080)">
                <button
                    class="ipfs-btn open-btn"
                    on:click={confirmOpenGateway}
                    disabled={!validation}
                    aria-label="Open in gateway"
                >
                    ↗
                </button>
            </Tooltip>
        {/if}
    </div>
</div>

{#if showOpenConfirm}
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
        class="gateway-confirm-overlay"
        on:click={cancelOpen}
        role="button"
        tabindex="0"
    >
        <div class="gateway-confirm" on:click|stopPropagation>
            <div class="confirm-header">Open IPFS Gateway</div>
            <div class="confirm-body">
                <p>Opening this hash in an external gateway may expose your interest in this asset or message to the gateway operator.</p>
                <p class="confirm-url mono">{gatewayUrl}</p>
            </div>
            <div class="confirm-actions">
                <button class="action-btn primary" on:click={openGatewayUrl}>
                    OPEN
                </button>
                <button class="action-btn" on:click={cancelOpen}>
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

{#if gatewayError}
    <div class="gateway-error">Error: {gatewayError}</div>
{/if}

<style>
    .ipfs-ref {
        display: inline-flex;
        align-items: center;
        gap: 0.35rem;
        font-size: 0.55rem;
    }

    .ipfs-ref.compact .ipfs-hash {
        max-width: 140px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .ipfs-label {
        font-size: 0.45rem;
        color: #555;
        letter-spacing: 0.5px;
    }

    .ipfs-hash {
        font-family: var(--font-mono);
        color: #888;
    }

    .ipfs-hash.invalid {
        color: #ff5555;
    }

    .ipfs-actions {
        display: flex;
        gap: 0.15rem;
        align-items: center;
    }

    .ipfs-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        color: #666;
        cursor: pointer;
        font-size: 0.5rem;
        width: 18px;
        height: 18px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        transition: all 0.15s;
    }

    .ipfs-btn:hover:not(:disabled) {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.3);
    }

    .ipfs-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    .gateway-confirm-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10000;
    }

    .gateway-confirm {
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        padding: 1rem;
        width: 90%;
        max-width: 380px;
    }

    .confirm-header {
        font-size: 0.7rem;
        font-weight: 600;
        color: var(--color-primary);
        letter-spacing: 1px;
        margin-bottom: 0.5rem;
    }

    .confirm-body {
        font-size: 0.55rem;
        color: #aaa;
        margin-bottom: 0.75rem;
    }

    .confirm-body p {
        margin: 0.25rem 0;
    }

    .confirm-url {
        color: #888;
        font-size: 0.5rem;
        word-break: break-all;
        margin-top: 0.4rem;
    }

    .confirm-actions {
        display: flex;
        gap: 0.5rem;
        justify-content: flex-end;
    }

    .action-btn {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 10px;
        padding: 0.4rem 0.8rem;
        color: #aaa;
        font-size: 0.6rem;
        font-weight: 600;
        letter-spacing: 1px;
        cursor: pointer;
        transition: all 0.15s;
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

    .gateway-error {
        font-size: 0.5rem;
        color: #ff5555;
        margin-top: 0.25rem;
    }
</style>
