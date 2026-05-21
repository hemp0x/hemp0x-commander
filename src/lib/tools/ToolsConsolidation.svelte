<script>
    import { onMount } from "svelte";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { formatAmount } from "../utils.js";
    import { systemStatus } from "../../stores.js";
    import { addTransactionNotification, addToolNotification } from "../stores/notifications.js";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    let utxos = [];
    let selectedIds = new Set();
    let totalSelected = 0;
    let minConfirmations = 1;
    let minAmount = 0.0;
    let destination = "";
    let loading = false;
    let previewData = null;
    let previewJournalId = null;
    let showPreviewModal = false;
    let previewing = false;
    let broadcasting = false;
    let status = "";
    let policyDiag = null;
    let showPlanModal = false;
    let planData = null;
    let planning = false;
    let estimatedSelectedBytes = 0;
    let estimatedSelectedFee = "0.00000000";
    let feeRateSatPerByte = 1000;
    let previewFeeRateSatPerByte = 1000;

    function activeFeeRate() {
        const parsed = Number(feeRateSatPerByte);
        if (!Number.isFinite(parsed)) return 1000;
        return Math.max(1, Math.min(10000, Math.trunc(parsed)));
    }

    function normalizeFeeRate() {
        feeRateSatPerByte = activeFeeRate();
        calculateTotal();
    }

    function setFeeRate(rate) {
        feeRateSatPerByte = rate;
        calculateTotal();
    }

    $: effectiveFeeRateSatPerByte = activeFeeRate();
    $: feeRateSatPerByteLabel = `${effectiveFeeRateSatPerByte} sat/B`;
    $: {
        effectiveFeeRateSatPerByte;
        filteredUtxos;
        selectedIds;
        calculateTotal();
    }

    $: safeUtxoCount = (() => {
        let count = 0;
        for (const u of utxos) {
            if (!isUnsafe(u)) count++;
        }
        return count;
    })();

    $: maxSafeInputs = policyDiag?.max_safe_inputs_for_one_output || 607;
    $: exceedsOneRoundMax = safeUtxoCount > maxSafeInputs;

    $: filteredUtxos = (() => {
        return utxos.filter((u) => {
            if (u.confirmations < minConfirmations) return false;
            if (u.amount < minAmount) return false;
            return true;
        });
    })();

    $: selectedCount = (() => {
        let count = 0;
        for (const u of filteredUtxos) {
            if (selectedIds.has(`${u.txid}:${u.vout}`)) count++;
        }
        return count;
    })();

    $: pruneSelectionToFiltered(filteredUtxos);

    function pruneSelectionToFiltered(visibleUtxos) {
        const visibleIds = new Set(visibleUtxos.map((u) => `${u.txid}:${u.vout}`));
        let changed = false;
        const next = new Set();
        for (const id of selectedIds) {
            if (visibleIds.has(id)) {
                next.add(id);
            } else {
                changed = true;
            }
        }
        if (changed) {
            selectedIds = next;
            calculateTotal();
        }
    }

    function calculateTotal() {
        let sum = 0;
        let count = 0;
        for (const u of filteredUtxos) {
            if (selectedIds.has(`${u.txid}:${u.vout}`)) {
                sum += u.amount;
                count++;
            }
        }
        totalSelected = sum;
        estimatedSelectedBytes = 10 + count * 148 + 1 * 34;
        estimatedSelectedFee = ((estimatedSelectedBytes * effectiveFeeRateSatPerByte) / 100000000).toFixed(8);
    }

    function toggleUtxo(id) {
        const utxo = utxos.find((u) => `${u.txid}:${u.vout}` === id);
        if (utxo && isUnsafe(utxo)) {
            status = "Unsafe, unspendable, or asset-bearing UTXOs are excluded from HEMP consolidation.";
            return;
        }
        if (selectedIds.has(id)) {
            selectedIds.delete(id);
        } else {
            selectedIds.add(id);
        }
        selectedIds = selectedIds;
        calculateTotal();
    }

    function selectAll() {
        for (const u of filteredUtxos) {
            if (!isUnsafe(u)) {
                selectedIds.add(`${u.txid}:${u.vout}`);
            }
        }
        selectedIds = selectedIds;
        calculateTotal();
    }

    function selectAllSafe() {
        clearSelected();
        for (const u of filteredUtxos) {
            if (!isUnsafe(u)) {
                selectedIds.add(`${u.txid}:${u.vout}`);
            }
        }
        selectedIds = selectedIds;
        calculateTotal();
    }

    function selectSafeMax() {
        clearSelected();
        const safe = filteredUtxos
            .filter((u) => !isUnsafe(u))
            .sort((a, b) => a.amount - b.amount);
        const maxInputs = policyDiag?.max_safe_inputs_for_one_output || 607;
        for (let i = 0; i < Math.min(safe.length, maxInputs); i++) {
            selectedIds.add(`${safe[i].txid}:${safe[i].vout}`);
        }
        selectedIds = selectedIds;
        calculateTotal();
    }

    async function handlePlanBatches() {
        if (!tauriReady || planning) return;
        planning = true;
        planData = null;
        status = "Building consolidation plan...";
        try {
            planData = await core.invoke("plan_wallet_consolidation", {
                destination: destination.trim() || null,
                targetFinalUtxoCount: null,
                maxRounds: null,
                targetMaxTxBytes: null,
                feeRateSatPerByte: effectiveFeeRateSatPerByte,
            });
            status = "";
            showPlanModal = true;
        } catch (err) {
            status = "Plan failed: " + err;
            planData = null;
        } finally {
            planning = false;
        }
    }

    function clearSelected() {
        selectedIds = new Set();
        selectedIds = selectedIds;
        totalSelected = 0;
        calculateTotal();
    }

    function isUnsafe(u) {
        if (u.spendable === false) return true;
        if (u.safe === false) return true;
        if (u.asset && u.asset !== "HEMP") return true;
        if (u.asset_amount && u.asset_amount > 0) return true;
        return false;
    }

    function unsafeCount() {
        let count = 0;
        for (const u of filteredUtxos) {
            if (selectedIds.has(`${u.txid}:${u.vout}`) && isUnsafe(u)) count++;
        }
        return count;
    }

    async function fetchUtxos(refreshAddr = false) {
        if (!tauriReady) return;
        loading = true;
        status = "";
        try {
            utxos = await core.invoke("list_utxos");
            utxos.sort((a, b) => b.amount - a.amount);
            policyDiag = await core.invoke("get_policy_diagnostics", {
                feeRateSatPerByte: effectiveFeeRateSatPerByte,
            });
        } catch (err) {
            showToast("Failed to list UTXOs: " + err, "error");
        }
        loading = false;

        if (refreshAddr || !destination) {
            await refreshDestination();
        }
    }

    async function refreshDestination() {
        if (!tauriReady) return;
        try {
            const addr = await core.invoke("new_address", { label: null });
            destination = addr;
        } catch (err) {
            showToast("Failed to get new address: " + err, "error");
        }
    }

    async function handlePreview() {
        if (!tauriReady) return;
        if (previewing || broadcasting) return;
        if (selectedCount === 0) {
            status = "Select at least one UTXO to consolidate.";
            return;
        }
        if (!destination.trim()) {
            status = "Consolidation destination address is required.";
            return;
        }

        status = "Building consolidation preview...";
        previewing = true;
        previewData = null;
        previewJournalId = null;

        const inputs = [];
        for (const u of filteredUtxos) {
            const id = `${u.txid}:${u.vout}`;
            if (selectedIds.has(id)) {
                inputs.push({ txid: u.txid, vout: u.vout });
            }
        }

        try {
            previewData = await core.invoke("preview_wallet_consolidation", {
                utxos: inputs,
                destination: destination.trim(),
                feeRateSatPerByte: effectiveFeeRateSatPerByte,
            });
            previewFeeRateSatPerByte = effectiveFeeRateSatPerByte;
            status = "";

            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "wallet_consolidation",
                        summary: previewData.summary,
                        txid: null,
                        details: {
                            utxo_count: previewData.utxo_count,
                            input_total: previewData.input_total,
                            output_amount: previewData.output_amount,
                            fee_estimate: previewData.fee_estimate,
                            fee_rate_sat_per_byte: previewFeeRateSatPerByte,
                            destination: previewData.destination,
                            warnings: previewData.warnings,
                            selected_utxos: (previewData.utxos || []).map((u) => ({
                                txid: u.txid,
                                vout: u.vout,
                                amount: u.amount,
                            })),
                        },
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }

            showPreviewModal = true;
            addToolNotification(
                "Consolidation preview ready",
                `${previewData.utxo_count} UTXOs, ${previewData.input_total} HEMP in, ${previewData.fee_estimate} HEMP fee`,
                "info",
            );
        } catch (err) {
            status = "Preview failed: " + err;
            previewData = null;
            addToolNotification(
                "Consolidation preview failed",
                String(err).substring(0, 200),
                "error",
            );
        } finally {
            previewing = false;
        }
    }

    async function executeConsolidation() {
        showPreviewModal = false;
        broadcasting = true;
        status = "Broadcasting consolidation...";

        const inputs = [];
        for (const u of filteredUtxos) {
            const id = `${u.txid}:${u.vout}`;
            if (selectedIds.has(id)) {
                inputs.push({ txid: u.txid, vout: u.vout });
            }
        }

        const fee = parseFloat(previewData?.fee_estimate || "0.01");

        try {
            const txid = await core.invoke("broadcast_wallet_consolidation", {
                utxos: inputs,
                destination: destination.trim(),
                fee,
                feeRateSatPerByte: previewFeeRateSatPerByte,
            });

            status = "Consolidated! TXID: " + txid.substring(0, 16) + "...";
            addTransactionNotification(
                "Consolidation broadcasted",
                `${previewData.utxo_count} UTXOs merged`,
                "success",
                txid,
            );

            if (previewJournalId) {
                try {
                    await core.invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Broadcasted",
                        txid: txid,
                        details: null,
                    });
                } catch (journalErr) {
                    console.warn("Failed to update journal entry:", journalErr);
                }
            }

            previewData = null;
            previewJournalId = null;
            selectedIds = new Set();
            selectedIds = selectedIds;
            totalSelected = 0;
            await fetchUtxos(true);
        } catch (err) {
            status = "Consolidation failed: " + err;
            addToolNotification("Consolidation failed", String(err).substring(0, 200), "error");
            if (previewJournalId) {
                try {
                    await core.invoke("update_tx_journal_entry", {
                        id: previewJournalId,
                        status: "Failed",
                        txid: null,
                        details: { error: String(err) },
                    });
                } catch (journalErr) {
                    console.warn("Failed to record journal failure:", journalErr);
                }
            }
        }

        broadcasting = false;
    }

    function cancelConsolidation() {
        if (previewJournalId) {
            core.invoke("update_tx_journal_entry", {
                id: previewJournalId,
                status: "Abandoned",
                txid: null,
                details: { reason: "user_cancelled" },
            }).catch((e) => console.warn("Failed to mark journal entry as abandoned:", e));
        }
        showPreviewModal = false;
        previewData = null;
        previewJournalId = null;
        status = "";
    }

    onMount(() => {
        fetchUtxos(true);
    });
