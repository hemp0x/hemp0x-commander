<script>
    import { createEventDispatcher } from "svelte";
    import { fade, scale } from "svelte/transition";
    import { invoke } from "@tauri-apps/api/core";
    import "../../components.css";
    import ModalAlert from "./ModalAlert.svelte";
    import AddressBookPicker from "../ui/AddressBookPicker.svelte";
    import WalletUnlockModal from "../ui/WalletUnlockModal.svelte";
    import { ensureNodeSyncedForBroadcast } from "../utils/nodeSync.js";

    export let isOpen = false;
    export let inline = false;
    export let showBack = false;
    export let asset; // { name, balance, hasOwner, reissuable, ipfs_hash, ... }

    const dispatch = createEventDispatcher();

    function goBack() {
        dispatch("back");
    }

    function openRewards() {
        dispatch("openRewards");
    }

    let activeTab = "transfer"; // transfer, dividends, lock
    let newOwnerAddress = "";

    // Safety & Loading State
    let lockConfirmed = false;
    let isSubmitting = false;
    let lockHoldSeconds = 10;

    // Wallet unlock
    let showUnlockModal = false;
    let unlockPassword = "";
    let unlocking = false;
    let unlockError = "";
    let unlockActionType = "";
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

    function isWalletUnlockError(err) {
        const text = String(err || "");
        const lower = text.toLowerCase();
        return text.includes("ERROR CODE: -13")
            || lower.includes("walletpassphrase")
            || lower.includes("wallet passphrase")
            || lower.includes("please enter the wallet passphrase")
            || /wallet.*locked|passphrase|unlock/i.test(text);
    }

    function requestWalletUnlock(actionType) {
        unlockPassword = "";
        unlockError = "";
        unlockActionType = actionType;
        showUnlockModal = true;
    }

    async function unlockAndRetry() {
        if (!unlockPassword.trim() || unlocking) return;
        unlocking = true;
        unlockError = "";
        try {
            await invoke("wallet_unlock", { password: unlockPassword, duration: 300 });
            unlockPassword = "";
            showUnlockModal = false;
            if (unlockActionType === "transfer") {
                await handleTransferLogic();
            } else if (unlockActionType === "lock") {
                await handleLock();
            }
        } catch (err) {
            if (isWalletUnlockError(err)) {
                unlockError = "Wallet unlock failed. Check the passphrase and try again.";
            } else {
                unlockError = String(err);
            }
        } finally {
            unlocking = false;
        }
    }

    // Called by timer when 10s is up
    async function handleTransferLogic() {
        isSubmitting = true;
        try {
            await ensureNodeSyncedForBroadcast();
        } catch (e) {
            isSubmitting = false;
            triggerAlert("Node Not Synced", String(e), "error");
            return;
        }
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
            if (isWalletUnlockError(e)) {
                isSubmitting = false;
                requestWalletUnlock("transfer");
                return;
            }
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
            await ensureNodeSyncedForBroadcast();
        } catch (e) {
            isSubmitting = false;
            triggerAlert("Node Not Synced", String(e), "error");
            return;
        }
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
            if (isWalletUnlockError(e)) {
                isSubmitting = false;
                requestWalletUnlock("lock");
                return;
            }
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

