<script>
    import { onMount, tick } from "svelte";
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { formatAmount } from "../utils.js";
    import { systemStatus } from "../../stores.js";
    import { addTransactionNotification, addToolNotification } from "../stores/notifications.js";
    import WalletAddressPicker from "../ui/WalletAddressPicker.svelte";
    import HelpHitbox from "../ui/HelpHitbox.svelte";
    import CommanderLoader from "../ui/CommanderLoader.svelte";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    let utxos = [];
    let selectedIds = new Set();
    let totalSelected = 0;
    let estimatedSelectedBytes = 0;
    let estimatedSelectedFee = "0.00000000";
    let minConfirmations = 1;
    let minAmount = 0.0;
    let maxAmount = "";
    let maxRoundsToRun = "";
    let sourceAddressFilter = "__all__";
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
    let showUnlockModal = false;
    let unlockPassword = "";
    let unlockError = "";
    let unlocking = false;
    let unlockAfterAction = "single";
    let planData = null;
    let planning = false;
    let multiRoundRunning = false;
    let stopAfterCurrentRound = false;
    let executionLog = [];
    let activeRun = null;
    let displayManualSortVersion = 0;
    let lastAppliedSortVersion = -1;
    let consolidationPhase = "";
    const RUN_STORAGE_KEY = "commander.consolidation.run.v1";
    const CONFIRM_POLL_MS = 15000;
    const CLEAR_BUSY_THRESHOLD = 1500;
    const VIRTUAL_ROW_HEIGHT_PX = 33;
    const VIRTUAL_OVERSCAN_ROWS = 14;
    const VIRTUAL_HEADER_HEIGHT_PX = 30;
    let selectionBusy = false;
    let selectionStatus = "";
    let selectionProgressCurrent = 0;
    let selectionProgressTotal = 0;
    let selectionMode = "manual";
    let previewFeeRateSatPerByte = 1000;
    let walletAddresses = [];
    let lastSourceAddressFilter = "__all__";
    let utxoListEl;
    let utxoListScrollTop = 0;
    let utxoListViewportHeight = 500;

    $: safeUtxoCount = (() => {
        let count = 0;
        for (const u of filteredUtxos) {
            if (!isUnsafe(u)) count++;
        }
        return count;
    })();

    $: maxSafeInputs = policyDiag?.max_safe_inputs_for_one_output || 607;
    $: exceedsOneRoundMax = safeUtxoCount > maxSafeInputs;
    $: selectedExceedsOneRound = selectedCount > maxSafeInputs;
    $: selectedEstimatedRounds = estimateRoundsForCount(selectedCount, 80, maxSafeInputs);
    $: plannedRoundLimit = Number(maxRoundsToRun || 0);

    $: filteredUtxos = (() => {
        const maxAmountValue = Number(maxAmount || 0);
        return utxos.filter((u) => {
            if (sourceAddressFilter !== "__all__" && (u.address || "") !== sourceAddressFilter) return false;
            if (u.confirmations < minConfirmations) return false;
            if (u.amount < minAmount) return false;
            if (maxAmountValue > 0 && u.amount > maxAmountValue) return false;
            return true;
        });
    })();

    $: sourceAddressStats = (() => {
        const stats = new Map();
        let allTotal = 0;
        let allSafe = 0;
        let allCount = 0;
        for (const u of utxos) {
            const addr = u.address || "";
            const current = stats.get(addr) || { address: addr, total: 0, safeCount: 0, utxoCount: 0 };
            current.total += Number(u.amount || 0);
            current.utxoCount += 1;
            if (!isUnsafe(u)) current.safeCount += 1;
            stats.set(addr, current);
            allTotal += Number(u.amount || 0);
            allCount += 1;
            if (!isUnsafe(u)) allSafe += 1;
        }
        const byAddress = Array.from(stats.values()).sort((a, b) => b.total - a.total);
        return {
            all: { total: allTotal, safeCount: allSafe, utxoCount: allCount },
            byAddress,
        };
    })();

    $: sourceAddressEntries = (() => {
        const byAddr = new Map((walletAddresses || []).map((w) => [w.address, w]));
        return sourceAddressStats.byAddress
            .filter((s) => s.address && s.address.trim())
            .map((s) => ({
                ...s,
                label: byAddr.get(s.address)?.label || "",
                walletBalance: byAddr.get(s.address)?.balance || "",
            }));
    })();

    $: effectiveSelectedIds = (() => {
        if (selectionMode === "all-safe-filtered") {
            const ids = new Set();
            for (const u of filteredUtxos) {
                if (!isUnsafe(u)) ids.add(`${u.txid}:${u.vout}`);
            }
            return ids;
        }
        return selectedIds;
    })();

    $: effectiveSelectedUtxos = (() => {
        const out = [];
        for (const u of filteredUtxos) {
            if (effectiveSelectedIds.has(`${u.txid}:${u.vout}`)) out.push(u);
        }
        return out;
    })();

    $: selectedCount = effectiveSelectedUtxos.length;

    $: totalSelected = effectiveSelectedUtxos.reduce((sum, u) => sum + Number(u.amount || 0), 0);

    $: estimatedSelectedBytes = 10 + effectiveSelectedUtxos.length * 148 + 1 * 34;

    $: estimatedSelectedFee = (() => {
        const diagRate = Number(policyDiag?.fee_rate_sat_per_byte || previewFeeRateSatPerByte || 1000);
        return ((estimatedSelectedBytes * diagRate) / 100000000).toFixed(8);
    })();

    $: displayUtxos = (() => {
        void displayManualSortVersion;
        if (selectionMode === "all-safe-filtered") {
            const eligible = [];
            const excluded = [];
            for (const u of filteredUtxos) {
                if (!isUnsafe(u)) eligible.push(u);
                else excluded.push(u);
            }
            return [...eligible, ...excluded];
        }
        if (effectiveSelectedIds.size === 0) return filteredUtxos;
        if (displayManualSortVersion === lastAppliedSortVersion) {
            const eligible = [];
            const excluded = [];
            for (const u of filteredUtxos) {
                if (!isUnsafe(u)) eligible.push(u);
                else excluded.push(u);
            }
            return [...eligible, ...excluded];
        }
        lastAppliedSortVersion = displayManualSortVersion;
        const selectedArr = [];
        const unselected = [];
        for (const u of filteredUtxos) {
            const id = `${u.txid}:${u.vout}`;
            if (effectiveSelectedIds.has(id)) selectedArr.push(u);
            else unselected.push(u);
        }
        selectedArr.sort((a, b) => (Number(a.amount || 0) - Number(b.amount || 0)) || String(a.txid || "").localeCompare(String(b.txid || "")) || (Number(a.vout || 0) - Number(b.vout || 0)));
        unselected.sort((a, b) => (Number(a.amount || 0) - Number(b.amount || 0)) || String(a.txid || "").localeCompare(String(b.txid || "")) || (Number(a.vout || 0) - Number(b.vout || 0)));
        return [...selectedArr, ...unselected];
    })();

    $: pruneSelectionToFiltered(filteredUtxos);
    $: virtualBodyHeight = Math.max(0, utxoListViewportHeight - VIRTUAL_HEADER_HEIGHT_PX);
    $: visibleRowWindow = Math.max(1, Math.ceil(virtualBodyHeight / VIRTUAL_ROW_HEIGHT_PX) + VIRTUAL_OVERSCAN_ROWS * 2);
    $: scrollRows = Math.max(0, Math.floor(Math.max(0, utxoListScrollTop - VIRTUAL_HEADER_HEIGHT_PX) / VIRTUAL_ROW_HEIGHT_PX) - VIRTUAL_OVERSCAN_ROWS);
    $: maxVirtualStart = Math.max(0, displayUtxos.length - visibleRowWindow);
    $: virtualStartIndex = Math.min(scrollRows, maxVirtualStart);
    $: virtualEndIndex = Math.min(displayUtxos.length, virtualStartIndex + visibleRowWindow);
    $: virtualTopPadding = virtualStartIndex * VIRTUAL_ROW_HEIGHT_PX;
    $: virtualBottomPadding = (displayUtxos.length - virtualEndIndex) * VIRTUAL_ROW_HEIGHT_PX;
    $: visibleUtxos = displayUtxos.slice(virtualStartIndex, virtualEndIndex);

    function pruneSelectionToFiltered(visibleUtxos) {
        if (selectionMode === "all-safe-filtered") return;
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
            if (sourceAddressFilter !== lastSourceAddressFilter) {
                status = "Selection updated for the current source address.";
            }
        }
    }

    $: {
        if (sourceAddressFilter !== lastSourceAddressFilter) {
            lastSourceAddressFilter = sourceAddressFilter;
        }
    }

    function estimateRoundsForCount(count, targetFinal, maxInputsPerRound) {
        if (!Number.isFinite(count) || count <= targetFinal || maxInputsPerRound < 2) return 0;
        let rounds = 0;
        let current = count;
        const maxReduction = maxInputsPerRound - 1;
        while (current > targetFinal) {
            const reduction = Math.min(maxReduction, current - targetFinal);
            current -= reduction;
            rounds += 1;
        }
        return rounds;
    }

    function clearZeroOptionalFilter(name) {
        if (name === "maxAmount" && Number(maxAmount || 0) <= 0) {
            maxAmount = "";
        }
        if (name === "maxRoundsToRun" && Number(maxRoundsToRun || 0) <= 0) {
            maxRoundsToRun = "";
        }
    }

    function selectedSafeInputs() {
        const out = [];
        for (const u of filteredUtxos) {
            const id = `${u.txid}:${u.vout}`;
            if (effectiveSelectedIds.has(id) && !isUnsafe(u)) {
                out.push({ txid: u.txid, vout: u.vout });
            }
        }
        return out;
    }

    function isWalletUnlockError(err) {
        const errText = String(err || "");
        const lowerErr = errText.toLowerCase();
        return errText.includes("ERROR CODE: -13")
            || lowerErr.includes("walletpassphrase")
            || lowerErr.includes("wallet passphrase")
            || lowerErr.includes("please enter the wallet passphrase");
    }

    function requestWalletUnlock(action = "single") {
        unlockAfterAction = action;
        status = action === "multi"
            ? "Wallet unlock required before starting multi-round consolidation."
            : "Wallet unlock required before broadcasting consolidation.";
        unlockError = "";
        unlockPassword = "";
        showUnlockModal = true;
        if (action === "single") {
            showPreviewModal = true;
        }
    }

    function toggleUtxo(id) {
        const utxo = utxos.find((u) => `${u.txid}:${u.vout}` === id);
        if (utxo && isUnsafe(utxo)) {
            status = "Unsafe, unspendable, or asset-bearing UTXOs are excluded from HEMP consolidation.";
            return;
        }
        const next = selectionMode === "all-safe-filtered"
            ? new Set(effectiveSelectedIds)
            : new Set(selectedIds);
        selectionMode = "manual";
        if (next.has(id)) next.delete(id);
        else next.add(id);
        selectedIds = next;
    }

    async function showSelectionBusy(label, detail = "") {
        selectionBusy = true;
        selectionStatus = detail || label;
        selectionProgressCurrent = 0;
        selectionProgressTotal = 0;
        await tick();
        await new Promise((resolve) => requestAnimationFrame(resolve));
    }

    async function selectAllSafe() {
        if (selectionBusy) return;
        try {
            await showSelectionBusy("Selecting UTXOs", "Applying current filters...");
            displayManualSortVersion++;
            selectedIds = new Set();
            selectionMode = "all-safe-filtered";
            planData = null;
            showPlanModal = false;
            await tick();
            const estimatedRounds = estimateRoundsForCount(selectedCount, 80, maxSafeInputs);
            if (selectedCount === 0) {
                status = "No HEMP UTXOs match the current filters.";
                selectionMode = "manual";
                return;
            }
            if (selectedCount > maxSafeInputs) {
                status = "";
                selectionStatus = `Preparing multi-round plan button... ${estimatedRounds.toLocaleString()} rounds`;
            } else {
                status = "Selected all UTXOs matching the current filters.";
            }
            await tick();
            await new Promise((resolve) => requestAnimationFrame(resolve));
        } finally {
            selectionBusy = false;
            selectionStatus = "";
            selectionProgressCurrent = 0;
            selectionProgressTotal = 0;
        }
    }

    async function selectOneSafeBatch() {
        if (selectionBusy) return;
        await clearSelected();
        displayManualSortVersion++;
        selectionMode = "manual";
        const safe = filteredUtxos
            .filter((u) => !isUnsafe(u))
            .sort((a, b) => a.amount - b.amount);
        if (safe.length === 0) {
            status = "No HEMP UTXOs match the current filters.";
            return;
        }
        const maxInputs = policyDiag?.max_safe_inputs_for_one_output || 607;
        const limit = Math.min(safe.length, maxInputs);
        const next = new Set();
        let total = 0;
        for (let i = 0; i < limit; i++) {
            next.add(`${safe[i].txid}:${safe[i].vout}`);
            total += Number(safe[i].amount || 0);
        }
        selectedIds = next;
        status = `Selected one round: ${limit.toLocaleString()} UTXOs, ${formatAmount(total)} HEMP.`;
    }

    async function handlePlanBatches() {
        if (!tauriReady || planning || selectionBusy) return;
        if (!selectedExceedsOneRound) {
            status = "Plan Multi-Round uses the current selected set and requires more than one-round max inputs.";
            return;
        }
        planning = true;
        planData = null;
        status = "Building consolidation plan...";
        try {
            planData = await core.invoke("plan_wallet_consolidation", {
                destination: destination.trim() || null,
                targetFinalUtxoCount: null,
                maxRounds: plannedRoundLimit > 0 ? plannedRoundLimit : null,
                targetMaxTxBytes: null,
                feeRateSatPerByte: null,
                selectedOutpoints: selectedSafeInputs(),
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

    async function clearSelected() {
        if (selectionBusy) return;
        const hadSelectionCount = selectedIds.size;
        const shouldShowBusy = hadSelectionCount >= CLEAR_BUSY_THRESHOLD;
        if (shouldShowBusy) {
            selectionBusy = true;
            selectionStatus = "Clearing selection...";
            selectionProgressCurrent = 0;
            selectionProgressTotal = 0;
            await tick();
            await new Promise((resolve) => requestAnimationFrame(resolve));
        }
        displayManualSortVersion++;
        selectedIds = new Set();
        selectionMode = "manual";
        planData = null;
        showPlanModal = false;
        if (shouldShowBusy) {
            await tick();
            await new Promise((resolve) => requestAnimationFrame(resolve));
            selectionBusy = false;
            selectionStatus = "";
        }
    }

    function handleUtxoListScroll() {
        if (!utxoListEl) return;
        utxoListScrollTop = utxoListEl.scrollTop;
    }

    function syncUtxoViewportMetrics() {
        if (!utxoListEl) return;
        utxoListViewportHeight = utxoListEl.clientHeight || 500;
        const maxScroll = Math.max(0, utxoListEl.scrollHeight - utxoListViewportHeight);
        if (utxoListScrollTop > maxScroll) {
            utxoListScrollTop = maxScroll;
            utxoListEl.scrollTop = maxScroll;
        }
    }

    function persistRunState() {
        if (typeof localStorage === "undefined") return;
        if (!activeRun) {
            localStorage.removeItem(RUN_STORAGE_KEY);
            return;
        }
        localStorage.setItem(RUN_STORAGE_KEY, JSON.stringify(activeRun));
    }

    function loadPersistedRunState() {
        if (typeof localStorage === "undefined") return;
        try {
            const raw = localStorage.getItem(RUN_STORAGE_KEY);
            if (!raw) return;
            const parsed = JSON.parse(raw);
            if (parsed && parsed.status && parsed.destination) {
                activeRun = parsed;
                executionLog = parsed.rounds || [];
            }
        } catch {
            activeRun = null;
        }
    }

    function clearRunState() {
        activeRun = null;
        executionLog = [];
        multiRoundRunning = false;
        stopAfterCurrentRound = false;
        consolidationPhase = "";
        status = "Run cleared. Create a new plan when ready.";
        persistRunState();
    }

    function setPhase(phase) {
        consolidationPhase = phase;
        status = phase;
    }

    async function completeRun(message) {
        activeRun = null;
        executionLog = [];
        multiRoundRunning = false;
        stopAfterCurrentRound = false;
        planData = null;
        showPlanModal = false;
        selectedIds = new Set();
        selectionMode = "manual";
        displayManualSortVersion++;
        consolidationPhase = "";
        status = message;
        persistRunState();
        await fetchUtxos(false);
        status = message;
    }

    async function broadcastConsolidationRound(opts) {
        const { inputs, roundLabel, plan, journalEntryId, runData } = opts;
        const roundDestination = String(plan?.destination || runData?.destination || destination || "").trim();
        if (!roundDestination) throw new Error("Destination is required.");
        setPhase(`Previewing round${roundLabel ? ` ${roundLabel}` : ""}...`);
        await tick();
        let preview = plan;
        if (!preview) {
            preview = await core.invoke("preview_wallet_consolidation", {
                utxos: inputs, destination: roundDestination, feeRateSatPerByte: plan?.fee_rate_sat_per_byte || null,
            });
        }
        let journalId = journalEntryId;
        if (!journalId && !runData) {
            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: { status: "Previewed", operation_type: "wallet_consolidation_round", summary: `Consolidation round${roundLabel ? ` ${roundLabel}` : ""}`, txid: null, details: { utxo_count: preview.utxo_count, input_total: preview.input_total, output_amount: preview.output_amount, fee_estimate: preview.fee_estimate, fee_rate_sat_per_byte: preview.fee_rate_sat_per_byte, destination: preview.destination, warnings: preview.warnings } },
                });
                journalId = entry.id;
            } catch {}
        }
        setPhase(`Broadcasting round${roundLabel ? ` ${roundLabel}` : ""}...`);
        await tick();
        const feeValue = parseFloat(preview.fee_estimate || "0.01");
        const txid = await core.invoke("broadcast_wallet_consolidation", {
            utxos: inputs, destination: roundDestination, fee: feeValue, feeRateSatPerByte: preview.fee_rate_sat_per_byte || 1000,
        });
        addTransactionNotification("Consolidation round broadcasted", `${preview.utxo_count} UTXOs merged`, "success", txid);
        if (journalId) {
            try { await core.invoke("update_tx_journal_entry", { id: journalId, status: "Broadcasted", txid, details: null }); } catch {}
        }
        if (runData) {
            const currentRound = (runData.rounds?.length || 0) + 1;
            const entry = { round_number: currentRound, input_count: preview.utxo_count, fee_estimate: preview.fee_estimate, txid, status: "broadcasted", confirmed: false, updated_at: new Date().toISOString() };
            runData.rounds = [...(runData.rounds || []), entry];
            const consumed = new Set(inputs.map((u) => `${u.txid}:${u.vout}`));
            runData.remaining_selected_outpoints = (runData.remaining_selected_outpoints || []).filter((op) => !consumed.has(op));
            executionLog = runData.rounds;
            activeRun = runData;
            persistRunState();
            try {
                const je = await core.invoke("add_tx_journal_entry", { input: { status: "Broadcasted", operation_type: "wallet_consolidation_round", summary: `Consolidation round ${currentRound}${roundLabel ? ` ${roundLabel}` : ""}`, txid, details: entry } });
                journalId = je.id;
            } catch {}
        }
        return { txid, preview, journalId };
    }

    async function confirmRound(txid, roundLabel) {
        setPhase(`Waiting for confirmation${roundLabel ? ` on round ${roundLabel}` : ""}...`);
        while (true) {
            if (!multiRoundRunning && !broadcasting) return false;
            try {
                const history = await core.invoke("get_transaction_history", { count: 500, skip: 0, category: null });
                const match = (history?.items || []).find((item) => item.txid === txid);
                if (match && Number(match.confirmations || 0) > 0) return true;
            } catch {}
            await new Promise((r) => setTimeout(r, CONFIRM_POLL_MS));
        }
    }

    function collectInputsFromOutpoints(outpoints) {
        const lookup = new Map();
        for (const u of utxos) lookup.set(`${u.txid}:${u.vout}`, u);
        const inputs = [];
        for (const op of outpoints || []) {
            const hit = lookup.get(op);
            if (!hit || isUnsafe(hit)) continue;
            inputs.push({ txid: hit.txid, vout: hit.vout });
        }
        return inputs;
    }

    async function runMultiRound() {
        if (!activeRun) return;
        multiRoundRunning = true;
        stopAfterCurrentRound = false;
        activeRun.status = "running";
        activeRun.updated_at = new Date().toISOString();
        persistRunState();
        let plannedRounds = activeRun.planned_round_count || activeRun.estimated_round_count || 1;
        try {
            while (multiRoundRunning) {
                if (activeRun.max_rounds_to_run && (activeRun.rounds?.length || 0) >= activeRun.max_rounds_to_run) {
                    const completedRounds = activeRun.max_rounds_to_run;
                    await completeRun(`Consolidation run complete. Completed ${completedRounds} planned round${completedRounds === 1 ? "" : "s"}. Create a new plan to continue.`);
                    break;
                }
                if ((activeRun.remaining_selected_outpoints || []).length < 2) {
                    await completeRun("Consolidation run complete. Create a new plan if you want to consolidate more UTXOs.");
                    break;
                }
                setPhase("Refreshing UTXOs for next round...");
                await tick();
                await fetchUtxos(false);
                const nextPlan = await core.invoke("plan_wallet_consolidation", {
                    destination: activeRun.destination,
                    targetFinalUtxoCount: activeRun.target_final_utxo_count || 80,
                    maxRounds: 1,
                    targetMaxTxBytes: null,
                    feeRateSatPerByte: null,
                    selectedOutpoints: (activeRun.remaining_selected_outpoints || []).map((op) => {
                        const [txid, vout] = op.split(":");
                        return { txid, vout: Number(vout) };
                    }),
                });
                if (!nextPlan?.rounds?.length) {
                    addToolNotification("Consolidation run completed", "All planned rounds finished", "success");
                    await completeRun("Consolidation run complete. Create a new plan if you want to consolidate more UTXOs.");
                    break;
                }

                plannedRounds = activeRun.planned_round_count || Math.max(plannedRounds, nextPlan.estimated_round_count || 1);
                const currentRound = (activeRun.rounds?.length || 0) + 1;
                const nextRoundPlan = nextPlan.rounds[0];
                const roundInputs = collectInputsFromOutpoints(nextRoundPlan.selected_outpoints);
                if (roundInputs.length < 2) {
                    activeRun.status = "stopped";
                    activeRun.last_error = "Not enough selected safe outpoints remain to build another round.";
                    persistRunState();
                    consolidationPhase = "";
                    status = "Selected-scope run paused: wait for confirmations and resume if needed.";
                    break;
                }

                const { txid } = await broadcastConsolidationRound({
                    inputs: roundInputs,
                    roundLabel: `${currentRound}/${plannedRounds}`,
                    plan: { fee_estimate: nextRoundPlan.fee_estimate, fee_rate_sat_per_byte: nextRoundPlan.fee_rate_sat_per_byte || previewFeeRateSatPerByte, utxo_count: roundInputs.length, input_total: nextRoundPlan.input_total, output_amount: nextRoundPlan.projected_output, destination: activeRun.destination, warnings: [] },
                    runData: activeRun,
                });

                const confirmed = await confirmRound(txid, `${currentRound}/${plannedRounds}`);
                if (!confirmed) break;
                const entry = activeRun.rounds?.find((r) => r.txid === txid);
                if (entry) { entry.status = "confirmed"; entry.confirmed = true; entry.updated_at = new Date().toISOString(); }
                activeRun.updated_at = new Date().toISOString();
                executionLog = [...activeRun.rounds];
                persistRunState();
                setPhase(`Round ${currentRound}/${plannedRounds} confirmed. Preparing next round...`);
                await core.invoke("add_tx_journal_entry", { input: { status: "Confirmed", operation_type: "wallet_consolidation_round", summary: `Consolidation round ${currentRound} confirmed`, txid, details: { round_number: currentRound } } });

                if (stopAfterCurrentRound) {
                    activeRun.status = "stopped";
                    activeRun.last_error = null;
                    persistRunState();
                    consolidationPhase = "";
                    status = `Stopped after round ${currentRound}. Resume when ready.`;
                    break;
                }
            }
        } finally {
            multiRoundRunning = false;
            consolidationPhase = "";
        }
    }

    async function startMultiRound() {
        if (!planData || !destination.trim()) return;
        activeRun = {
            status: "planned",
            destination: destination.trim(),
            target_final_utxo_count: planData.target_final_utxo_count || 80,
            estimated_round_count: planData.estimated_round_count || planData.rounds?.length || 1,
            planned_round_count: planData.planned_round_count || planData.rounds?.length || planData.estimated_round_count || 1,
            max_rounds_to_run: plannedRoundLimit > 0 ? plannedRoundLimit : null,
            max_inputs_per_round: planData.max_inputs_per_round,
            original_selected_outpoints: selectedSafeInputs().map((u) => `${u.txid}:${u.vout}`),
            remaining_selected_outpoints: selectedSafeInputs().map((u) => `${u.txid}:${u.vout}`),
            rounds: [],
            last_error: null,
            started_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
        };
        executionLog = [];
        persistRunState();
        showPlanModal = false;
        await core.invoke("add_tx_journal_entry", {
            input: {
                status: "Previewed",
                operation_type: "wallet_consolidation_multi_round",
                summary: `Planned multi-round consolidation (${activeRun.planned_round_count} planned of ${activeRun.estimated_round_count} est rounds)`,
                txid: null,
                details: activeRun,
            },
        });
        try {
            await runMultiRound();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                consolidationPhase = "";
                activeRun.status = "planned";
                activeRun.last_error = null;
                activeRun.updated_at = new Date().toISOString();
                persistRunState();
                requestWalletUnlock("multi");
                return;
            }
            activeRun.status = "failed";
            activeRun.last_error = String(err);
            activeRun.updated_at = new Date().toISOString();
            persistRunState();
            consolidationPhase = "";
            status = `Multi-round failed: ${err}`;
        }
    }

    async function resumeRun() {
        if (!activeRun) return;
        try {
            await runMultiRound();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                consolidationPhase = "";
                activeRun.status = "stopped";
                activeRun.last_error = null;
                activeRun.updated_at = new Date().toISOString();
                persistRunState();
                requestWalletUnlock("multi");
                return;
            }
            activeRun.status = "failed";
            activeRun.last_error = String(err);
            activeRun.updated_at = new Date().toISOString();
            persistRunState();
            consolidationPhase = "";
            status = `Resume failed: ${err}`;
        }
    }

    function requestStopAfterCurrentRound() {
        stopAfterCurrentRound = true;
        status = "Will stop after current round confirmation.";
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
            if (effectiveSelectedIds.has(`${u.txid}:${u.vout}`) && isUnsafe(u)) count++;
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
                feeRateSatPerByte: null,
            });
            await loadWalletAddresses();
        } catch (err) {
            showToast("Failed to list UTXOs: " + err, "error");
        }
        loading = false;

        if (refreshAddr || !destination) {
            await refreshDestination();
        }
    }

    async function refreshDestination(label = null) {
        if (!tauriReady) return;
        try {
            const addr = await core.invoke("new_address", { label });
            destination = addr;
            await loadWalletAddresses();
        } catch (err) {
            showToast("Failed to get new address: " + err, "error");
        }
    }

    async function loadWalletAddresses() {
        try {
            walletAddresses = await core.invoke("get_receive_addresses", { showChange: false });
            if ((!destination || !destination.trim()) && walletAddresses.length > 0) {
                destination = walletAddresses[0].address;
            }
        } catch {
            walletAddresses = [];
        }
    }

    async function handleAddressGenerate(event) {
        await refreshDestination(event?.detail?.label || null);
    }

    async function handlePreview() {
        if (!tauriReady) return;
        if (previewing || broadcasting || selectionBusy) return;
        if (selectedCount === 0) {
            status = "Select at least one UTXO to consolidate.";
            return;
        }
        if (!destination.trim()) {
            status = "Consolidation destination address is required.";
            return;
        }
        if (selectedExceedsOneRound) {
            status = `Selected ${selectedCount} inputs exceeds the one-round limit of ${maxSafeInputs}. Use PLAN MULTI-ROUND for this selected set.`;
            return;
        }

        setPhase("Building consolidation preview...");
        previewing = true;
        previewData = null;
        previewJournalId = null;

        const inputs = selectedSafeInputs();

        try {
            previewData = await core.invoke("preview_wallet_consolidation", {
                utxos: inputs,
                destination: destination.trim(),
                feeRateSatPerByte: null,
            });
            previewFeeRateSatPerByte = Number(previewData?.fee_rate_sat_per_byte || 1000);
            status = "";
            consolidationPhase = "";

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
            consolidationPhase = "";
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
        if (!previewData) {
            status = "Preview expired. Build a fresh consolidation preview first.";
            return;
        }
        showPreviewModal = false;
        broadcasting = true;
        const inputs = (previewData.utxos || []).map((u) => ({ txid: u.txid, vout: u.vout }));
        try {
            await broadcastConsolidationRound({
                inputs,
                roundLabel: "1/1",
                plan: { fee_estimate: previewData.fee_estimate, fee_rate_sat_per_byte: previewFeeRateSatPerByte, utxo_count: previewData.utxo_count, input_total: previewData.input_total, output_amount: previewData.output_amount, destination: previewData.destination, warnings: previewData.warnings },
                journalEntryId: previewJournalId,
            });
            const completeMessage = "Consolidation complete. Create a new plan if you want to consolidate more UTXOs.";
            consolidationPhase = "";
            previewData = null;
            previewJournalId = null;
            selectedIds = new Set();
            selectionMode = "manual";
            displayManualSortVersion++;
            await fetchUtxos(true);
            status = completeMessage;
        } catch (err) {
            if (isWalletUnlockError(err)) {
                consolidationPhase = "";
                requestWalletUnlock("single");
                return;
            }
            status = "Consolidation failed: " + err;
            consolidationPhase = "";
            addToolNotification("Consolidation failed", String(err).substring(0, 200), "error");
            if (previewJournalId) {
                try { await core.invoke("update_tx_journal_entry", { id: previewJournalId, status: "Failed", txid: null, details: { error: String(err) } }); } catch {}
            }
        } finally {
            broadcasting = false;
        }
    }

    async function unlockAndBroadcastConsolidation() {
        if (!unlockPassword.trim() || unlocking) return;
        unlocking = true;
        unlockError = "";
        try {
            await core.invoke("wallet_unlock", { password: unlockPassword, duration: 300 });
            unlockPassword = "";
            showUnlockModal = false;
            if (unlockAfterAction === "multi") {
                showPreviewModal = false;
                if (activeRun) {
                    activeRun.status = "planned";
                    activeRun.last_error = null;
                    persistRunState();
                }
                await runMultiRound();
            } else {
                showPreviewModal = true;
                broadcasting = true;
                try {
                    await executeConsolidation();
                } finally {
                    broadcasting = false;
                }
            }
        } catch (err) {
            if (isWalletUnlockError(err)) {
                unlockError = "Wallet unlock failed. Check the passphrase and try again.";
                showUnlockModal = true;
            } else {
                if (unlockAfterAction === "multi" && activeRun) {
                    activeRun.status = "failed";
                    activeRun.last_error = String(err);
                    activeRun.updated_at = new Date().toISOString();
                    persistRunState();
                    status = `Multi-round failed after unlock: ${err}`;
                } else {
                    status = `Broadcast failed after unlock: ${err}`;
                }
            }
            console.warn("Wallet unlock failed:", err);
        } finally {
            unlocking = false;
        }
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
        loadPersistedRunState();
        fetchUtxos(true);
        let raf = null;
        const syncViewport = () => {
            if (raf !== null) cancelAnimationFrame(raf);
            raf = requestAnimationFrame(() => {
                syncUtxoViewportMetrics();
                raf = null;
            });
        };
        const resizeObserver = typeof ResizeObserver !== "undefined" ? new ResizeObserver(syncViewport) : null;
        if (resizeObserver && utxoListEl) resizeObserver.observe(utxoListEl);
        window.addEventListener("resize", syncViewport, { passive: true });
        tick().then(syncViewport);
        return () => {
            if (raf !== null) cancelAnimationFrame(raf);
            resizeObserver?.disconnect();
            window.removeEventListener("resize", syncViewport);
        };
    });