</script>

<div class="consolidation-view">
    <div class="consolidation-header-bar">
        <span class="header-label mono">WALLET CONSOLIDATION</span>
        <span class="header-sub">Merge multiple UTXOs into a single wallet address</span>
    </div>

    <div class="destination-row">
        <div class="dest-group">
            <label for="cons-dest">DESTINATION ADDRESS</label>
            <div class="input-group brackets">
                <input
                    type="text"
                    id="cons-dest"
                    class="input-glass mono full-width"
                    bind:value={destination}
                    placeholder="Wallet consolidation address..."
                />
                <button class="icon-btn" on:click={refreshDestination} title="Generate new address">
                    NEW
                </button>
            </div>
        </div>
    </div>

    <div class="filter-row">
        <div class="filter-group">
            <label for="cons-min-conf">MIN CONFIRMATIONS</label>
            <select id="cons-min-conf" class="input-glass" bind:value={minConfirmations}>
                <option value={0}>0</option>
                <option value={1}>1</option>
                <option value={6}>6</option>
                <option value={12}>12</option>
                <option value={100}>100</option>
            </select>
        </div>
        <div class="filter-group">
            <label for="cons-min-amount">MIN AMOUNT (HEMP)</label>
            <input
                type="number"
                id="cons-min-amount"
                class="input-glass"
                bind:value={minAmount}
                step="0.001"
                min="0"
            />
        </div>
        <div class="filter-group actions-right">
            <button class="cyber-btn ghost" on:click={selectAllSafe}>SEL ALL SAFE</button>
            <button class="cyber-btn ghost" on:click={selectSafeMax}>SEL SAFE MAX</button>
            <button class="cyber-btn ghost" on:click={clearSelected}>CLEAR</button>
            <button class="cyber-btn" on:click={() => fetchUtxos(false)} disabled={loading}>
                {loading ? "LOADING..." : "REFRESH"}
            </button>
        </div>
    </div>

    <div class="fee-rate-row">
        <div class="filter-group">
            <label for="cons-fee-rate">FEE RATE (sat/byte)</label>
            <div class="fee-rate-control">
                <button
                    class="cyber-btn ghost preset"
                    class:active={effectiveFeeRateSatPerByte === 10}
                    on:click={() => setFeeRate(10)}
                >10</button
                >
                <button
                    class="cyber-btn ghost preset"
                    class:active={effectiveFeeRateSatPerByte === 100}
                    on:click={() => setFeeRate(100)}
                >100</button
                >
                <button
                    class="cyber-btn ghost preset"
                    class:active={effectiveFeeRateSatPerByte === 1000}
                    on:click={() => setFeeRate(1000)}
                >1000</button
                >
                <input
                    id="cons-fee-rate"
                    type="number"
                    class="input-glass fee-rate-input"
                    bind:value={feeRateSatPerByte}
                    min="1"
                    max="10000"
                    step="1"
                    on:change={normalizeFeeRate}
                />
                <span class="fee-rate-label mono">{feeRateSatPerByteLabel}</span>
            </div>
        </div>
    </div>

    <div class="utxo-stats-row">
        <span class="stat-item">
            <span class="stat-label">UTXOS:</span>
            <span class="stat-value mono">{filteredUtxos.length}</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">SAFE:</span>
            <span class="stat-value mono">{safeUtxoCount}</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">MAX/ROUND:</span>
            <span class="stat-value mono neon">{maxSafeInputs}</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">SELECTED:</span>
            <span class="stat-value mono neon">{selectedCount}</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">TOTAL:</span>
            <span class="stat-value mono neon">{formatAmount(totalSelected)} HEMP</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">EST TX:</span>
            <span class="stat-value mono">{estimatedSelectedBytes}B</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">EST FEE:</span>
            <span class="stat-value mono">{estimatedSelectedFee}</span>
        </span>
        {#if unsafeCount() > 0}
            <span class="stat-item warn">
                <span class="stat-label">UNSAFE:</span>
                <span class="stat-value mono">{unsafeCount()}</span>
            </span>
        {/if}
    </div>

    {#if exceedsOneRoundMax}
        <div class="warning-banner">
            <span>&#9888; Safe UTXO count ({safeUtxoCount}) exceeds one-round max ({maxSafeInputs}). Multi-round consolidation recommended.</span>
        </div>
    {/if}

    <div class="utxo-list">
        {#if loading}
            <div class="empty-state">Loading UTXOs...</div>
        {:else if filteredUtxos.length === 0}
            <div class="empty-state">
                {utxos.length === 0 ? "No UTXOs found. Wallet may be empty." : "No UTXOs match current filters."}
            </div>
        {:else}
            <div class="utxo-table-wrap">
                <table class="utxo-table">
                    <thead>
                        <tr>
                            <th></th>
                            <th>AMOUNT</th>
                            <th>ADDRESS</th>
                            <th>CONF</th>
                            <th>STATUS</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each filteredUtxos as u}
                            {@const id = `${u.txid}:${u.vout}`}
                            {@const unsafe = isUnsafe(u)}
                            <tr
                                class:selected={selectedIds.has(id)}
                                class:unsafe={unsafe}
                                on:click={() => toggleUtxo(id)}
                            >
                                <td>
                                    <div
                                        class="checkbox"
                                        class:checked={selectedIds.has(id)}
                                        class:disabled={unsafe}
                                    ></div>
                                </td>
                                <td class="amount-cell">{u.amount.toFixed(8)}</td>
                                <td class="addr-cell">
                                    {#if u.address}
                                        {u.address}
                                    {:else if u.label}
                                        {u.label}
                                    {:else}
                                        <span class="dim">(no address)</span>
                                    {/if}
                                </td>
                                <td>{u.confirmations}</td>
                                <td class="status-cell">
                                    {#if u.spendable === false}
                                        <span class="badge bad">UNSPENDABLE</span>
                                    {:else if u.safe === false}
                                        <span class="badge warn-badge">UNSAFE</span>
                                    {:else if u.asset && u.asset !== "HEMP"}
                                        <span class="badge asset-badge">{u.asset}</span>
                                    {:else}
                                        <span class="badge ok">OK</span>
                                    {/if}
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        {/if}
    </div>

    <div class="action-row">
        <button
            class="cyber-btn primary"
            disabled={selectedCount === 0 || !destination.trim() || loading || previewing || broadcasting}
            on:click={handlePreview}
        >
            {broadcasting ? "BROADCASTING..." : previewing ? "PREVIEWING..." : "PREVIEW CONSOLIDATION"}
        </button>
        <button
            class="cyber-btn"
            disabled={safeUtxoCount < 2 || planning}
            on:click={handlePlanBatches}
        >
            {planning ? "PLANNING..." : "PLAN BATCHES (PREVIEW ONLY)"}
        </button>
        {#if status}
            <span class="status-text mono">{status}</span>
        {/if}
    </div>
</div>

{#if showPreviewModal}
    <div
        class="modal-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cons-preview-title"
        tabindex="0"
        on:click={cancelConsolidation}
        on:keydown={(e) => e.key === "Escape" && cancelConsolidation()}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="modal-box" role="document" on:click|stopPropagation>
            <div class="modal-header">
                <span class="header-icon">&#9888;</span>
                <h2 id="cons-preview-title">CONSOLIDATION REVIEW</h2>
            </div>

            <div class="modal-body">
                {#if previewData}
                    <div class="tx-detail">
                        <span class="label">OPERATION:</span>
                        <span class="value">WALLET CONSOLIDATION</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">UTXOS:</span>
                        <span class="value">{previewData.utxo_count} selected</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">INPUT TOTAL:</span>
                        <span class="value neon">{previewData.input_total} HEMP</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">ESTIMATED FEE:</span>
                        <span class="value">{previewData.fee_estimate} HEMP</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">ESTIMATED OUTPUT:</span>
                        <span class="value neon">{previewData.output_amount} HEMP</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">DESTINATION:</span>
                        <span class="value mono addr-trunc">{previewData.destination.substring(0, 20)}...{previewData.destination.substring(previewData.destination.length - 8)}</span>
                    </div>

                    {#if previewData.warnings && previewData.warnings.length > 0}
                        <div class="warnings-section">
                            <span class="warning-section-title">WARNINGS</span>
                            {#each previewData.warnings as warn}
                                <div class="warning-box">
                                    <span class="warning-text">&#9888; {warn}</span>
                                </div>
                            {/each}
                        </div>
                    {/if}

                    <div class="warning-box highlight">
                        <span class="warning-text">
                            <strong>THIS ACTION IS IRREVERSIBLE.</strong>
                            All selected UTXOs will be merged into one output.
                            This costs fees and may affect privacy by linking UTXOs.
                        </span>
                    </div>
                {/if}
            </div>

            <div class="modal-footer">
                <button class="btn-cancel" on:click={cancelConsolidation} disabled={broadcasting}>
                    [ CANCEL ]
                </button>
                <button class="btn-confirm" on:click={executeConsolidation} disabled={broadcasting}>
                    {broadcasting ? "[ BROADCASTING... ]" : "[ CONFIRM CONSOLIDATION ]"}
                </button>
            </div>
        </div>
    </div>
{/if}

{#if showPlanModal}
    <div
        class="modal-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cons-plan-title"
        tabindex="0"
        on:click={() => (showPlanModal = false)}
        on:keydown={(e) => e.key === "Escape" && (showPlanModal = false)}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="modal-box plan-modal" role="document" on:click|stopPropagation>
            <div class="modal-header">
                <span class="header-icon">&#9776;</span>
                <h2 id="cons-plan-title">CONSOLIDATION PLAN</h2>
            </div>

            <div class="modal-body">
                {#if planData}
                    <div class="warning-box highlight" style="margin-top: 0; margin-bottom: 0.8rem; background: rgba(255, 221, 0, 0.08); border-color: rgba(255, 221, 0, 0.25);">
                        <span class="warning-text" style="color: #ffdd00;">
                            This is a preview plan only. Rounds must be executed manually through the Preview &amp; Broadcast flow.
                        </span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">INITIAL UTXOS:</span>
                        <span class="value">{planData.initial_utxo_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">PROJECTED FINAL:</span>
                        <span class="value neon">{planData.projected_final_utxo_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">MAX INPUTS/ROUND:</span>
                        <span class="value">{planData.max_inputs_per_round}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">TARGET TX BYTES:</span>
                        <span class="value">{planData.target_max_tx_bytes}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">TOTAL EST FEE:</span>
                        <span class="value neon">{planData.total_estimated_fee} HEMP</span>
                    </div>

                    {#if planData.rounds && planData.rounds.length > 0}
                        <div class="plan-rounds-section">
                            <span class="section-header">ROUNDS ({planData.rounds.length})</span>
                            {#each planData.rounds as round}
                                <div class="plan-round">
                                    <span class="round-label">#{round.round_number}</span>
                                    <div class="round-details">
                                        <span class="round-stat">{round.input_count} inputs</span>
                                        <span class="round-stat">{round.input_total} HEMP</span>
                                        <span class="round-stat">{round.estimated_bytes}B</span>
                                        <span class="round-stat">{round.fee_estimate} HEMP fee</span>
                                        <span class="round-stat-out">{round.projected_output} HEMP out</span>
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {:else}
                        <div class="empty-state" style="padding:1rem;">No rounds planned. UTXO count is already at target or insufficient inputs.</div>
                    {/if}
                {/if}
            </div>

            <div class="modal-footer">
                <button class="btn-cancel" on:click={() => (showPlanModal = false)}>
                    [ CLOSE ]
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .consolidation-view {
        display: flex;
        flex-direction: column;
        height: 100%;
        gap: 0.5rem;
        overflow: hidden;
    }

    .consolidation-header-bar {
        display: flex;
        align-items: baseline;
        gap: 1rem;
        flex-shrink: 0;
    }

    .header-label {
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }

    .header-sub {
        font-size: 0.7rem;
        color: #666;
        font-family: var(--font-mono);
    }

    .destination-row {
        flex-shrink: 0;
    }

    .dest-group {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .dest-group label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1px;
        text-transform: uppercase;
    }

    .input-group {
        display: flex;
        gap: 4px;
    }

    .input-glass {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.4rem 0.6rem;
        font-size: 0.75rem;
        border-radius: 4px;
        outline: none;
        font-family: var(--font-mono);
    }

    .input-glass:focus {
        border-color: var(--color-primary);
    }

    .full-width {
        flex: 1;
    }

    .icon-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        font-size: 0.65rem;
        padding: 0 0.8rem;
        cursor: pointer;
        font-family: var(--font-mono);
        border-radius: 4px;
        letter-spacing: 1px;
        transition: all 0.2s;
        white-space: nowrap;
    }

    .icon-btn:hover {
        background: var(--color-primary);
        color: #000;
    }

    select.input-glass {
        cursor: pointer;
        appearance: none;
        -webkit-appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath fill='%2300ff41' d='M5 7L1 3h8z'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 0.5rem center;
        padding-right: 1.6rem;
    }

    select.input-glass option {
        background: #0a0a0a;
        color: #fff;
    }

    .filter-row {
        display: flex;
        gap: 0.8rem;
        align-items: flex-end;
        flex-shrink: 0;
        flex-wrap: wrap;
    }

    .filter-group {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .filter-group label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1px;
        text-transform: uppercase;
    }

    .actions-right {
        margin-left: auto;
        flex-direction: row;
        gap: 0.5rem;
        align-self: flex-end;
    }

    .fee-rate-row {
        display: flex;
        flex-shrink: 0;
        padding: 0.2rem 0;
    }

    .fee-rate-control {
        display: flex;
        gap: 0.3rem;
        align-items: center;
    }

    .fee-rate-input {
        width: 70px;
        text-align: center;
        padding: 0.3rem 0.4rem;
    }

    .fee-rate-label {
        font-size: 0.65rem;
        color: #ccc;
    }

    .preset {
        padding: 0.2rem 0.5rem;
        font-size: 0.6rem;
    }

    .preset.active {
        border-color: var(--color-primary);
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
    }

    .utxo-stats-row {
        display: flex;
        gap: 1.2rem;
        flex-shrink: 0;
        padding: 0.3rem 0;
        flex-wrap: wrap;
    }

    .stat-item {
        display: flex;
        gap: 0.3rem;
        align-items: baseline;
    }

    .stat-label {
        font-size: 0.6rem;
        color: #666;
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }

    .stat-value {
        font-size: 0.75rem;
        color: #ccc;
    }

    .neon {
        color: var(--color-primary) !important;
        text-shadow: 0 0 6px rgba(0, 255, 65, 0.3);
    }

    .stat-item.warn .stat-value {
        color: #ff6644 !important;
    }

    .utxo-list {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }

    .utxo-table-wrap {
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
    }

    .utxo-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.75rem;
    }

    .utxo-table th {
        text-align: left;
        padding: 0.4rem 0.5rem;
        color: #555;
        font-size: 0.6rem;
        letter-spacing: 1px;
        text-transform: uppercase;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        position: sticky;
        top: 0;
        z-index: 1;
    }

    .utxo-table td {
        padding: 0.35rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        color: #ccc;
        cursor: pointer;
        white-space: nowrap;
    }

    .utxo-table tr {
        transition: background 0.1s;
    }

    .utxo-table tr:hover {
        background: rgba(0, 255, 65, 0.03);
    }

    .utxo-table tr.selected {
        background: rgba(0, 255, 65, 0.08);
    }

    .utxo-table tr.selected td {
        color: #fff;
    }

    .utxo-table tr.unsafe {
        opacity: 0.6;
    }

    .amount-cell {
        color: var(--color-primary) !important;
        font-weight: 600;
    }

    .addr-cell {
        max-width: 300px;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .dim {
        color: #555;
        font-style: italic;
    }

    .status-cell {
        text-align: center;
    }

    .badge {
        font-size: 0.6rem;
        padding: 0.1rem 0.4rem;
        border-radius: 3px;
        font-family: var(--font-mono);
        letter-spacing: 0.5px;
        white-space: nowrap;
    }

    .badge.ok {
        background: rgba(0, 255, 65, 0.1);
        color: var(--color-primary);
    }

    .badge.bad {
        background: rgba(255, 68, 68, 0.15);
        color: #ff6644;
    }

    .badge.warn-badge {
        background: rgba(255, 170, 0, 0.15);
        color: #ffaa00;
    }

    .badge.asset-badge {
        background: rgba(0, 180, 255, 0.15);
        color: #00b4ff;
    }

    .checkbox {
        width: 14px;
        height: 14px;
        border: 1px solid #555;
        border-radius: 2px;
        cursor: pointer;
    }

    .checkbox.checked {
        background: var(--color-primary);
        border-color: var(--color-primary);
        box-shadow: 0 0 5px var(--color-primary);
    }

    .checkbox.disabled {
        opacity: 0.35;
        cursor: not-allowed;
        background: rgba(255, 68, 68, 0.08);
        border-color: rgba(255, 68, 68, 0.35);
    }

    .action-row {
        display: flex;
        align-items: center;
        gap: 1rem;
        flex-shrink: 0;
        padding-top: 0.3rem;
    }

    .status-text {
        font-size: 0.7rem;
        color: var(--color-primary);
        text-transform: uppercase;
        letter-spacing: 0.5px;
        flex: 1;
    }

    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.4rem 0.8rem;
        font-size: 0.7rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        white-space: nowrap;
        font-family: var(--font-mono);
        border-radius: 4px;
    }

    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
    }

    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .cyber-btn.ghost {
        border-color: rgba(255, 255, 255, 0.2);
        color: #aaa;
        background: transparent;
    }

    .cyber-btn.ghost:hover:not(:disabled) {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }

    .cyber-btn.primary {
        background: rgba(0, 255, 65, 0.1);
        padding: 0.5rem 1.2rem;
        font-size: 0.75rem;
    }

    .empty-state {
        text-align: center;
        color: #555;
        padding: 2rem;
        font-size: 0.85rem;
    }

    /* MODAL STYLES */
    .modal-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.85);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 300;
        padding: 1rem;
    }

    .modal-box {
        background: linear-gradient(180deg, #0a0a0a 0%, #121212 100%);
        border: 1px solid rgba(255, 221, 0, 0.4);
        border-radius: 8px;
        width: 500px;
        max-width: 90vw;
        max-height: 80vh;
        overflow-y: auto;
        box-shadow:
            0 0 40px rgba(255, 221, 0, 0.2),
            0 20px 60px rgba(0, 0, 0, 0.8);
    }

    .modal-header {
        background: rgba(255, 221, 0, 0.1);
        padding: 0.8rem 1.2rem;
        border-bottom: 1px solid rgba(255, 221, 0, 0.2);
        display: flex;
        align-items: center;
        gap: 0.6rem;
    }

    .modal-header h2 {
        margin: 0;
        font-size: 0.9rem;
        color: #ffdd00;
        letter-spacing: 2px;
        font-family: var(--font-mono);
    }

    .header-icon {
        font-size: 1.1rem;
        color: #ffdd00;
    }

    .modal-body {
        padding: 1rem 1.2rem;
    }

    .tx-detail {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem 0.6rem;
        background: rgba(0, 255, 65, 0.04);
        border-left: 3px solid var(--color-primary);
        margin-bottom: 0.5rem;
        border-radius: 0 4px 4px 0;
    }

    .tx-detail .label {
        color: #888;
        font-size: 0.7rem;
        letter-spacing: 0.5px;
        text-transform: uppercase;
        font-family: var(--font-mono);
    }

    .tx-detail .value {
        color: #fff;
        font-weight: 600;
        font-size: 0.8rem;
    }

    .tx-detail .value.neon {
        color: var(--color-primary);
        text-shadow: 0 0 8px rgba(0, 255, 65, 0.3);
    }

    .addr-trunc {
        font-size: 0.7rem !important;
    }

    .warnings-section {
        margin-top: 0.8rem;
    }

    .warning-section-title {
        display: block;
        color: #ffaa00;
        font-size: 0.65rem;
        font-weight: bold;
        letter-spacing: 1px;
        text-transform: uppercase;
        margin-bottom: 0.4rem;
    }

    .warning-box {
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.2);
        border-radius: 4px;
        padding: 0.5rem 0.6rem;
        margin-top: 0.4rem;
    }

    .warning-box.highlight {
        background: rgba(255, 68, 68, 0.12);
        border-color: rgba(255, 68, 68, 0.4);
        margin-top: 0.6rem;
    }

    .warning-text {
        color: #ff9999;
        font-size: 0.7rem;
        line-height: 1.3;
        display: block;
    }

    .warning-text strong {
        color: #ff4444;
    }

    .modal-footer {
        display: flex;
        gap: 0.8rem;
        padding: 0.8rem 1.2rem;
        background: rgba(0, 0, 0, 0.3);
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }

    .btn-cancel,
    .btn-confirm {
        flex: 1;
        padding: 0.6rem;
        font-size: 0.75rem;
        font-weight: bold;
        letter-spacing: 1px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
        font-family: var(--font-mono);
    }

    .btn-cancel {
        background: transparent;
        border: 1px solid #666;
        color: #888;
    }

    .btn-cancel:hover:not(:disabled) {
        border-color: #ff4444;
        color: #ff4444;
    }

    .btn-cancel:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-confirm {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
    }

    .btn-confirm:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
    }

    .btn-confirm:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .warning-banner {
        background: rgba(255, 170, 0, 0.12);
        border: 1px solid rgba(255, 170, 0, 0.3);
        border-radius: 4px;
        padding: 0.4rem 0.6rem;
        font-size: 0.65rem;
        color: #ffaa00;
        flex-shrink: 0;
        font-family: var(--font-mono);
        letter-spacing: 0.3px;
    }

    .plan-modal {
        width: 620px;
        max-width: 90vw;
    }

    .plan-rounds-section {
        margin-top: 0.8rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
        padding-top: 0.8rem;
    }

    .section-header {
        display: block;
        color: var(--color-primary);
        font-size: 0.7rem;
        font-weight: bold;
        letter-spacing: 1px;
        text-transform: uppercase;
        margin-bottom: 0.5rem;
    }

    .plan-round {
        background: rgba(0, 255, 65, 0.04);
        border-left: 3px solid var(--color-primary);
        border-radius: 0 4px 4px 0;
        padding: 0.4rem 0.6rem;
        margin-bottom: 0.4rem;
        display: flex;
        align-items: center;
        gap: 0.6rem;
    }

    .round-label {
        color: var(--color-primary);
        font-weight: bold;
        font-size: 0.75rem;
        font-family: var(--font-mono);
        min-width: 30px;
    }

    .round-details {
        display: flex;
        flex-wrap: wrap;
        gap: 0.4rem 0.8rem;
    }

    .round-stat {
        color: #ccc;
        font-size: 0.65rem;
        font-family: var(--font-mono);
    }

    .round-stat-out {
        color: var(--color-primary);
        font-size: 0.65rem;
        font-family: var(--font-mono);
        font-weight: bold;
    }
</style>
