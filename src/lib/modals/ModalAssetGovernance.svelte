<script>
    import { createEventDispatcher } from "svelte";
    import { fade, scale } from "svelte/transition";
    import { invoke } from "@tauri-apps/api/core";
    import "../../components.css";
    import ModalAlert from "./ModalAlert.svelte";

    export let isOpen = false;
    export let asset; // { name, balance, hasOwner, reissuable, ipfs_hash, ... }

    const dispatch = createEventDispatcher();

    let activeTab = "transfer"; // transfer, lock, metadata
    let newOwnerAddress = "";
    let newIpfsHash = "";

    // Safety & Loading State
    let lockConfirmed = false;
    let isSubmitting = false;
    let lockHoldSeconds = 10;
    let isHolding = false;
    let holdTimer;

    // Journal State
    let previewJournalId = null;

    // Alert State
    let alertOpen = false;
    let alertTitle = "";
    let alertMessage = "";
    let alertType = "info";
    let shouldCloseOnAlertDismiss = false;

    // Indexing State
    let assetIndexEnabled = false; // We will check this on mount

    async function checkAssetIndex() {
        // TODO: Implement backend command to check hemp.conf
        // For now, assume false to show the button
        // const info = await invoke("get_info");
        // assetIndexEnabled = info.assetindex;
    }

    async function enableAssetIndex() {
        // TODO: invoke("enable_asset_index")
        triggerAlert(
            "Coming Soon",
            "We need to add the backend logic to modify hemp.conf and restart the node. But the UI is ready!",
            "info",
        );
    }

    $: if (activeTab === "dividends") {
        checkAssetIndex();
    }

    function startHold() {
        if (!lockConfirmed || isSubmitting) return;
        isHolding = true;
        lockHoldSeconds = 10;

        holdTimer = setInterval(() => {
            lockHoldSeconds--;
            if (lockHoldSeconds <= 0) {
                stopHold();
                if (activeTab === "lock") {
                    handleLock();
                } else if (activeTab === "transfer") {
                    handleTransferLogic();
                }
            }
        }, 1000);
    }

    function stopHold() {
        if (holdTimer) clearInterval(holdTimer);
        isHolding = false;
        lockHoldSeconds = 10;
    }

    $: if (isOpen && asset) {
        newIpfsHash = asset?.ipfs_hash || "";
        lockConfirmed = false;
        isSubmitting = false;
        stopHold();
    }

    $: if (activeTab) {
        lockConfirmed = false;
        stopHold();
    }

    function close() {
        dispatch("close");
    }

    function triggerAlert(title, message, type = "info", closeParent = false) {
        alertTitle = title;
        alertMessage = message;
        alertType = type;
        shouldCloseOnAlertDismiss = closeParent;
        alertOpen = true;
    }

    function handleAlertClose() {
        alertOpen = false;
        if (shouldCloseOnAlertDismiss) {
            close();
        }
    }

    // Called by timer when 10s is up
    async function handleTransferLogic() {
        isSubmitting = true;
        const ownerToken = (asset?.name || "") + "!";
        const dest = newOwnerAddress;
        try {
            const details = {
                operation_type: "owner_transfer",
                asset_name: asset?.name || "",
                owner_token: ownerToken,
                destination: dest,
                irreversible: true,
            };
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "owner_transfer",
                        summary: `Transfer ownership of ${asset?.name} to ${dest.substring(0, 16)}${dest.length > 16 ? "..." : ""}`,
                        txid: null,
                        details,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            const txid = await invoke("transfer_asset", {
                asset: ownerToken,
                amount: "1.0",
                to: dest,
            });
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid,
                        details: null,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }
            previewJournalId = null;
            triggerAlert(
                "Ownership Transferred",
                "You have successfully transferred ownership. You no longer control this asset.",
                "success",
                true, // Close modal after OK
            );
        } catch (e) {
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(e) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
                previewJournalId = null;
            }
            triggerAlert("Error", e.toString(), "error");
        } finally {
            isSubmitting = false;
        }
    }

    // Dummy handler if needed for button, but button uses mousedown=startHold
    function handleTransfer() {
        // Validation check usually done in template disabled state
    }

    async function handleLock() {
        if (!lockConfirmed) return;
        isSubmitting = true;
        try {
            const units = asset?.units ?? 8;
            const assetName = asset?.name || "";
            const details = {
                operation_type: "lock_supply",
                asset_name: assetName,
                current_units: units,
                irreversible: true,
            };
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "lock_supply",
                        summary: `Permanently lock supply of ${assetName}`,
                        txid: null,
                        details,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            const txid = await invoke("lock_asset_supply", {
                name: assetName,
                currentUnits: units,
            });
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid,
                        details: null,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }
            previewJournalId = null;
            triggerAlert(
                "Supply Locked",
                "The asset supply has been permanently locked.",
                "success",
                true,
            );
        } catch (e) {
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(e) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
                previewJournalId = null;
            }
            triggerAlert("Error", e.toString(), "error");
        } finally {
            isSubmitting = false;
        }
    }

    async function handleMetadata() {
        isSubmitting = true;
        try {
            const units = asset?.units ?? 8;
            const assetName = asset?.name || "";
            const ipfs = newIpfsHash.trim();
            const details = {
                operation_type: "metadata_update",
                asset_name: assetName,
                ipfs_hash: ipfs,
                current_units: units,
            };
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "metadata_update",
                        summary: `Update metadata for ${assetName}${ipfs ? " with IPFS " + ipfs.substring(0, 16) + "..." : ""}`,
                        txid: null,
                        details,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
            const txid = await invoke("update_asset_metadata", {
                name: assetName,
                ipfsHash: ipfs,
                currentUnits: units,
            });
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid,
                        details: null,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }
            previewJournalId = null;
            triggerAlert(
                "Metadata Updated",
                "Asset metadata has been updated on the blockchain.",
                "success",
                true,
            );
        } catch (e) {
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(e) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
                previewJournalId = null;
            }
            triggerAlert("Error", e.toString(), "error");
        } finally {
            isSubmitting = false;
        }
    }
