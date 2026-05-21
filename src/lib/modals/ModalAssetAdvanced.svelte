<script>
    import { createEventDispatcher } from "svelte";
    import { fade, scale } from "svelte/transition";
    import { invoke } from "@tauri-apps/api/core";
    import "../../components.css";
    import ModalAlert from "./ModalAlert.svelte";
    import ModalConfirm from "./ModalConfirm.svelte";

    export let isOpen = false;
    export let nodeOnline = false;

    const dispatch = createEventDispatcher();

    // Tabs
    let activeTab = "qualifier"; // qualifier, restricted, tags, snapshot, rewards

    // Qualifier State
    let qualifierName = "";
    let qualifierQty = "1";
    let qualifierIpfs = "";
    let qualifierDest = "";

    // Restricted State
    let restrictedName = "";
    let restrictedQty = "1";
    let restrictedVerifier = "";
    let restrictedDest = "";
    let restrictedUnits = 0;
    let restrictedReissuable = true;
    let restrictedIpfs = "";

    // Tag State
    let tagName = "";
    let tagAddr = "";
    let tagAction = "add"; // add or remove

    // Tag Lookup State
    let tagLookupAddr = "";
    let tagLookupResults = [];
    let tagLookupLoading = false;

    // Verifier Lookup State
    let verifierLookupName = "";
    let verifierString = "";
    let verifierLookupLoading = false;

    // Snapshot State
    let snapAssetName = "";
    let snapBlockHeight = "";
    let snapRequests = [];
    let snapRequestsLoading = false;
    let snapGetName = "";
    let snapGetHeight = "";
    let snapData = null;
    let snapGetLoading = false;

    // Rewards State
    let rewardOwnershipAsset = "";
    let rewardSnapshotHeight = "";
    let rewardDistAsset = "HEMP";
    let rewardGrossAmount = "";
    let rewardExceptions = "";
    let rewardStatusData = null;
    let rewardStatusLoading = false;

    // Alerts
    let alertOpen = false;
    let alertTitle = "";
    let alertMessage = "";
    let alertType = "info";

    // Confirm Modal
    let confirmOpen = false;
    let confirmType = "";
    let previewData = null;
    let isBroadcasting = false;
    let previewInProgress = false;
    let confirmPayload = null;
    let previewJournalId = null;

    function close() {
        dispatch("close");
    }

    function triggerAlert(title, message, type = "info") {
        alertTitle = title;
        alertMessage = message;
        alertType = type;
        alertOpen = true;
    }

    function resetFields() {
        qualifierName = "";
        qualifierQty = "1";
        qualifierIpfs = "";
        qualifierDest = "";
        restrictedName = "";
        restrictedQty = "1";
        restrictedVerifier = "";
        restrictedDest = "";
        restrictedUnits = 0;
        restrictedReissuable = true;
        restrictedIpfs = "";
        tagName = "";
        tagAddr = "";
        tagAction = "add";
        snapGetName = "";
        snapGetHeight = "";
        snapData = null;
        snapGetLoading = false;
        rewardOwnershipAsset = "";
        rewardSnapshotHeight = "";
        rewardDistAsset = "HEMP";
        rewardGrossAmount = "";
        rewardExceptions = "";
        rewardStatusData = null;
        rewardStatusLoading = false;
    }

    // ---- Qualifier Operations ----

    async function previewQualifier() {
        if (previewInProgress) return;
        if (!qualifierName || !qualifierQty) {
            triggerAlert("Validation", "Qualifier name and quantity are required.", "error");
            return;
        }
        previewInProgress = true;
        try {
            previewData = await invoke("preview_issue_qualifier_asset", {
                name: qualifierName,
                qty: qualifierQty,
                destination: qualifierDest || null,
                ipfs: qualifierIpfs || null,
            });
            confirmType = "QUALIFIER ISSUE";
            confirmOpen = true;
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "issue_qualifier",
                        summary: previewData.summary,
                        txid: null,
                        details: previewData,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
        } catch (err) {
            triggerAlert("Preview Failed", String(err), "error");
        } finally {
            previewInProgress = false;
        }
    }

    // ---- Restricted Operations ----

    async function previewRestricted() {
        if (previewInProgress) return;
        if (!restrictedName || !restrictedQty || !restrictedVerifier) {
            triggerAlert("Validation", "Restricted asset name, quantity, and verifier are required.", "error");
            return;
        }
        previewInProgress = true;
        try {
            previewData = await invoke("preview_issue_restricted_asset", {
                name: restrictedName,
                qty: restrictedQty,
                verifier: restrictedVerifier,
                destination: restrictedDest || null,
                units: restrictedUnits,
                reissuable: restrictedReissuable,
                ipfs: restrictedIpfs || null,
            });
            confirmType = "RESTRICTED ASSET ISSUE";
            confirmOpen = true;
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "issue_restricted",
                        summary: previewData.summary,
                        txid: null,
                        details: previewData,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
        } catch (err) {
            triggerAlert("Preview Failed", String(err), "error");
        } finally {
            previewInProgress = false;
        }
    }

    // ---- Tag Operations ----

    async function doTagAction() {
        if (previewInProgress) return;
        if (!tagName || !tagAddr) {
            triggerAlert("Validation", "Tag name and address are required.", "error");
            return;
        }
        previewInProgress = true;
        try {
            if (tagAction === "add") {
                confirmPayload = { tag_name: tagName, address: tagAddr };
                previewData = await invoke("preview_add_tag_to_address", {
                    tagName: tagName,
                    address: tagAddr,
                });
                confirmType = "ADD TAG";
            } else {
                confirmPayload = { tag_name: tagName, address: tagAddr };
                previewData = await invoke("preview_remove_tag_from_address", {
                    tagName: tagName,
                    address: tagAddr,
                });
                confirmType = "REMOVE TAG";
            }
            confirmOpen = true;
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: tagAction === "add" ? "add_tag" : "remove_tag",
                        summary: previewData.summary,
                        txid: null,
                        details: previewData,
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
        } catch (err) {
            triggerAlert("Preview Failed", String(err), "error");
        } finally {
            previewInProgress = false;
        }
    }

    async function lookupTags() {
        if (!tagLookupAddr) return;
        tagLookupLoading = true;
        try {
            tagLookupResults = await invoke("list_tags_for_address", {
                address: tagLookupAddr,
            });
        } catch (err) {
            tagLookupResults = [];
            triggerAlert("Lookup Failed", String(err), "error");
        }
        tagLookupLoading = false;
    }

    async function lookupVerifier() {
        if (!verifierLookupName) return;
        verifierLookupLoading = true;
        try {
            verifierString = await invoke("get_verifier_string", {
                restrictedName: verifierLookupName,
            });
        } catch (err) {
            verifierString = "";
            triggerAlert("Lookup Failed", String(err), "error");
        }
        verifierLookupLoading = false;
    }

    // ---- Snapshot Operations ----

    async function doSnapshotRequest() {
        if (!snapAssetName || !snapBlockHeight) {
            triggerAlert("Validation", "Asset name and block height are required.", "error");
            return;
        }
        const height = Number(snapBlockHeight);
        if (!Number.isInteger(height) || height <= 0) {
            triggerAlert("Validation", "Block height must be a positive whole number.", "error");
            return;
        }
        try {
            const result = await invoke("request_snapshot", {
                assetName: snapAssetName.trim(),
                blockHeight: height,
            });
            triggerAlert("Snapshot Requested", JSON.stringify(result, null, 2), "success");
            snapAssetName = "";
            snapBlockHeight = "";
        } catch (err) {
            triggerAlert("Request Failed", String(err), "error");
        }
    }

    async function listSnapshots() {
        snapRequestsLoading = true;
        try {
            snapRequests = await invoke("list_snapshot_requests", {});
        } catch (err) {
            snapRequests = [];
            triggerAlert("List Failed", String(err), "error");
        }
        snapRequestsLoading = false;
    }

    async function getSnapshot() {
        if (!snapGetName || !snapGetHeight) {
            triggerAlert("Validation", "Asset name and block height are required.", "error");
            return;
        }
        const height = Number(snapGetHeight);
        if (!Number.isInteger(height) || height <= 0) {
            triggerAlert("Validation", "Block height must be a positive whole number.", "error");
            return;
        }
        snapGetLoading = true;
        try {
            snapData = await invoke("get_asset_snapshot", {
                assetName: snapGetName.trim(),
                blockHeight: height,
            });
        } catch (err) {
            snapData = null;
            triggerAlert("Get Failed", String(err), "error");
        }
        snapGetLoading = false;
    }

    // ---- Rewards / Dividends ----

    async function previewReward() {
        if (previewInProgress) return;
        if (!rewardOwnershipAsset || !rewardSnapshotHeight || !rewardGrossAmount) {
            triggerAlert("Validation", "Ownership asset, snapshot height, and gross amount are required.", "error");
            return;
        }
        const height = Number(rewardSnapshotHeight);
        if (!Number.isInteger(height) || height <= 0) {
            triggerAlert("Validation", "Snapshot block height must be a positive whole number.", "error");
            return;
        }
        previewInProgress = true;
        try {
            previewData = await invoke("preview_distribute_reward", {
                ownershipAsset: rewardOwnershipAsset.trim(),
                snapshotHeight: height,
                distributionAsset: rewardDistAsset.trim() || "HEMP",
                grossAmount: rewardGrossAmount.trim(),
                exceptionAddresses: rewardExceptions.trim() || null,
            });
            confirmType = "REWARD DISTRIBUTION";
            confirmOpen = true;
            try {
                const entry = await invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "distribute_reward",
                        summary: previewData.summary,
                        txid: null,
                        details: {
                            ownership_asset: previewData.ownership_asset,
                            snapshot_height: previewData.snapshot_height,
                            distribution_asset: previewData.distribution_asset,
                            gross_amount: previewData.gross_amount,
                            exception_addresses: previewData.exception_addresses,
                        },
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }
        } catch (err) {
            triggerAlert("Preview Failed", String(err), "error");
        } finally {
            previewInProgress = false;
        }
    }

    async function checkRewardStatus() {
        if (!rewardOwnershipAsset || !rewardSnapshotHeight || !rewardGrossAmount) {
            triggerAlert("Validation", "Ownership asset, snapshot height, and gross amount are required for status check.", "error");
            return;
        }
        const height = Number(rewardSnapshotHeight);
        if (!Number.isInteger(height) || height <= 0) {
            triggerAlert("Validation", "Snapshot block height must be a positive whole number.", "error");
            return;
        }
        rewardStatusLoading = true;
        try {
            rewardStatusData = await invoke("get_distribute_reward_status", {
                ownershipAsset: rewardOwnershipAsset.trim(),
                snapshotHeight: height,
                distributionAsset: rewardDistAsset.trim() || "HEMP",
                grossAmount: rewardGrossAmount.trim(),
                exceptionAddresses: rewardExceptions.trim() || null,
            });
        } catch (err) {
            rewardStatusData = null;
            triggerAlert("Status Check Failed", String(err), "error");
        }
        rewardStatusLoading = false;
    }

    // ---- Confirm Handler ----

    async function confirmAction() {
        isBroadcasting = true;
        try {
            let txid = "";
            let successMessage = "";
            let journalDetails = null;
            if (confirmType === "QUALIFIER ISSUE") {
                txid = await invoke("issue_qualifier_asset", {
                    name: previewData.qualifier_name,
                    qty: previewData.qty,
                    destination: qualifierDest || null,
                    ipfs: qualifierIpfs || null,
                });
            } else if (confirmType === "RESTRICTED ASSET ISSUE") {
                txid = await invoke("issue_restricted_asset", {
                    name: previewData.asset_name,
                    qty: previewData.qty,
                    verifier: restrictedVerifier,
                    destination: restrictedDest || null,
                    units: restrictedUnits,
                    reissuable: restrictedReissuable,
                    ipfs: restrictedIpfs || null,
                });
            } else if (confirmType === "ADD TAG") {
                txid = await invoke("add_tag_to_address", {
                    tagName: confirmPayload.tag_name,
                    address: confirmPayload.address,
                });
            } else if (confirmType === "REMOVE TAG") {
                txid = await invoke("remove_tag_from_address", {
                    tagName: confirmPayload.tag_name,
                    address: confirmPayload.address,
                });
            } else if (confirmType === "REWARD DISTRIBUTION") {
                const result = await invoke("distribute_reward", {
                    ownershipAsset: previewData.ownership_asset,
                    snapshotHeight: previewData.snapshot_height,
                    distributionAsset: previewData.distribution_asset,
                    grossAmount: previewData.gross_amount,
                    exceptionAddresses: previewData.exception_addresses,
                });
                successMessage = result.status || JSON.stringify(result);
                journalDetails = {
                    distribution_status: successMessage,
                    ownership_asset: previewData.ownership_asset,
                    snapshot_height: previewData.snapshot_height,
                    distribution_asset: previewData.distribution_asset,
                    gross_amount: previewData.gross_amount,
                    exception_addresses: previewData.exception_addresses,
                };
            }
            if (!successMessage) {
                successMessage = `Transaction broadcasted. TXID: ${txid}`;
            }

            confirmOpen = false;
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid || null,
                        details: journalDetails,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }
            previewJournalId = null;
            previewData = null;
            confirmPayload = null;
            triggerAlert("Success", successMessage, "success");
            resetFields();
        } catch (err) {
            if (previewJournalId) {
                try {
                    await invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(err) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
            }
            confirmOpen = false;
            previewJournalId = null;
            triggerAlert("Error", String(err), "error");
        } finally {
            isBroadcasting = false;
        }
    }

    function cancelConfirm() {
        if (previewJournalId) {
            invoke("update_tx_journal_entry", {
                id: previewJournalId,
                status: "Abandoned",
                txid: null,
                details: { reason: "user_cancelled" },
            }).catch((e) => console.warn("Failed to mark journal entry as abandoned:", e));
        }
        confirmOpen = false;
        previewData = null;
        previewJournalId = null;
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
                <h3>Advanced Asset Controls</h3>
                <button class="close-btn" on:click={close}>&times;</button>
            </div>

            <!-- Tabs -->
            <div class="tabs">
                <button
                    class:active={activeTab === "qualifier"}
                    on:click={() => (activeTab = "qualifier")}>Qualifiers</button
                >
                <button
                    class:active={activeTab === "restricted"}
                    on:click={() => (activeTab = "restricted")}>Restricted</button
                >
                <button
                    class:active={activeTab === "tags"}
                    on:click={() => (activeTab = "tags")}>Tags</button
                >
                <button
                    class:active={activeTab === "snapshot"}
                    on:click={() => (activeTab = "snapshot")}>Snapshots</button
                >
                <button
                    class:active={activeTab === "rewards"}
                    on:click={() => (activeTab = "rewards")}>Rewards</button
                >
            </div>

            <div class="modal-body">
                {#if !nodeOnline}
                    <div class="offline-banner">
                        Node offline or unavailable. Advanced controls require an active RPC connection.
                    </div>
                {:else}
                    {#if activeTab === "qualifier"}
                        <div class="tab-panel">
                            <h4>Issue Qualifier Asset</h4>
                            <p class="section-desc">
                                Qualifier assets are tag tokens (prefixed with #). Units are fixed at 0, non-reissuable, and amounts must be 1-10.
                            </p>
                            <label for="qualifier-name">Qualifier Name</label>
                            <input id="qualifier-name" type="text" bind:value={qualifierName} placeholder="#TAG or just TAG" class="cyber-input" />
                            <label for="qualifier-qty">Quantity (1-10)</label>
                            <input id="qualifier-qty" type="text" bind:value={qualifierQty} placeholder="1" class="cyber-input" />
                            <label for="qualifier-dest">Destination Address (optional)</label>
                            <input id="qualifier-dest" type="text" bind:value={qualifierDest} placeholder="H..." class="cyber-input" />
                            <label for="qualifier-ipfs">IPFS Hash (optional)</label>
                            <input id="qualifier-ipfs" type="text" bind:value={qualifierIpfs} placeholder="Qm..." class="cyber-input" />
                            <div class="actions">
                                <button class="cyber-btn" on:click={previewQualifier} disabled={previewInProgress}>
                                    {previewInProgress ? "Building Preview..." : "Preview Qualifier Issue"}
                                </button>
                            </div>
                        </div>
                    {:else if activeTab === "restricted"}
                        <div class="tab-panel">
                            <h4>Issue Restricted Asset</h4>
                            <p class="section-desc">
                                Restricted assets (prefixed with $) enforce holder verification. Requires a verifier string using qualifier logic, e.g. <code>#KYC & !#AML</code>.
                            </p>
                            <label for="restricted-name">Asset Name</label>
                            <input id="restricted-name" type="text" bind:value={restrictedName} placeholder="$ASSET or just ASSET" class="cyber-input" />
                            <label for="restricted-qty">Quantity</label>
                            <input id="restricted-qty" type="text" bind:value={restrictedQty} placeholder="1000" class="cyber-input" />
                            <label for="restricted-verifier">Verifier String</label>
                            <input id="restricted-verifier" type="text" bind:value={restrictedVerifier} placeholder="#KYC & !#AML" class="cyber-input" />
                            <label for="restricted-dest">Destination Address (optional)</label>
                            <input id="restricted-dest" type="text" bind:value={restrictedDest} placeholder="H..." class="cyber-input" />
                            <label for="restricted-units">Units (0-8)</label>
                            <input id="restricted-units" type="number" bind:value={restrictedUnits} min="0" max="8" class="cyber-input" />
                            <label class="confirm-check">
                                <input type="checkbox" bind:checked={restrictedReissuable} />
                                <span>Reissuable</span>
                            </label>
                            <label for="restricted-ipfs">IPFS Hash (optional)</label>
                            <input id="restricted-ipfs" type="text" bind:value={restrictedIpfs} placeholder="Qm..." class="cyber-input" />
                            <div class="actions">
                                <button class="cyber-btn" on:click={previewRestricted} disabled={previewInProgress}>
                                    {previewInProgress ? "Building Preview..." : "Preview Restricted Issue"}
                                </button>
                            </div>
                        </div>
                    {:else if activeTab === "tags"}
                        <div class="tab-panel">
                            <h4>Manage Address Tags</h4>
                            <p class="section-desc">
                                Assign or remove qualifier tags from addresses. Requires ownership of the qualifier token.
                            </p>
                            <!-- Tag Action -->
                            <div class="subsection">
                                <h5>Add / Remove Tag</h5>
                                <label for="tag-name">Tag Name</label>
                                <input id="tag-name" type="text" bind:value={tagName} placeholder="#KYC" class="cyber-input" />
                                <label for="tag-address">Address</label>
                                <input id="tag-address" type="text" bind:value={tagAddr} placeholder="H..." class="cyber-input" />
                                <div class="tag-action-row">
                                    <label><input type="radio" bind:group={tagAction} value="add" /> Add Tag</label>
                                    <label><input type="radio" bind:group={tagAction} value="remove" /> Remove Tag</label>
                                </div>
                                <div class="actions">
                                    <button class="cyber-btn warning" on:click={doTagAction} disabled={previewInProgress}>
                                        {previewInProgress ? "Building Preview..." : `Preview Tag ${tagAction === "add" ? "Add" : "Remove"}`}
                                    </button>
                                </div>
                            </div>
                            <!-- Tag Lookup -->
                            <div class="subsection">
                                <h5>Lookup Tags for Address</h5>
                                <label for="tag-lookup-address">Address</label>
                                <input id="tag-lookup-address" type="text" bind:value={tagLookupAddr} placeholder="H..." class="cyber-input" />
                                <div class="actions">
                                    <button class="cyber-btn" on:click={lookupTags} disabled={tagLookupLoading}>
                                        {tagLookupLoading ? "Loading..." : "List Tags"}
                                    </button>
                                </div>
                                {#if tagLookupResults.length > 0}
                                    <div class="result-list">
                                        {#each tagLookupResults as t}
                                            <div class="result-item">{t}</div>
                                        {/each}
                                    </div>
                                {:else if tagLookupResults.length === 0 && !tagLookupLoading}
                                    <p class="result-empty">No tags found for this address.</p>
                                {/if}
                            </div>
                            <!-- Verifier Lookup -->
                            <div class="subsection">
                                <h5>Lookup Verifier String</h5>
                                <label for="verifier-lookup-name">Restricted Asset Name</label>
                                <input id="verifier-lookup-name" type="text" bind:value={verifierLookupName} placeholder="$ASSET" class="cyber-input" />
                                <div class="actions">
                                    <button class="cyber-btn" on:click={lookupVerifier} disabled={verifierLookupLoading}>
                                        {verifierLookupLoading ? "Loading..." : "Get Verifier"}
                                    </button>
                                </div>
                                {#if verifierString}
                                    <div class="result-item verifier-result">{verifierString}</div>
                                {/if}
                            </div>
                        </div>
                    {:else if activeTab === "snapshot"}
                        <div class="tab-panel">
                            <h4>Asset Snapshots</h4>
                            <p class="section-desc">
                                Request snapshots, list pending requests, and retrieve snapshot data. Requires <code>-assetindex</code> enabled on the node.
                            </p>
                            <!-- Request Snapshot -->
                            <div class="subsection">
                                <h5>Request Snapshot</h5>
                                <label for="snapshot-request-asset">Asset Name</label>
                                <input id="snapshot-request-asset" type="text" bind:value={snapAssetName} placeholder="ASSET_NAME" class="cyber-input" />
                                <label for="snapshot-request-height">Block Height</label>
                                <input id="snapshot-request-height" type="number" bind:value={snapBlockHeight} placeholder="Future block number" class="cyber-input" />
                                <div class="actions">
                                    <button class="cyber-btn" on:click={doSnapshotRequest}>
                                        Request Snapshot
                                    </button>
                                </div>
                            </div>
                            <!-- List Snapshots -->
                            <div class="subsection">
                                <h5>Pending Snapshot Requests</h5>
                                <div class="actions">
                                    <button class="cyber-btn" on:click={listSnapshots} disabled={snapRequestsLoading}>
                                        {snapRequestsLoading ? "Loading..." : "List Requests"}
                                    </button>
                                </div>
                                {#if snapRequests.length > 0}
                                    <div class="result-list">
                                        {#each snapRequests as sr}
                                            <div class="result-item">
                                                <span class="snap-asset">{sr.asset_name}</span>
                                                <span class="snap-height">@ {sr.block_height}</span>
                                            </div>
                                        {/each}
                                    </div>
                                {:else if snapRequests.length === 0 && !snapRequestsLoading}
                                    <p class="result-empty">No pending snapshot requests.</p>
                                {/if}
                            </div>
                            <!-- Get Snapshot -->
                            <div class="subsection">
                                <h5>Get Snapshot Data</h5>
                                <label for="snapshot-get-asset">Asset Name</label>
                                <input id="snapshot-get-asset" type="text" bind:value={snapGetName} placeholder="ASSET_NAME" class="cyber-input" />
                                <label for="snapshot-get-height">Block Height</label>
                                <input id="snapshot-get-height" type="number" bind:value={snapGetHeight} placeholder="Completed block number" class="cyber-input" />
                                <div class="actions">
                                    <button class="cyber-btn" on:click={getSnapshot} disabled={snapGetLoading}>
                                        {snapGetLoading ? "Loading..." : "Get Snapshot"}
                                    </button>
                                </div>
                                {#if snapData}
                                    <div class="snapshot-result">
                                        <p><strong>Asset:</strong> {snapData.name} @ height {snapData.height}</p>
                                        <p><strong>Holders:</strong> {snapData.owners.length}</p>
                                        {#if snapData.owners.length > 0}
                                            <div class="result-list">
                                                {#each snapData.owners as owner}
                                                    <div class="result-item">
                                                        <span class="snap-addr">{owner.address}</span>
                                                        <span class="snap-amount">{typeof owner.amount_owned === 'number' ? owner.amount_owned : JSON.stringify(owner.amount_owned)}</span>
                                                    </div>
                                                {/each}
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    {:else if activeTab === "rewards"}
                        <div class="tab-panel">
                            <h4>Rewards / Dividends Distribution</h4>
                            <p class="section-desc">
                                Distribute rewards or dividends to all holders of an asset using a previously completed snapshot.
                                Requires <code>-assetindex</code> enabled on the node and a completed snapshot at the target height.
                            </p>
                            <!-- Distribution Form -->
                            <div class="subsection">
                                <h5>Setup Distribution</h5>
                                <label for="reward-ownership">Ownership Asset</label>
                                <input id="reward-ownership" type="text" bind:value={rewardOwnershipAsset} placeholder="ASSET_NAME whose holders receive rewards" class="cyber-input" />
                                <label for="reward-height">Snapshot Block Height</label>
                                <input id="reward-height" type="number" bind:value={rewardSnapshotHeight} placeholder="Completed snapshot block number" class="cyber-input" />
                                <label for="reward-dist-asset">Distribution Asset</label>
                                <input id="reward-dist-asset" type="text" bind:value={rewardDistAsset} placeholder="HEMP or ASSET_NAME to distribute" class="cyber-input" />
                                <label for="reward-gross">Gross Distribution Amount</label>
                                <input id="reward-gross" type="text" bind:value={rewardGrossAmount} placeholder="Total amount to split among holders" class="cyber-input" />
                                <label for="reward-exceptions">Excluded Addresses (optional)</label>
                                <input id="reward-exceptions" type="text" bind:value={rewardExceptions} placeholder="Comma-separated addresses to exclude" class="cyber-input" />
                                <div class="actions">
                                    <button class="cyber-btn warning" on:click={previewReward} disabled={previewInProgress}>
                                        {previewInProgress ? "Building Preview..." : "Preview Distribution"}
                                    </button>
                                </div>
                            </div>
                            <!-- Status Check -->
                            <div class="subsection">
                                <h5>Check Distribution Status</h5>
                                <p class="section-desc">
                                    For distributions already submitted, use the same parameters to check status.
                                </p>
                                <div class="actions">
                                    <button class="cyber-btn" on:click={checkRewardStatus} disabled={rewardStatusLoading}>
                                        {rewardStatusLoading ? "Checking..." : "Check Status"}
                                    </button>
                                </div>
                                {#if rewardStatusData}
                                    <div class="snapshot-result">
                                        {#each Object.entries(rewardStatusData) as [k, v]}
                                            <p><strong>{k}:</strong> {typeof v === 'object' ? JSON.stringify(v) : v}</p>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    {/if}
                {/if}
            </div>
        </div>
    </div>

    <ModalAlert
        isOpen={alertOpen}
        title={alertTitle}
        message={alertMessage}
        type={alertType}
        on:close={() => (alertOpen = false)}
    />

    <ModalConfirm
        isOpen={confirmOpen}
        type={confirmType}
        payload={confirmPayload}
        {previewData}
        {isBroadcasting}
        on:close={cancelConfirm}
        on:confirm={confirmAction}
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
        z-index: 200000;
        backdrop-filter: blur(5px);
    }
    .modal {
        width: 640px;
        max-width: 95vw;
        max-height: 90vh;
        border: 1px solid rgba(0, 255, 65, 0.2);
        box-shadow: 0 0 30px rgba(0, 0, 0, 0.8);
        border-radius: 16px;
        overflow: hidden;
        display: flex;
        flex-direction: column;
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
    .close-btn {
        background: none;
        border: none;
        color: #888;
        font-size: 1.5rem;
        cursor: pointer;
    }
    .close-btn:hover { color: #fff; }
    .tabs {
        display: flex;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .tabs button {
        flex: 1;
        padding: 0.6rem 0.5rem;
        background: none;
        border: none;
        color: #666;
        font-size: 0.75rem;
        font-weight: 600;
        cursor: pointer;
        letter-spacing: 1px;
        transition: all 0.2s;
    }
    .tabs button:hover { color: #aaa; }
    .tabs button.active {
        color: var(--color-primary);
        border-bottom: 2px solid var(--color-primary);
    }
    .modal-body {
        padding: 1.25rem;
        overflow-y: auto;
        flex: 1;
    }
    .tab-panel {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .tab-panel h4 {
        margin: 0 0 0.3rem 0;
        color: var(--color-primary);
        font-size: 0.9rem;
    }
    .section-desc {
        color: #888;
        font-size: 0.75rem;
        margin-bottom: 0.5rem;
        line-height: 1.3;
    }
    .section-desc code {
        background: rgba(0, 255, 65, 0.1);
        padding: 1px 4px;
        border-radius: 3px;
        font-size: 0.7rem;
    }
    .subsection {
        margin-top: 1rem;
        padding-top: 1rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .subsection h5 {
        margin: 0 0 0.5rem 0;
        color: #aaa;
        font-size: 0.8rem;
    }
    .cyber-input {
        width: 100%;
        padding: 0.5rem;
        margin-bottom: 0.5rem;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        color: #fff;
        font-family: var(--font-mono);
        font-size: 0.8rem;
        box-sizing: border-box;
    }
    .cyber-input:focus {
        border-color: var(--color-primary);
        outline: none;
    }
    label {
        color: #aaa;
        font-size: 0.7rem;
        display: block;
        margin-bottom: 0.2rem;
    }
    .confirm-check {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: #ddd;
        font-size: 0.8rem;
        cursor: pointer;
    }
    .tag-action-row {
        display: flex;
        gap: 1rem;
        margin-bottom: 0.5rem;
    }
    .tag-action-row label {
        color: #ddd;
        font-size: 0.8rem;
        cursor: pointer;
    }
    .actions {
        display: flex;
        justify-content: flex-end;
        margin-top: 0.5rem;
    }
    .cyber-btn {
        padding: 0.5rem 1rem;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        font-size: 0.7rem;
        font-weight: 600;
        border-radius: 6px;
        cursor: pointer;
        letter-spacing: 1px;
        transition: all 0.2s;
    }
    .cyber-btn:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.2);
    }
    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .cyber-btn.warning {
        border-color: rgba(255, 170, 0, 0.5);
        color: #ffaa00;
        background: rgba(255, 170, 0, 0.08);
    }
    .cyber-btn.warning:hover:not(:disabled) {
        background: rgba(255, 170, 0, 0.15);
        box-shadow: 0 0 15px rgba(255, 170, 0, 0.2);
    }
    .offline-banner {
        text-align: center;
        padding: 2rem;
        color: #ff5555;
        font-size: 0.85rem;
        border: 1px solid rgba(255, 0, 0, 0.2);
        border-radius: 8px;
        background: rgba(255, 0, 0, 0.05);
    }
    .result-list {
        margin-top: 0.5rem;
        max-height: 200px;
        overflow-y: auto;
    }
    .result-item {
        padding: 0.4rem 0.6rem;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        margin-bottom: 0.3rem;
        font-size: 0.75rem;
        color: #ccc;
        font-family: var(--font-mono);
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .result-empty {
        color: #555;
        font-size: 0.75rem;
        margin-top: 0.5rem;
    }
    .verifier-result {
        color: var(--color-primary);
        margin-top: 0.5rem;
        font-size: 0.8rem;
    }
    .snap-asset { font-weight: 600; color: #fff; }
    .snap-height { color: #888; }
    .snap-addr { font-size: 0.7rem; color: #aaa; }
    .snap-amount { color: var(--color-primary); }
    .snapshot-result {
        margin-top: 0.5rem;
        padding: 0.5rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 4px;
        border: 1px solid rgba(255, 255, 255, 0.05);
    }
    .snapshot-result p {
        margin: 0.3rem 0;
        font-size: 0.75rem;
        color: #ccc;
    }
</style>