{#snippet panelContent()}
    <div class="modal-header">
        {#if showBack}
            <button class="back-btn" on:click={goBack} title="Back to Asset">←</button>
        {/if}
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
                <AddressBookPicker
                    id="owner-address"
                    label="New Owner Address"
                    bind:value={newOwnerAddress}
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

                <div class="dividends-soon">
                    <p class="dividends-soon-title">Feature coming soon.</p>
                    <p class="dividends-soon-body">
                        Owner dividends distribution is being finalized. In the
                        meantime, you can run a snapshot-based payout from
                        Advanced / Rewards, or use the dividends function in
                        WebCom.
                    </p>
                    <div class="dividends-soon-actions">
                        <button type="button" class="cyber-btn small" on:click={openRewards}>
                            Open Advanced / Rewards
                        </button>
                    </div>
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
{/snippet}

{#if isOpen}
    {#if inline}
        <div class="gov-panel" in:fade={{ duration: 150 }}>
            {@render panelContent()}
        </div>
    {:else}
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
                {@render panelContent()}
            </div>
        </div>
    {/if}

    <!-- Alert Modal for Success/Error -->
    <ModalAlert
        isOpen={alertOpen}
        title={alertTitle}
        message={alertMessage}
        type={alertType}
        on:close={handleAlertClose}
    />
    <WalletUnlockModal
        show={showUnlockModal}
        bind:password={unlockPassword}
        {unlocking}
        error={unlockError}
        title="UNLOCK WALLET"
        body="Your wallet is locked. Commander will unlock it for 5 minutes to broadcast this transaction."
        confirmLabel="UNLOCK AND BROADCAST"
        on:cancel={() => { showUnlockModal = false; unlockPassword = ""; unlockError = ""; }}
        on:confirm={unlockAndRetry}
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
        padding: 0.75rem;
        z-index: 200000; /* Must be > 99999 (Detail Modal) */
        backdrop-filter: blur(5px);
        box-sizing: border-box;
    }
    .modal {
        width: min(600px, 92vw);
        max-width: 92vw;
        max-height: min(44rem, calc(100dvh - 2rem));
        border: 1px solid rgba(0, 255, 65, 0.2);
        box-shadow: 0 20px 50px rgba(0, 0, 0, 0.8);
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
        padding: 0.45rem 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        flex-shrink: 0;
    }
    .modal-header h3 {
        margin: 0;
        color: var(--color-primary);
        font-size: 0.9rem;
        letter-spacing: 1px;
    }
    .back-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.2rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .back-btn:hover {
        color: var(--color-primary);
    }
    .close-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.3rem;
        cursor: pointer;
        transition: all 0.15s;
        padding: 0.15rem 0.4rem;
        line-height: 1;
    }
    .close-btn:hover { color: #fff; }

    /* Tabs — match asset detail / advanced style */
    .tabs {
        display: flex;
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        flex-shrink: 0;
    }
    .tabs button {
        flex: 1;
        padding: 0.5rem 0.4rem;
        background: none;
        border: none;
        color: #555;
        font-size: 0.58rem;
        font-weight: 600;
        cursor: pointer;
        letter-spacing: 1px;
        transition: all 0.2s;
        border-bottom: 2px solid transparent;
        text-transform: uppercase;
    }
    .tabs button:hover { color: #aaa; }
    .tabs button.active {
        color: var(--color-primary);
        border-bottom-color: var(--color-primary);
    }
    .tabs button.tab-danger.active {
        color: #ff5555;
        border-bottom-color: #ff5555;
    }
    .tabs button:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .modal-body {
        padding: 0.6rem 1rem;
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        overflow-y: auto;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .modal-body::-webkit-scrollbar {
        width: 8px;
    }
    .modal-body::-webkit-scrollbar-track {
        background: transparent;
    }
    .modal-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.35);
        border-radius: 4px;
    }
    .modal-body::-webkit-scrollbar-thumb:hover {
        background: rgba(0, 255, 65, 0.55);
    }
    .panel {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
    }
    .section-desc {
        color: #aaa;
        font-size: 0.72rem;
        margin-bottom: 0;
        line-height: 1.35;
    }
    .actions {
        display: flex;
        justify-content: flex-end;
        margin-top: 0.25rem;
    }
    .danger-zone {
        border: 1px solid rgba(255, 0, 0, 0.2);
        background: rgba(255, 0, 0, 0.05);
        padding: 0.6rem;
        border-radius: 8px;
        text-align: center;
        gap: 0.3rem;
    }
    .danger-zone h4 {
        color: #ff3333;
        margin: 0;
        font-size: 0.85rem;
        letter-spacing: 0.5px;
    }
    .locked-msg {
        color: #ffaaaa;
        font-weight: bold;
        font-size: 0.72rem;
        margin-bottom: 0.3rem;
    }
    .danger-zone p {
        color: #ddd;
        font-size: 0.72rem;
        margin-bottom: 0.3rem;
        line-height: 1.35;
    }
    .danger-zone p:last-child {
        margin-bottom: 0;
    }
    .warning-icon {
        font-size: 1.2rem;
        margin-bottom: 0.1rem;
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
        gap: 0.4rem;
        margin: 0.5rem 0;
        cursor: pointer;
        color: #ffaaaa;
        font-size: 0.7rem;
    }
    .warning-text {
        color: #ffaaaa;
        font-size: 0.7rem;
        margin-bottom: 0.3rem;
    }
    .dividends-soon {
        background: rgba(0, 200, 255, 0.05);
        border: 1px solid rgba(0, 200, 255, 0.22);
        border-radius: 8px;
        padding: 0.85rem 1rem;
        margin-top: 0.5rem;
    }
    .dividends-soon-title {
        margin: 0 0 0.35rem;
        color: var(--color-primary);
        font-size: 0.78rem;
        letter-spacing: 0.5px;
        font-weight: 700;
    }
    .dividends-soon-body {
        margin: 0 0 0.6rem;
        color: #9fb6c4;
        font-size: 0.68rem;
        line-height: 1.45;
    }
    .dividends-soon-actions {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .gov-panel {
        flex: 1;
        min-height: 0;
        display: flex;
        flex-direction: column;
    }
    .gov-panel .modal-body {
        min-height: 0;
        flex: 1 1 0%;
    }
</style>