</script>

{#if isOpen}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        on:click={close}
        on:keydown={(e) => e.key === "Escape" && close()}
        transition:fade={{ duration: 200 }}
    >
        <div
            class="modal glass-panel"
            role="dialog"
            aria-modal="true"
            tabindex="-1"
            on:click|stopPropagation
            on:keydown={() => {}}
            transition:scale={{ duration: 200, start: 0.95 }}
        >
            <div class="modal-header">
                <h3>{asset ? asset.name : ""}</h3>
                <button class="close-btn" on:click={close}>&times;</button>
            </div>

            <!-- Tabs -->
            <div class="tabs">
                <button
                    class:active={activeTab === "transfer"}
                    on:click={() => (activeTab = "transfer")}
                    >Transfer Owner</button
                >
                <button
                    class:active={activeTab === "dividends"}
                    on:click={() => (activeTab = "dividends")}>Dividends</button
                >
                <button
                    class:active={activeTab === "metadata"}
                    on:click={() => (activeTab = "metadata")}>Metadata</button
                >
                <button
                    class:active={activeTab === "lock"}
                    on:click={() => (activeTab = "lock")}
                    class="tab-danger">Lock Supply</button
                >
            </div>

            <div class="modal-body">
                <!-- Inline alerts removed in favor of ModalAlert -->

                {#if activeTab === "transfer"}
                    <div class="panel danger-zone">
                        <h4>Transfer Ownership</h4>
                        <p class="section-desc">
                            Send the <strong
                                >Administrator Token ({asset
                                    ? asset.name
                                    : ""}!)</strong
                            > to another wallet.
                        </p>
                        <p class="warning-text">
                            You will lose all control of this asset forever.
                        </p>
                        <label for="owner-address">New Owner Address</label>
                        <input
                            id="owner-address"
                            type="text"
                            bind:value={newOwnerAddress}
                            placeholder="H..."
                            class="cyber-input"
                        />

                        <!-- Confirmation Check for Transfer -->
                        <label class="confirm-check">
                            <input
                                type="checkbox"
                                bind:checked={lockConfirmed}
                            />
                            <span>I confirm I want to transfer ownership.</span>
                        </label>

                        <div class="actions">
                            <button
                                class="cyber-btn danger"
                                on:mousedown={startHold}
                                on:touchstart={startHold}
                                on:mouseup={stopHold}
                                on:touchend={stopHold}
                                on:mouseleave={stopHold}
                                disabled={!lockConfirmed ||
                                    isSubmitting ||
                                    !newOwnerAddress}
                                style={isHolding
                                    ? "transform: scale(0.98); opacity: 0.9;"
                                    : ""}
                            >
                                {#if isSubmitting}
                                    TRANSFERRING...
                                {:else if isHolding}
                                    HOLD TO TRANSFER ({lockHoldSeconds}s)...
                                {:else}
                                    HOLD 10s TO TRANSFER OWNERSHIP
                                {/if}
                            </button>
                        </div>
                    </div>
                {:else if activeTab === "dividends"}
                    <div class="panel">
                        <p class="section-desc">
                            Distribute dividends to all holders of <strong
                                >{asset ? asset.name : ""}</strong
                            >.
                        </p>

                        <div class="coming-soon">
                            <p>Feature Coming Soon</p>
                            <small
                                >Snapshot and Payout logic requires backend
                                update.</small
                            >
                        </div>
                    </div>
                {:else if activeTab === "metadata"}
                    <div class="panel">
                        <p class="section-desc">
                            Update the IPFS hash associated with this asset.
                        </p>
                        <label for="ipfs-hash">New IPFS Hash / Data</label>
                        <input
                            id="ipfs-hash"
                            type="text"
                            bind:value={newIpfsHash}
                            placeholder="Qm..."
                            class="cyber-input"
                        />
                        <div class="actions">
                            <button
                                class="cyber-btn"
                                on:click={handleMetadata}
                                disabled={isSubmitting}
                            >
                                {isSubmitting
                                    ? "Updating..."
                                    : "Update Metadata"}
                            </button>
                        </div>
                    </div>
                {:else if activeTab === "lock"}
                    <div class="panel danger-zone">
                        <div class="warning-icon">⚠️</div>
                        <h4>Danger Zone</h4>

                        {#if asset?.reissuable === false}
                            <p class="locked-msg">
                                This asset is <strong>ALREADY LOCKED</strong>.
                            </p>
                            <p>
                                The supply cannot be changed. This action is
                                permanent and has already been taken.
                            </p>

                            <div class="actions">
                                <button
                                    class="cyber-btn danger"
                                    disabled
                                    style="opacity: 0.5; cursor: not-allowed;"
                                >
                                    SUPPLY LOCKED FOREVER
                                </button>
                            </div>
                        {:else}
                            <p>
                                This will <strong>PERMANENTLY LOCK</strong> the
                                supply of {asset ? asset.name : ""}.
                            </p>
                            <p>
                                No more tokens can ever be minted. This action
                                cannot be undone.
                            </p>

                            <label class="confirm-check">
                                <input
                                    type="checkbox"
                                    bind:checked={lockConfirmed}
                                />
                                <span>I understand this is permanent.</span>
                            </label>

                            <div class="actions">
                                <button
                                    class="cyber-btn danger"
                                    on:mousedown={startHold}
                                    on:touchstart={startHold}
                                    on:mouseup={stopHold}
                                    on:touchend={stopHold}
                                    on:mouseleave={stopHold}
                                    disabled={!lockConfirmed || isSubmitting}
                                    style={isHolding
                                        ? "transform: scale(0.98); opacity: 0.9;"
                                        : ""}
                                >
                                    {#if isSubmitting}
                                        LOCKING...
                                    {:else if isHolding}
                                        HOLD TO LOCK ({lockHoldSeconds}s)...
                                    {:else}
                                        HOLD 10s TO LOCK FOREVER
                                    {/if}
                                </button>
                            </div>
                        {/if}
                    </div>
                {/if}
            </div>
        </div>
    </div>

    <!-- Alert Modal for Success/Error -->
    <ModalAlert
        isOpen={alertOpen}
        title={alertTitle}
        message={alertMessage}
        type={alertType}
        on:close={handleAlertClose}
    />
{/if}

<style>
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.85);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 200000; /* Must be > 99999 (Detail Modal) */
        backdrop-filter: blur(5px);
    }
    .modal {
        width: 600px; /* Widened to fit all tabs on one line */
        max-width: 95vw;
        border: 1px solid rgba(0, 255, 65, 0.2);
        box-shadow: 0 0 30px rgba(0, 0, 0, 0.8);
        border-radius: 8px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }

    @media (max-height: 700px) {
        .modal-body {
            min-height: 300px;
            overflow-y: auto;
        }
    }
    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.8rem 1.25rem;
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
        font-size: 1.1rem;
    }
    /* ... */
    .modal-body {
        padding: 1.25rem;
        min-height: 310px; /* Fixed height to accommodate Lock Tab without jumping */
        display: flex;
        flex-direction: column;
        justify-content: center;
    }
    .section-desc {
        color: #aaa;
        font-size: 0.85rem;
        margin-bottom: 1rem;
        line-height: 1.3;
    }
    .cyber-input {
        width: 100%;
        padding: 0.6rem;
        margin-bottom: 1rem;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        color: #fff;
        font-family: var(--font-mono);
    }
    .cyber-input:focus {
        border-color: var(--color-primary);
        outline: none;
    }
    .actions {
        display: flex;
        justify-content: flex-end;
    }
    .danger-zone {
        border: 1px solid rgba(255, 0, 0, 0.2);
        background: rgba(255, 0, 0, 0.05);
        padding: 1rem; /* Compact padding */
        border-radius: 8px;
        text-align: center;
    }
    .danger-zone h4 {
        color: #ff3333;
        margin: 0.2rem 0; /* Tighter */
        font-size: 1rem;
    }
    .locked-msg {
        color: #ffaaaa;
        font-weight: bold;
        text-shadow: 0 0 10px rgba(255, 0, 0, 0.3);
        margin-bottom: 0.5rem;
    }
    .danger-zone p {
        color: #ddd;
        font-size: 0.85rem;
        margin-bottom: 0.5rem;
    }
    .warning-icon {
        font-size: 1.5rem; /* Smaller icon */
        margin-bottom: 0.2rem;
    }
    .cyber-btn.danger {
        border-color: #ff3333;
        color: #ff3333;
        box-shadow: none;
    }
    .cyber-btn.danger:hover:not(:disabled) {
        background: #ff3333;
        color: #fff;
        box-shadow: 0 0 15px rgba(255, 0, 0, 0.4);
    }
    .confirm-check {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
        margin: 0.8rem 0; /* Compact margin */
        cursor: pointer;
        color: #ffaaaa;
        font-size: 0.85rem;
    }
    .warning-text {
        color: #ffaaaa;
        font-size: 0.8rem;
        margin-bottom: 0.5rem;
    }
    .coming-soon {
        text-align: center;
        padding: 2rem;
        border: 1px dashed rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        color: #aaa;
    }
</style>