</script>

<div class="consolidation-view">
    <div class="consolidation-header-bar">
        <span class="header-label mono">WALLET CONSOLIDATION</span> <HelpHitbox title="Wallet Consolidation">
            <ul>
                <li><strong>What it does:</strong> merges small HEMP UTXOs into fewer outputs so future sends stay within policy size limits.</li>
                <li><strong>Privacy tradeoff:</strong> selected UTXOs are linked on-chain. Asset-bearing or unspendable UTXOs are excluded automatically.</li>
                <li><strong>Filters:</strong> Min confirmations limits to older, deeper-confirmed outputs. Min/Max HEMP filters by amount (useful for isolating mining dust).</li>
                <li><strong>Max rounds:</strong> leave at 0 to run the full plan, or set a small number to test in stages.</li>
                <li><strong>Modes:</strong> <em>Select One Round</em> prepares a single transaction. <em>Select All</em> can trigger a multi-round run if the set is large.</li>
                <li><strong>Fees:</strong> estimated before each broadcast based on current network conditions.</li>
            </ul>
        </HelpHitbox>
        <span class="header-sub">Merge eligible HEMP UTXOs with single-round or multi-round execution</span>
    </div>

    <div class="destination-row">
        <div class="dest-group">
            <WalletAddressPicker
                id="cons-dest"
                label="DESTINATION ADDRESS"
                bind:value={destination}
                addresses={walletAddresses}
                nodeOnline={tauriReady}
                placeholder="Wallet consolidation address..."
                on:generate={handleAddressGenerate}
            />
        </div>
    </div>

    <div class="filter-row">
        <div class="filter-group source-filter">
            <label for="cons-source-addr">SOURCE ADDRESS</label>
            <select id="cons-source-addr" class="input-glass" bind:value={sourceAddressFilter}>
                <option value="__all__">All wallet addresses  •  {formatAmount(sourceAddressStats.all.total)} HEMP  •  {sourceAddressStats.all.safeCount.toLocaleString()} safe / {sourceAddressStats.all.utxoCount.toLocaleString()} UTXOs</option>
                {#each sourceAddressEntries as row}
                    <option value={row.address}>
                        {(row.label && row.label.trim()) ? `${row.label} - ` : ""}{row.address.slice(0, 10)}...{row.address.slice(-6)}  •  {formatAmount(row.total)} HEMP  •  {row.safeCount.toLocaleString()} safe / {row.utxoCount.toLocaleString()} UTXOs
                    </option>
                {/each}
            </select>
        </div>
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
        <div class="filter-group amount-filter">
            <label for="cons-min-amount">MIN HEMP</label>
            <input
                type="number"
                id="cons-min-amount"
                class="input-glass"
                bind:value={minAmount}
                step="0.001"
                min="0"
            />
        </div>
        <div class="filter-group amount-filter">
            <label for="cons-max-amount">MAX HEMP</label>
            <input
                type="number"
                id="cons-max-amount"
                class="input-glass"
                bind:value={maxAmount}
                step="0.001"
                min="0"
                placeholder="ANY"
                on:blur={() => clearZeroOptionalFilter("maxAmount")}
            />
        </div>
        <div class="filter-group rounds-filter">
            <label for="cons-max-rounds">MAX ROUNDS</label>
            <input
                type="number"
                id="cons-max-rounds"
                class="input-glass"
                bind:value={maxRoundsToRun}
                step="1"
                min="0"
                placeholder="ALL"
                on:blur={() => clearZeroOptionalFilter("maxRoundsToRun")}
            />
        </div>
        <div class="filter-group actions-right">
            <button class="cyber-btn ghost" on:click={selectAllSafe} disabled={selectionBusy || loading}>SELECT ALL</button>
            <button class="cyber-btn ghost" on:click={selectOneSafeBatch} disabled={selectionBusy || loading}>SELECT ONE ROUND</button>
            <button class="cyber-btn ghost" on:click={clearSelected} disabled={selectionBusy}>CLEAR</button>
            <button class="cyber-btn" on:click={() => fetchUtxos(false)} disabled={loading || selectionBusy}>
                {loading ? "LOADING..." : "REFRESH"}
            </button>
        </div>
    </div>

    {#if selectionBusy}
        <CommanderLoader
            compact={true}
            label={selectionStatus.startsWith("Clearing") ? "Clearing selection" : "Selecting UTXOs"}
            detail={selectionStatus.startsWith("Clearing")
                ? "Resetting selected UTXO set..."
                : selectionProgressTotal > 0
                    ? `${selectionProgressCurrent.toLocaleString()} / ${selectionProgressTotal.toLocaleString()}`
                    : "Preparing..."}
        />
    {/if}

    {#if consolidationPhase}
        <CommanderLoader compact={true} label={consolidationPhase} detail={previewing || broadcasting || multiRoundRunning ? "Do not close Commander" : ""} />
    {/if}

    <div class="utxo-stats-row">
        <span class="stat-item">
            <span class="stat-label">UTXOS:</span>
            <span class="stat-value mono">{filteredUtxos.length}</span>
        </span>
        <span class="stat-item">
            <span class="stat-label">ELIGIBLE:</span>
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
            <span class="stat-label">AUTO FEE:</span>
            <span class="stat-value mono">{policyDiag?.fee_rate_sat_per_byte || previewFeeRateSatPerByte} sat/B</span>
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

    {#if selectedCount === 0 && exceedsOneRoundMax}
        <div class="warning-banner">
            <span>This wallet has more eligible UTXOs than one transaction can merge. Use SELECT ALL for a full multi-round run or SELECT ONE ROUND for one transaction-sized group.</span>
        </div>
    {/if}
    {#if selectedExceedsOneRound}
        <div class="info-banner">
            <span>Commander will use multi-round consolidation for the selected UTXOs: about {selectedEstimatedRounds} rounds at up to {maxSafeInputs} inputs per round{plannedRoundLimit > 0 ? `, capped at ${plannedRoundLimit} this run` : ""}.</span>
        </div>
    {/if}

    <div class="action-row">
        <button
            class="cyber-btn primary"
            disabled={selectedCount === 0 || selectedExceedsOneRound || !destination.trim() || loading || selectionBusy || previewing || broadcasting}
            on:click={handlePreview}
        >
            {broadcasting ? "BROADCASTING..." : previewing ? "PREVIEWING..." : "PREVIEW CONSOLIDATION"}
        </button>
        <button
            class="cyber-btn"
            disabled={selectedCount < 2 || !selectedExceedsOneRound || selectionBusy || planning}
            on:click={handlePlanBatches}
        >
            {planning ? "PLANNING..." : "PLAN MULTI-ROUND"}
        </button>
        <button
            class="cyber-btn ghost"
            disabled={!activeRun || multiRoundRunning || activeRun.status === "completed"}
            on:click={resumeRun}
        >
            RESUME
        </button>
        <button
            class="cyber-btn ghost"
            disabled={!activeRun}
            on:click={clearRunState}
        >
            CLEAR RUN
        </button>
        {#if multiRoundRunning}
            <button class="cyber-btn danger" on:click={requestStopAfterCurrentRound}>
                STOP AFTER CURRENT ROUND
            </button>
        {/if}
        {#if status}
            <span class="status-text mono">{status}</span>
        {/if}
    </div>

    <div class="utxo-list" bind:this={utxoListEl} on:scroll={handleUtxoListScroll}>
        {#if loading}
            <CommanderLoader compact={true} label="Loading UTXOs" detail="Fetching wallet UTXO set..." />
        {:else if filteredUtxos.length === 0}
            <div class="empty-state">
                {utxos.length === 0 ? "No UTXOs found. Wallet may be empty." : sourceAddressFilter !== "__all__" ? "No UTXOs match this source address and filter set." : "No UTXOs match current filters."}
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
                        {#if virtualTopPadding > 0}
                            <tr class="spacer-row" aria-hidden="true">
                                <td colspan="5" style={`height: ${virtualTopPadding}px;`}></td>
                            </tr>
                        {/if}
                        {#each visibleUtxos as u}
                            {@const id = `${u.txid}:${u.vout}`}
                            {@const unsafe = isUnsafe(u)}
                             <tr
                                 class="utxo-row"
                                 class:selected={effectiveSelectedIds.has(id)}
                                 class:unsafe={unsafe}
                                 on:click={() => toggleUtxo(id)}
                             >
                                <td>
                                    <div
                                        class="checkbox"
                                        class:checked={effectiveSelectedIds.has(id)}
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
                        {#if virtualBottomPadding > 0}
                            <tr class="spacer-row" aria-hidden="true">
                                <td colspan="5" style={`height: ${virtualBottomPadding}px;`}></td>
                            </tr>
                        {/if}
                    </tbody>
                </table>
            </div>
        {/if}
    </div>
    {#if planning}
        <CommanderLoader compact={true} label="Planning multi-round" detail="Analyzing selected UTXO scope..." />
    {/if}
    {#if activeRun}
        <div class="warning-banner" style="margin-top:0.25rem;">
            <span>RUN STATUS: {activeRun.status?.toUpperCase()} • TARGET {activeRun.target_final_utxo_count} • EST ROUNDS {activeRun.estimated_round_count}</span>
        </div>
    {/if}
    {#if executionLog.length > 0}
        <div class="utxo-table-wrap" style="max-height: 150px;">
            <table class="utxo-table">
                <thead>
                    <tr>
                        <th>ROUND</th>
                        <th>STATUS</th>
                        <th>INPUTS</th>
                        <th>FEE</th>
                        <th>TXID</th>
                    </tr>
                </thead>
                <tbody>
                    {#each executionLog as row}
                        <tr>
                            <td>#{row.round_number}</td>
                            <td>{row.status}</td>
                            <td>{row.input_count}</td>
                            <td>{row.fee_estimate}</td>
                            <td class="mono">{row.txid ? `${row.txid.substring(0, 16)}...` : "-"}</td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
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
                        <span class="label">FEE RATE:</span>
                        <span class="value">{previewData.fee_rate_sat_per_byte} sat/B</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">ESTIMATED BYTES:</span>
                        <span class="value">{previewData.estimated_bytes}B</span>
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
                            <span class="warning-section-title">REVIEW NOTES</span>
                            {#each previewData.warnings as warn}
                                <div class="warning-box">
                                    <span class="warning-text">&#9888; {warn}</span>
                                </div>
                            {/each}
                        </div>
                    {/if}

                    <div class="info-box">
                        <span class="info-text">
                            <strong>Review before broadcasting.</strong>
                            Commander will merge the selected UTXOs into one wallet output.
                            This is normal wallet maintenance, but it costs a network fee and links those UTXOs together on-chain.
                        </span>
                    </div>
                {/if}
            </div>

            <div class="modal-footer">
                <button class="btn-cancel" on:click={cancelConsolidation} disabled={broadcasting}>
                    CANCEL
                </button>
                <button class="btn-confirm" on:click={executeConsolidation} disabled={broadcasting}>
                    {broadcasting ? "BROADCASTING..." : "CONFIRM CONSOLIDATION"}
                </button>
            </div>
        </div>
    </div>
{/if}

{#if showUnlockModal}
    <div
        class="modal-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cons-unlock-title"
        tabindex="0"
        on:click={() => (showUnlockModal = false)}
        on:keydown={(e) => e.key === "Escape" && (showUnlockModal = false)}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="modal-box unlock-modal" role="document" on:click|stopPropagation>
            <div class="modal-header">
                <span class="header-icon">&#128274;</span>
                <h2 id="cons-unlock-title">UNLOCK WALLET</h2>
            </div>
            <div class="modal-body">
                <p class="warning-text">
                    Your wallet is locked. Commander will unlock it for 5 minutes to {unlockAfterAction === "multi" ? "run multi-round consolidation" : "broadcast this consolidation transaction"}. Lock the wallet again after consolidation if you are done sending transactions.
                </p>
                <input
                    class="input-glass unlock-input"
                    type="password"
                    bind:value={unlockPassword}
                    placeholder="Wallet passphrase"
                    autocomplete="current-password"
                    on:keydown={(e) => e.key === "Enter" && unlockAndBroadcastConsolidation()}
                />
                {#if unlockError}
                    <div class="warning-box">
                        <span class="warning-text">{unlockError}</span>
                    </div>
                {/if}
            </div>
            <div class="modal-footer">
                <button class="btn-cancel" on:click={() => (showUnlockModal = false)} disabled={unlocking}>
                    CANCEL
                </button>
                <button class="btn-confirm" on:click={unlockAndBroadcastConsolidation} disabled={unlocking || !unlockPassword.trim()}>
                    {unlocking ? "UNLOCKING..." : "UNLOCK AND BROADCAST"}
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
                <h2 id="cons-plan-title">CONSOLIDATION PLAN</h2>
            </div>

            <div class="modal-body">
                {#if planData}
                    <div class="info-box" style="margin-top: 0; margin-bottom: 0.8rem;">
                        <span class="info-text">
                            Commander will consolidate the selected source UTXOs in reviewed rounds. Each round waits for confirmation before continuing. A round cap stops the run early so you can test in smaller batches.
                        </span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">INITIAL UTXOS:</span>
                        <span class="value">{planData.initial_utxo_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">TARGET FINAL:</span>
                        <span class="value">{planData.target_final_utxo_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">PROJECTED AFTER THIS RUN:</span>
                        <span class="value neon">{planData.projected_final_utxo_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">FULL RUN WOULD TAKE:</span>
                        <span class="value neon">{planData.estimated_round_count}</span>
                    </div>
                    <div class="tx-detail">
                        <span class="label">THIS RUN WILL DO:</span>
                        <span class="value neon">{planData.planned_round_count || planData.rounds?.length || planData.estimated_round_count}</span>
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
                    <div class="tx-detail">
                        <span class="label">TOTAL EST BYTES:</span>
                        <span class="value">{planData.total_estimated_bytes}</span>
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
                    <div class="warnings-section">
                        <span class="warning-section-title">REVIEW NOTES</span>
                        <div class="warning-box highlight">
                            <span class="warning-text">
                                <strong>{planData.planned_round_count} round{planData.planned_round_count !== 1 ? "s" : ""} will be executed{plannedRoundLimit > 0 ? ` (capped from ${planData.estimated_round_count} estimated)` : ""}, reducing UTXOs from {planData.initial_utxo_count} to approximately {planData.projected_final_utxo_count}.</strong> Commander will wait for confirmation after each round before starting the next.
                            </span>
                        </div>
                        {#if plannedRoundLimit > 0}
                            <div class="warning-box">
                                <span class="warning-text">&#9888; Run is capped at {plannedRoundLimit} round{plannedRoundLimit !== 1 ? "s" : ""}. Remaining {Math.max(0, planData.estimated_round_count - plannedRoundLimit)} estimated round{planData.estimated_round_count - plannedRoundLimit !== 1 ? "s" : ""} must be planned separately.</span>
                            </div>
                        {/if}
                        <div class="warning-box">
                            <span class="warning-text">&#9888; Fee estimates are approximate and based on current network conditions. Actual fees may differ. Each round combines multiple UTXOs into one output which creates a permanent on-chain link between them.</span>
                        </div>
                    </div>
                {:else}
                    <div class="empty-state" style="padding:1rem;">No rounds planned. UTXO count is already at target or insufficient inputs.</div>
                {/if}
            </div>

            <div class="modal-footer">
                <button class="btn-cancel" on:click={() => (showPlanModal = false)}>
                    CLOSE
                </button>
                <button class="btn-confirm" on:click={startMultiRound}>
                    CONFIRM START
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

    .source-filter {
        min-width: 420px;
        flex: 1 1 420px;
        max-width: 100%;
    }

    .rounds-filter {
        width: 120px;
    }

    .amount-filter {
        width: 120px;
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

    .utxo-table tr.utxo-row:hover {
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

    .utxo-row td {
        height: 33px;
        box-sizing: border-box;
    }

    .spacer-row td {
        padding: 0;
        border: 0;
        cursor: default;
        pointer-events: none;
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
        border: 1px solid rgba(0, 255, 65, 0.3);
        border-radius: 8px;
        width: 500px;
        max-width: 90vw;
        max-height: 80vh;
        overflow-y: auto;
        box-shadow:
            0 0 40px rgba(0, 255, 65, 0.15),
            0 20px 60px rgba(0, 0, 0, 0.8);
    }

    .modal-header {
        background: rgba(0, 255, 65, 0.08);
        padding: 0.8rem 1.2rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.2);
        display: flex;
        align-items: center;
        gap: 0.6rem;
    }

    .modal-header h2 {
        margin: 0;
        font-size: 0.9rem;
        color: var(--color-primary);
        letter-spacing: 2px;
        font-family: var(--font-mono);
    }

    .header-icon {
        font-size: 1.1rem;
        color: var(--color-primary);
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
        background: rgba(255, 170, 0, 0.08);
        border: 1px solid rgba(255, 170, 0, 0.22);
        border-radius: 4px;
        padding: 0.5rem 0.6rem;
        margin-top: 0.4rem;
    }

    .warning-box.highlight {
        background: rgba(255, 170, 0, 0.1);
        border-color: rgba(255, 170, 0, 0.35);
        margin-top: 0.6rem;
    }

    .info-box {
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 4px;
        padding: 0.5rem 0.6rem;
        margin-top: 0.4rem;
    }

    .info-text {
        color: #8ecfa3;
        font-size: 0.7rem;
        line-height: 1.3;
        display: block;
    }

    .info-text strong {
        color: #d8f0e0;
    }

    .info-banner {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 4px;
        padding: 0.4rem 0.6rem;
        font-size: 0.65rem;
        color: var(--color-primary);
        flex-shrink: 0;
        font-family: var(--font-mono);
        letter-spacing: 0.3px;
    }

    .warning-text {
        color: #ffd36a;
        font-size: 0.7rem;
        line-height: 1.3;
        display: block;
    }

    .warning-text strong {
        color: #ffdd00;
    }

    .modal-footer {
        display: flex;
        flex-wrap: wrap;
        gap: 0.8rem;
        padding: 0.8rem 1.2rem;
        background: rgba(0, 0, 0, 0.3);
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }

    .btn-cancel,
    .btn-confirm {
        flex: 1;
        padding: 0.55rem 0.6rem;
        font-size: 0.72rem;
        font-weight: bold;
        letter-spacing: 0.8px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
        font-family: var(--font-mono);
        white-space: nowrap;
        min-width: 0;
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

    .unlock-modal {
        max-width: 520px;
    }

    .unlock-input {
        width: 100%;
        margin-top: 0.8rem;
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
