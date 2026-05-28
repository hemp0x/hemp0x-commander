<script>
    import { onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { save, open } from "@tauri-apps/plugin-dialog";
    import { formatAmount } from "./utils.js";

    // --- FORM STATE ---
    let address = "";
    let amount = "";
    let asset = "HEMP";
    let assets = ["HEMP"];
    let status = "";

    // --- APP STATE (Refactored to Stores) ---
    import {
        nodeStatus,
        walletInfo as walletStore,
        systemStatus,
    } from "../stores.js";
    import { addNotification } from "./stores/notifications.js";

    // Reactive proxies to maintain API compatibility
    $: tauriReady = $systemStatus.tauriReady;
    $: walletInfo = $walletStore;
    $: nodeInfo = {
        state: $nodeStatus.online ? "RUNNING" : "OFFLINE",
        synced: true,
    };

    // Reactive checks derived from props/stores
    $: walletStatus = walletInfo.status;
    $: walletBalance = walletInfo.balance;
    // Require valid wallet status AND running node
    $: isNodeReady =
        walletBalance !== "--" &&
        walletStatus !== "--" &&
        nodeInfo.state === "RUNNING";

    // --- FAVORITES ---
    let favorites = [];
    let showFavorites = false;

    // --- COIN CONTROL STATE ---
    let isAdvanced = false;
    let showUtxoModal = false;
    let utxos = [];
    let selectedUtxos = new Set(); // Set of "txid:vout" strings
    let totalSelected = 0;
    let estimatedFee = 0.01; // Increased default fee to satisfy min relay fee
    let previewing = false; // In-flight guard for advanced preview/journal entry
    let broadcasting = false; // In-flight guard for advanced broadcast
    let estimatedSelectedTxBytes = 0;
    let policyDiag = null;
    let relayFeeSatPerByte = null;
    let relayFeeUnavailable = false;
    let estimatingFee = false;
    let feeEstimateError = "";

    $: maxSafeInputsForSend = policyDiag?.max_safe_inputs_for_two_outputs || 675;
    $: feeSatPerByte = estimatedSelectedTxBytes > 0
        ? (parseFloat(estimatedFee) || 0) * 100_000_000 / estimatedSelectedTxBytes
        : 0;

    // --- CONFIRMATION MODAL ---
    let showConfirmModal = false;
    let previewData = null;
    let previewJournalId = null;

    // --- LIFECYCLE ---
    onMount(() => {
        loadFavorites();
    });

    // --- ASSET DATA ---
    // Store full objects: { name, balance, type }
    let assetList = [];
    // Reactive: selected asset balance
    $: selectedBalance = (() => {
        if (asset === "HEMP") return walletBalance;
        const found = assetList.find((a) => a.name === asset);
        return found ? found.balance : "0.00";
    })();

    // React to node state (Persistent Tabs support)
    $: if (tauriReady) {
        if (nodeInfo.state === "RUNNING") {
            core.invoke("list_assets")
                .then((items) => {
                    // items is Vec<AssetItem> { name, balance, asset_type }
                    assetList = items;
                    // assets for dropdown (names only)
                    assets = [
                        "HEMP",
                        ...items
                            .filter((i) => !i.name.endsWith("!"))
                            .map((item) => item.name),
                    ];
                })
                .catch(() => {
                    assets = ["HEMP"];
                    assetList = [];
                });
        } else {
            // Node stopped - reset assets
            assets = ["HEMP"];
            assetList = [];
        }
    }

    // --- ACTIONS ---
    async function handleSend() {
        if (!tauriReady) return (status = "Backend unavailable.");
        if (!address || !amount)
            return (status = "Address and amount required.");

        if (isAdvanced) {
            await buildAdvancedPreview();
            return;
        }

        if (asset !== "HEMP") {
            await buildPreview("preview_transfer_asset", "asset_transfer", {
                destination: address,
                amount: String(amount),
                asset,
            });
            return;
        }

        await buildPreview("preview_send_hemp", "send", {
            destination: address,
            amount: String(amount),
            asset: "HEMP",
            label: null,
        });
    }

    async function buildPreview(command, operationType, params) {
        status = "Building preview...";
        try {
            previewData = await core.invoke(command, params);
            status = "";
            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: operationType,
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
            showConfirmModal = true;
        } catch (err) {
            status = `Preview failed: ${err}`;
            previewData = null;
        }
    }

    async function buildAdvancedPreview() {
        if (previewing || broadcasting) return;
        previewing = true;
        status = "Validating address...";
        previewData = null;
        previewJournalId = null;

        try {
            let isValid = false;
            try {
                const result = await core.invoke("run_cli_command", {
                    command: "validateaddress",
                    args: address,
                });
                const parsed = JSON.parse(result);
                if (!parsed.isvalid) {
                    status = "Invalid address format.";
                    return;
                }
                isValid = true;
            } catch (err) {
                status = `Could not validate address: ${err}`;
                return;
            }

            const sendAmount = parseFloat(amount);
            if (isNaN(sendAmount) || sendAmount <= 0) {
                status = "Amount must be greater than zero.";
                return;
            }
            if (selectedUtxos.size === 0) {
                status = "Select at least one UTXO.";
                return;
            }

            const maxForTwoOutputs = maxSafeInputsForSend;
            if (selectedUtxos.size > maxForTwoOutputs) {
                status = `Selected ${selectedUtxos.size} UTXOs exceeds policy limit of ${maxForTwoOutputs} for a two-output transaction. Reduce selection or consolidate first.`;
                return;
            }

            if (estimatedSelectedTxBytes > 100000) {
                status = `Estimated tx size ${estimatedSelectedTxBytes}B exceeds 100,000 byte relay limit. Reduce selected UTXOs.`;
                return;
            }

            const fee = parseFloat(estimatedFee) || 0;
            const changeAmount = totalSelected - sendAmount - fee;
            const warnings = [];

            // Fee safety checks
            if (fee <= 0) {
                status = "Fee must be greater than zero.";
                return;
            }
            if (fee < 0.00001) {
                warnings.push("Fee is below typical relay minimum (0.00001 HEMP).");
            }
            if (relayFeeSatPerByte != null && feeSatPerByte > 0 && feeSatPerByte < relayFeeSatPerByte) {
                warnings.push(`Fee rate ${feeSatPerByte.toFixed(1)} sat/byte is below min relay (${relayFeeSatPerByte.toFixed(1)} sat/byte).`);
            }
            if (sendAmount > 0 && fee > sendAmount * 0.2) {
                warnings.push("Fee exceeds 20% of send amount.");
            }

            if (totalSelected < sendAmount + fee) {
                status = "Selected inputs are insufficient for amount + fee.";
                return;
            }

            if (changeAmount > 0 && changeAmount < 0.0001) {
                warnings.push("Change amount is very small and may be treated as dust.");
            }

            warnings.push("Advanced mode manually spends only selected UTXOs. Unselected inputs are not used for fees or change.");

            const summary = `Advanced send ${sendAmount} HEMP to ${address.substring(0, 16)}${address.length > 16 ? "..." : ""}`;

            previewData = {
                destination: address,
                amount: String(sendAmount),
                asset: "HEMP",
                available_balance: String(totalSelected),
                fee_estimate: String(fee),
                fee_warning: null,
                warnings,
                summary,
                validated: isValid,
                utxo_count: selectedUtxos.size,
                input_total: totalSelected,
                change_amount: changeAmount > 0.00001 ? String(changeAmount) : null,
            };

            status = "";

            try {
                const entry = await core.invoke("add_tx_journal_entry", {
                    input: {
                        status: "Previewed",
                        operation_type: "advanced_send",
                        summary: previewData.summary,
                        txid: null,
                        details: {
                            utxo_count: previewData.utxo_count,
                            input_total: previewData.input_total,
                            amount: previewData.amount,
                            destination: previewData.destination,
                            fee: previewData.fee_estimate,
                            change_amount: previewData.change_amount,
                            warnings: previewData.warnings,
                        },
                    },
                });
                previewJournalId = entry.id;
            } catch (journalErr) {
                console.warn("Failed to record journal preview entry:", journalErr);
                previewJournalId = null;
            }

            showConfirmModal = true;
        } finally {
            previewing = false;
        }
    }

    async function executeSend() {
        showConfirmModal = false;
        try {
            status = "Broadcasting...";
            let txid;
            const amountStr = String(amount);
            if (asset === "HEMP") {
                txid = await core.invoke("send_hemp", {
                    to: address,
                    amount: amountStr,
                });
            } else {
                txid = await core.invoke("transfer_asset", {
                    asset,
                    amount: amountStr,
                    to: address,
                });
            }
            status = `Sent! ID: ${txid.substr(0, 16)}...`;
            addNotification({
                type: "transaction",
                severity: "success",
                title: "Transaction Broadcasted",
                body: `Sent ${amount} ${asset} to ${address.substring(0, 16)}...`,
                action: { label: "Copy TXID", txid },
            });
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
            amount = "";
            address = "";
            refreshWalletStatus();
        } catch (err) {
            status = `Error: ${err}`;
            addNotification({
                type: "transaction",
                severity: "error",
                title: "Transaction Failed",
                body: String(err),
            });
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
    }

    function cancelSend() {
        if (previewJournalId) {
            core.invoke("update_tx_journal_entry", {
                id: previewJournalId,
                status: "Abandoned",
                txid: null,
                details: { reason: "user_cancelled" },
            }).catch((e) => console.warn("Failed to mark journal entry as abandoned:", e));
        }
        showConfirmModal = false;
        previewData = null;
        previewJournalId = null;
    }

    async function refreshWalletStatus() {
        if (!tauriReady) return;
        try {
            const data = await core.invoke("dashboard_data");
            walletStatus = data?.wallet?.status || "--";
            walletBalance = data?.wallet?.balance || "--";
        } catch {
            /* ignore */
        }
    }

    // --- ADDRESS BOOK HELPERS ---
    let showAddressBook = false;
    let editingIndex = null;
    let editLabelText = "";

    // Add address form state
    let showAddForm = false;
    let newAddrInput = "";
    let newLabelInput = "";

    async function loadFavorites() {
        if (!tauriReady) return;
        try {
            const items = await core.invoke("load_address_book");
            favorites = items;
        } catch (err) {
            console.error("Failed to load address book:", err);
        }
    }

    async function saveFavorites() {
        if (!tauriReady) return;
        try {
            await core.invoke("save_address_book", { entries: favorites });
        } catch (err) {
            console.error("Failed to save address book:", err);
        }
    }

    // Export to JSON file via Dialog
    async function exportAddressBook() {
        try {
            const path = await save({
                filters: [{ name: "JSON", extensions: ["json"] }],
                defaultPath: "hemp0x_address_book.json",
            });
            if (!path) return;

            const data = JSON.stringify(favorites, null, 2);
            await core.invoke("write_text_file", { path, content: data });
            addNotification({
                type: "system",
                severity: "success",
                title: "Address Book Exported",
                body: "Address book saved successfully.",
            });
        } catch (err) {
            addNotification({
                type: "system",
                severity: "error",
                title: "Export Failed",
                body: String(err),
            });
        }
    }

    // Import from JSON file via Dialog
    async function triggerImport() {
        try {
            const path = await open({
                multiple: false,
                filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (!path) return;

            const text = await core.invoke("read_text_file", { path });
            const imported = JSON.parse(text);

            if (Array.isArray(imported)) {
                let count = 0;
                for (const item of imported) {
                    if (
                        item.address &&
                        !favorites.some((f) => f.address === item.address)
                    ) {
                        favorites.push({
                            label: item.label || "Imported",
                            address: item.address,
                            locked: item.locked || false,
                            date: item.date || Date.now(),
                        });
                        count++;
                    }
                }
                favorites = favorites; // react
                saveFavorites();
                addNotification({
                    type: "system",
                    severity: "success",
                    title: "Address Book Imported",
                    body: `Imported ${count} new addresses.`,
                });
            } else {
                addNotification({
                    type: "system",
                    severity: "error",
                    title: "Import Failed",
                    body: "Invalid file format: Expected an array.",
                });
            }
        } catch (err) {
            addNotification({
                type: "system",
                severity: "error",
                title: "Import Failed",
                body: String(err),
            });
        }
    }

    let showHelp = false;
    function toggleHelp() {
        showHelp = !showHelp;
    }

    const helpJson = `[
  {
    "label": "My Wallet",
    "address": "R...",
    "locked": false
  },
  {
    "label": "Cold Storage",
    "address": "HEMP...",
    "locked": true
  }
]`;

    function openAddressBook() {
        showAddressBook = true;
    }

    function closeAddressBook() {
        showAddressBook = false;
        editingIndex = null;
    }

    function selectAddress(fav) {
        address = fav.address;
        closeAddressBook();
    }

    function toggleLock(index) {
        favorites[index].locked = !favorites[index].locked;
        favorites = favorites; // trigger reactivity
        saveFavorites();
    }

    function startEditLabel(index) {
        editingIndex = index;
        editLabelText = favorites[index].label;
    }

    function saveLabel(index) {
        if (editLabelText.trim()) {
            favorites[index].label = editLabelText.trim();
            favorites = favorites;
            saveFavorites();
        }
        editingIndex = null;
    }

    function deleteAddress(index) {
        favorites.splice(index, 1);
        favorites = favorites;
        saveFavorites();
    }

    function clearUnlocked() {
        if (confirm("Clear all unlocked addresses?")) {
            favorites = favorites.filter((f) => f.locked);
            saveFavorites();
        }
    }

    function openAddForm() {
        showAddForm = true;
        newAddrInput = "";
        newLabelInput = "";
    }

    function cancelAddForm() {
        showAddForm = false;
        newAddrInput = "";
        newLabelInput = "";
    }

    async function submitNewAddress() {
        if (!newAddrInput.trim()) return;
        const addr = newAddrInput.trim();

        // 1. Check duplicates
        if (favorites.some((f) => f.address === addr)) {
            cancelAddForm();
            return;
        }

        // 2. Validate Address via RPC
        try {
            const res = await core.invoke("run_cli_args", {
                args: ["validateaddress", addr],
            });
            const json = JSON.parse(res);
            if (!json.isvalid) {
                addNotification({
                    type: "system",
                    severity: "error",
                    title: "Invalid Address",
                    body: "The entered address is not a valid Hemp0x address.",
                });
                return;
            }
        } catch (err) {
            addNotification({
                type: "system",
                severity: "error",
                title: "Validation Failed",
                body: String(err),
            });
            return;
        }

        favorites.push({
            label: newLabelInput.trim() || "Unlabeled",
            address: addr,
            locked: false,
            date: Date.now(),
        });
        favorites = favorites;
        saveFavorites();
        cancelAddForm();
    }

    function toggleStar() {
        if (!address) return;
        const idx = favorites.findIndex((f) => f.address === address);
        if (idx >= 0) {
            favorites.splice(idx, 1);
        } else {
            const label = prompt("Label for this address?", "");
            if (label !== null) {
                favorites.push({
                    label: label || "Unlabeled",
                    address,
                    locked: false,
                    date: Date.now(),
                });
            }
        }
        favorites = favorites;
        saveFavorites();
    }

    function isStarred(addr) {
        return favorites.some((f) => f.address === addr);
    }

    async function pasteAddress() {
        try {
            const text = await navigator.clipboard.readText();
            if (text) address = text.trim();
        } catch (e) {
            status = "Paste: Use Ctrl+V instead";
        }
    }

    function setMax() {
        if (isAdvanced) {
            // In advanced mode, MAX is total selected inputs - fee
            const maxVal = Math.max(0, totalSelected - estimatedFee);
            amount = maxVal.toFixed(8);
        } else {
            // Standard mode: selectedBalance
            amount = selectedBalance.replace(/,/g, "");
        }
    }

    // --- COIN CONTROL LOGIC ---

    async function toggleAdvanced() {
        isAdvanced = !isAdvanced;
        if (isAdvanced) {
            await fetchUtxos();
        } else {
            selectedUtxos = new Set();
            totalSelected = 0;
            utxos = [];
        }
    }

    async function fetchUtxos() {
        if (!tauriReady) return;
        try {
            const data = await core.invoke("list_utxos");
            utxos = data.sort((a, b) => b.amount - a.amount);
            pruneSelection(utxos);
            try {
                policyDiag = await core.invoke("get_policy_diagnostics", {
                    feeRateSatPerByte: null,
                });
            } catch (e) {
                policyDiag = null;
            }
            loadRelayFeeContext();
        } catch (e) {
            console.error("Failed to list UTXOs", e);
            status = "Error fetching UTXOs";
        }
    }

    async function loadRelayFeeContext() {
        relayFeeSatPerByte = null;
        relayFeeUnavailable = false;
        try {
            const mempoolInfo = await core.invoke("rpc_call", {
                method: "getmempoolinfo",
                params: [],
            });
            if (mempoolInfo.success && mempoolInfo.data) {
                const relayFeeHempPerKb =
                    mempoolInfo.data.mempoolminfee ||
                    mempoolInfo.data.minrelaytxfee;
                if (relayFeeHempPerKb != null && Number.isFinite(Number(relayFeeHempPerKb))) {
                    relayFeeSatPerByte = relayFeeHempPerKb * 100_000;
                    return;
                }
            }
        } catch {
            // fall through to getnetworkinfo
        }
        try {
            const netInfo = await core.invoke("rpc_call", {
                method: "getnetworkinfo",
                params: [],
            });
            if (netInfo.success && netInfo.data?.relayfee != null && Number.isFinite(Number(netInfo.data.relayfee))) {
                relayFeeSatPerByte = netInfo.data.relayfee * 100_000;
                return;
            }
        } catch {
            // unavailable
        }
        relayFeeUnavailable = true;
    }

    async function estimateSmartFee() {
        if (!isAdvanced || selectedUtxos.size === 0) {
            feeEstimateError = "Select UTXOs first.";
            return;
        }
        if (estimatedSelectedTxBytes <= 0) {
            feeEstimateError = "No transaction size estimate available.";
            return;
        }
        estimatingFee = true;
        feeEstimateError = "";
        try {
            const result = await core.invoke("rpc_call", {
                method: "estimatesmartfee",
                params: [6],
            });
            if (!result.success || !result.data || result.data.feerate == null) {
                feeEstimateError = "Fee estimation unavailable — set fee manually.";
                return;
            }
            const feerate = parseFloat(result.data.feerate);
            if (isNaN(feerate) || feerate <= 0) {
                feeEstimateError = "Fee estimation unavailable — set fee manually.";
                return;
            }
            let suggestedFee = feerate * estimatedSelectedTxBytes / 1000;
            const minRelayFee = relayFeeSatPerByte != null
                ? (relayFeeSatPerByte / 100_000) * estimatedSelectedTxBytes / 1000
                : 0.00001;
            if (suggestedFee < minRelayFee) {
                suggestedFee = minRelayFee;
            }
            if (suggestedFee > 1.0) {
                suggestedFee = 1.0;
            }
            estimatedFee = suggestedFee.toFixed(8);
        } catch {
            feeEstimateError = "Fee estimation unavailable — set fee manually.";
        } finally {
            estimatingFee = false;
        }
    }

    function pruneSelection(currentUtxos) {
        const currentIds = new Set(currentUtxos.map((u) => `${u.txid}:${u.vout}`));
        let changed = false;
        const next = new Set();
        for (const id of selectedUtxos) {
            const utxo = currentUtxos.find((u) => `${u.txid}:${u.vout}` === id);
            if (currentIds.has(id) && utxo && !isUnsafe(utxo)) {
                next.add(id);
            } else {
                changed = true;
            }
        }
        if (changed) {
            selectedUtxos = next;
            calculateSelectedTotal();
        }
    }

    function isUnsafe(u) {
        if (u.spendable === false) return true;
        if (u.safe === false) return true;
        if (u.asset && u.asset !== "HEMP") return true;
        if (u.asset_amount && u.asset_amount > 0 && u.asset !== "HEMP") return true;
        return false;
    }

    function toggleUtxo(u) {
        if (isUnsafe(u)) {
            status = "Unsafe, unspendable, or asset-bearing UTXOs cannot be used in advanced HEMP sends.";
            return;
        }
        const id = `${u.txid}:${u.vout}`;
        if (selectedUtxos.has(id)) {
            selectedUtxos.delete(id);
        } else {
            selectedUtxos.add(id);
        }
        selectedUtxos = selectedUtxos; // Reactivity
        calculateSelectedTotal();
    }

    function calculateSelectedTotal() {
        let sum = 0;
        let count = 0;
        for (const u of utxos) {
            if (selectedUtxos.has(`${u.txid}:${u.vout}`)) {
                sum += u.amount;
                count++;
            }
        }
        totalSelected = sum;
        estimatedSelectedTxBytes = 10 + count * 148 + 2 * 34;
    }

    function selectSafeMax() {
        selectedUtxos = new Set();
        const safe = utxos
            .filter((u) => !isUnsafe(u))
            .sort((a, b) => b.amount - a.amount);
        const maxInputs = maxSafeInputsForSend;
        for (let i = 0; i < Math.min(safe.length, maxInputs); i++) {
            selectedUtxos.add(`${safe[i].txid}:${safe[i].vout}`);
        }
        selectedUtxos = selectedUtxos;
        calculateSelectedTotal();
    }

    async function executeAdvancedSend() {
        if (broadcasting) return;
        broadcasting = true;
        showConfirmModal = false;
        try {
            status = "Preparing Advanced Tx...";

            const sendAmount = parseFloat(amount);
            if (isNaN(sendAmount) || sendAmount <= 0) throw "Invalid amount";
            const fee = parseFloat(estimatedFee) || 0;
            if (fee <= 0) throw "Fee must be greater than zero";

            if (totalSelected < sendAmount + fee) {
                throw "Insufficient inputs selected for Amount + Fee";
            }

            // 1. Get Raw Change Address
            const changeAddr = await core.invoke("get_change_address");

            // 2. Calculate Change
            const changeAmount = totalSelected - sendAmount - fee;

            // 3. Prepare Inputs
            let inputs = [];
            for (const id of selectedUtxos) {
                const [txid, voutStr] = id.split(":");
                inputs.push({ txid, vout: parseInt(voutStr) });
            }

            // 4. Prepare Outputs
            let outputs = {};
            outputs[address] = sendAmount.toFixed(8);

            if (changeAmount > 0.00001) {
                // Dust threshold
                outputs[changeAddr] = changeAmount.toFixed(8);
            }

            const txid = await core.invoke("broadcast_advanced_transaction", {
                inputs,
                outputs,
            });

            status = `Sent! ID: ${txid.substr(0, 16)}...`;
            addNotification({
                type: "transaction",
                severity: "success",
                title: "Advanced Transaction Broadcasted",
                body: `Sent ${amount} HEMP (advanced) to ${address.substring(0, 16)}...`,
                action: { label: "Copy TXID", txid },
            });
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
            amount = "";
            address = "";
            selectedUtxos.clear();
            totalSelected = 0;
            fetchUtxos();
            refreshWalletStatus();
        } catch (err) {
            status = `Error: ${err}`;
            addNotification({
                type: "transaction",
                severity: "error",
                title: "Advanced Transaction Failed",
                body: String(err),
            });
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
        } finally {
            broadcasting = false;
        }
    }
</script>

<div class="view-hud fade-in">
    <!-- HUD CARD -->
    <div class="glass-slab">
        <!-- HEADER STRIP -->
        <header class="slab-header">
            <div class="header-left">
                <span class="hud-title mono">[ SEND TRANSACTION ]</span>
                <div
                    class="status-chip"
                    class:locked={walletStatus === "LOCKED"}
                >
                    <span class="led"></span>
                    {walletStatus}
                </div>
                <!-- ADVANCED TOGGLE -->
                <button
                    class="toggle-adv"
                    class:active={isAdvanced}
                    on:click={toggleAdvanced}
                >
                    {isAdvanced ? "ADVANCED" : "BASIC"}
                </button>
            </div>

            <div class="balance-display">
                <div class="bal-label">AVAILABLE</div>
                <div>
                    <span class="value neon-glow">{selectedBalance}</span>
                    <span class="unit">{asset}</span>
                </div>
            </div>
        </header>

        <!-- MAIN FORM GRID -->
        <div class="slab-body">
            <!-- ROW 1: ASSET + AMOUNT (Grid) -->
            <div class="grid-row">
                <!-- Asset -->
                <div class="field-col asset-col">
                    <label for="asset-select">Asset</label>
                    <div class="input-wrapper brackets">
                        <select
                            id="asset-select"
                            bind:value={asset}
                            class="input-glass"
                        >
                            {#each assets as a}
                                <option value={a}>{a}</option>
                            {/each}
                        </select>
                    </div>
                </div>

                <div class="field-col amount-col">
                    <div class="label-row">
                        <label for="amount-input">Amount</label>
                        <button class="link-btn" on:click={setMax}>[MAX]</button
                        >
                    </div>
                    <div class="input-wrapper brackets">
                        <input
                            type="number"
                            bind:value={amount}
                            placeholder="0.00"
                            id="amount-input"
                            class="input-glass hero-input"
                        />
                    </div>
                </div>
            </div>

            <!-- UTXO READOUT (Advanced Mode) -->
            {#if isAdvanced}
                <div class="field-row utxo-control fade-in">
                    <div class="label-row">
                        <span class="field-label">Coin Control Inputs</span>
                        <span
                            class="mono"
                            style="font-size: 0.7rem; color: #fff;"
                        >
                            SELECTED: <span class="neon-glow"
                                >{formatAmount(totalSelected)}</span
                            >
                        </span>
                    </div>
                    <button
                        class="utxo-select-btn"
                        on:click={() => (showUtxoModal = true)}
                    >
                        [ SELECT UTXOS ({selectedUtxos.size}) ]
                    </button>
                    {#if estimatedSelectedTxBytes > 0}
                        <div class="utxo-estimate">
                            Selected: {estimatedSelectedTxBytes}B |
                            Max safe: {maxSafeInputsForSend} inputs
                            {#if policyDiag?.current_safe_utxo_count > maxSafeInputsForSend}
                                <span class="frag-warn"> | Wallet fragmented — consider consolidation</span>
                            {/if}
                        </div>
                    {/if}
                </div>

                <div class="field-row fee-control fade-in">
                    <div class="fee-control-row">
                        <div class="field-col fee-input-col">
                            <label for="fee-input">Manual fee (HEMP)</label>
                            <div class="input-wrapper brackets">
                                <input
                                    type="text"
                                    bind:value={estimatedFee}
                                    id="fee-input"
                                    class="input-glass"
                                />
                            </div>
                        </div>
                        <div class="field-col fee-action-col">
                            <button
                                class="btn-estimate"
                                disabled={estimatingFee || selectedUtxos.size === 0}
                                on:click={estimateSmartFee}
                            >
                                {estimatingFee ? "[ ... ]" : "[ ESTIMATE ]"}
                            </button>
                        </div>
                    </div>
                    <div class="fee-rate-info mono">
                        {#if estimatedSelectedTxBytes > 0 && feeSatPerByte > 0}
                            <span>{feeSatPerByte.toFixed(1)} sat/byte</span>
                        {/if}
                        {#if relayFeeSatPerByte != null}
                            <span class="relay-context"> | Min relay: {relayFeeSatPerByte.toFixed(1)} sat/byte</span>
                        {:else if relayFeeUnavailable}
                            <span class="relay-context muted"> | Min relay: unavailable</span>
                        {/if}
                    </div>
                    {#if feeEstimateError}
                        <div class="fee-estimate-error">{feeEstimateError}</div>
                    {/if}
                </div>
            {/if}

            <!-- ROW 2: ADDRESS (Full Width) -->
            <div class="field-row">
                <div class="label-row">
                    <label for="addr-input">Recipient Address</label>
                </div>

                <div class="input-group brackets">
                    <input
                        type="text"
                        bind:value={address}
                        placeholder="Addr..."
                        id="addr-input"
                        class="input-glass mono full-width"
                    />
                    <button
                        class="icon-btn star-btn"
                        class:active={isStarred(address)}
                        on:click={toggleStar}
                        title="Quick Save"
                    >
                        ★
                    </button>
                    <button
                        class="icon-btn book-btn"
                        on:click={openAddressBook}
                        title="Address Book"
                    >
                        📒
                    </button>
                </div>
            </div>
        </div>

        <!-- ACTION FOOTER -->
        <footer class="slab-footer">
            <button
                class="btn-send-hero"
                class:disabled={!isNodeReady || walletStatus === "LOCKED" || previewing}
                disabled={!isNodeReady || walletStatus === "LOCKED" || previewing}
                on:click={handleSend}
            >
                <span class="bracket">{`{`}</span>
                {previewing
                    ? "BUILDING PREVIEW"
                    : !isNodeReady
                    ? "NOT CONNECTED"
                    : walletStatus === "LOCKED"
                      ? "LOCKED"
                      : isAdvanced
                        ? "PREVIEW ADVANCED TX"
                        : asset === "HEMP"
                          ? "PREVIEW TX"
                          : "PREVIEW TRANSFER"}
                <span class="bracket">{`}`}</span>
            </button>

            {#if status}
                <div class="status-readout mono">{status}</div>
            {/if}
        </footer>
    </div>
</div>

<!-- CONFIRMATION MODAL -->
{#if showConfirmModal}
    <div
        class="modal-overlay"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        tabindex="-1"
        on:click={cancelSend}
        on:keydown={(e) => e.key === "Escape" && cancelSend()}
    >
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
        <div class="modal-box" role="document" on:click|stopPropagation>
            <div class="modal-header">
                <span class="warning-icon">⚠️</span>
                <h2 id="confirm-title">CONFIRM TRANSACTION</h2>
            </div>

            <div class="modal-body">
                <div class="tx-detail">
                    <span class="label">TYPE:</span>
                    <span class="value">{isAdvanced ? "ADVANCED (COIN CONTROL)" : "STANDARD"}</span>
                </div>
                <div class="tx-detail">
                    <span class="label">SENDING:</span>
                    <span class="value neon">{amount} {asset}</span>
                </div>
                <div class="tx-detail">
                    <span class="label">TO ADDRESS:</span>
                    <span class="value mono"
                        >{address.substring(0, 20)}...{address.substring(
                            address.length - 8,
                        )}</span
                    >
                </div>
                {#if previewData}
                    {#if isAdvanced}
                        <div class="tx-detail">
                            <span class="label">INPUTS:</span>
                            <span class="value">{previewData.utxo_count} UTXOs ({previewData.input_total} HEMP)</span>
                        </div>
                        {#if previewData.change_amount}
                            <div class="tx-detail">
                                <span class="label">CHANGE:</span>
                                <span class="value">{previewData.change_amount} HEMP</span>
                            </div>
                        {:else}
                            <div class="tx-detail">
                                <span class="label">CHANGE:</span>
                                <span class="value" style="color:#ff6644;">Dust / None (goes to fee)</span>
                            </div>
                        {/if}
                    {:else}
                        <div class="tx-detail">
                            <span class="label">AVAILABLE:</span>
                            <span class="value">{previewData.available_balance}</span>
                        </div>
                    {/if}
                    {#if previewData.fee_estimate}
                        <div class="tx-detail">
                            <span class="label">ESTIMATED FEE:</span>
                            <span class="value">{previewData.fee_estimate} HEMP</span>
                        </div>
                    {:else if isAdvanced}
                        <div class="tx-detail">
                            <span class="label">ESTIMATED FEE:</span>
                            <span class="value">{estimatedFee} HEMP</span>
                        </div>
                    {/if}
                    {#if previewData.fee_warning}
                        <div class="info-box">
                            <span class="info-icon">&#9432;</span>
                            <p>{previewData.fee_warning}</p>
                        </div>
                    {/if}
                    {#each previewData.warnings as warn}
                        <div class="warning-box">
                            <span class="warning-title">&#9888; {warn}</span>
                        </div>
                    {/each}
                {/if}
                <div class="warning-box">
                    <span class="warning-title">&#9888; IMPORTANT WARNING</span>
                    <p>
                        Transactions on the blockchain are <strong
                            >IRREVERSIBLE</strong
                        >.
                    </p>
                    <p>Only send to addresses you trust and have verified.</p>
                    <p>
                        Sending to the wrong address will result in <strong
                            >permanent loss</strong
                        > of funds.
                    </p>
                </div>
            </div>

            <div class="modal-footer">
                <button class="btn-cancel" on:click={cancelSend} disabled={broadcasting}
                    >[ CANCEL ]</button
                >
                <button
                    class="btn-confirm"
                    disabled={broadcasting}
                    on:click={isAdvanced ? executeAdvancedSend : executeSend}
                    >{broadcasting ? "[ BROADCASTING... ]" : "[ CONFIRM SEND ]"}</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- UTXO SELECTION MODAL -->
{#if showUtxoModal}
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="modal-overlay" on:click={() => (showUtxoModal = false)}>
        <div class="utxo-modal" on:click|stopPropagation>
            <div class="modal-header">
                <h2>SELECT COINS</h2>
                <div class="utxo-stats">
                <span>SELECTED: {formatAmount(totalSelected)}</span>
                <span>COUNT: {selectedUtxos.size}</span>
                <span>EST: {estimatedSelectedTxBytes}B</span>
            </div>
            </div>

            <div class="utxo-list">
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
                        {#each utxos as u}
                            {@const unsafe = isUnsafe(u)}
                            <tr
                                class:selected={selectedUtxos.has(
                                    `${u.txid}:${u.vout}`,
                                )}
                                class:unsafe={unsafe}
                                on:click={() => toggleUtxo(u)}
                            >
                                <td>
                                    <div
                                        class="checkbox"
                                        class:checked={selectedUtxos.has(
                                            `${u.txid}:${u.vout}`,
                                        )}
                                        class:disabled={unsafe}
                                    ></div>
                                </td>
                                <td class="amount-cell"
                                    >{u.amount.toFixed(8)}</td
                                >
                                <td class="addr-cell">
                                    {#if u.address}
                                        {u.address}
                                    {:else}
                                        <span
                                            style="color: #666; font-style: italic;"
                                            >(Change Output)</span
                                        >
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

            <div class="modal-footer">
                <button
                    class="btn-confirm"
                    on:click={selectSafeMax}>[ SELECT SAFE MAX ]</button
                >
                <button
                    class="btn-confirm"
                    on:click={() => (showUtxoModal = false)}>[ DONE ]</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- ADDRESS BOOK POPUP -->
{#if showAddressBook}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="ab-overlay" on:click={closeAddressBook}>
        <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
        <div class="ab-modal" on:click|stopPropagation>
            <div class="ab-header">
                <h3>📒 ADDRESS BOOK</h3>
                <div class="ab-header-actions">
                    <button
                        class="ab-add-btn"
                        on:click={openAddForm}
                        title="Add Address">+</button
                    >
                    <button class="ab-clear-btn" on:click={clearUnlocked}
                        >CLEAR UNLOCKED</button
                    >
                </div>
            </div>

            <!-- ADD ADDRESS FORM -->
            {#if showAddForm}
                <div class="ab-add-form">
                    <div class="ab-form-row">
                        <label for="ab-label-input">LABEL</label>
                        <input
                            id="ab-label-input"
                            type="text"
                            class="ab-form-input"
                            placeholder="e.g. Mining Pool"
                            bind:value={newLabelInput}
                        />
                    </div>
                    <div class="ab-form-row">
                        <label for="ab-addr-input">ADDRESS</label>
                        <input
                            id="ab-addr-input"
                            type="text"
                            class="ab-form-input mono"
                            placeholder="R..."
                            bind:value={newAddrInput}
                            on:keydown={(e) =>
                                e.key === "Enter" && submitNewAddress()}
                        />
                    </div>
                    <div class="ab-form-actions">
                        <button class="ab-btn" on:click={submitNewAddress}
                            >SAVE</button
                        >
                        <button class="ab-btn ghost" on:click={cancelAddForm}
                            >CANCEL</button
                        >
                    </div>
                </div>
            {/if}

            <div class="ab-list">
                {#if favorites.length === 0}
                    <div class="ab-empty">
                        No saved addresses yet. Use the ★ button to save.
                    </div>
                {:else}
                    {#each favorites as fav, i}
                        <div class="ab-row">
                            <!-- Label (editable) -->
                            <div class="ab-label-cell">
                                {#if editingIndex === i}
                                    <input
                                        type="text"
                                        class="ab-label-input"
                                        bind:value={editLabelText}
                                        on:blur={() => saveLabel(i)}
                                        on:keydown={(e) =>
                                            e.key === "Enter" && saveLabel(i)}
                                    />
                                {:else}
                                    <button
                                        class="ab-label"
                                        on:click={() => startEditLabel(i)}
                                        >{fav.label}</button
                                    >
                                {/if}
                            </div>

                            <!-- Address (click to select) -->
                            <button
                                class="ab-address mono"
                                on:click={() => selectAddress(fav)}
                            >
                                {fav.address.substring(0, 20)}...
                            </button>

                            <!-- Lock toggle -->
                            <button
                                class="ab-icon-btn"
                                class:locked={fav.locked}
                                on:click={() => toggleLock(i)}
                                title={fav.locked ? "Unlock" : "Lock"}
                            >
                                {fav.locked ? "🔒" : "🔓"}
                            </button>

                            <!-- Delete (only if not locked) -->
                            {#if !fav.locked}
                                <button
                                    class="ab-icon-btn delete"
                                    on:click={() => deleteAddress(i)}
                                    title="Delete"
                                >
                                    🗑️
                                </button>
                            {:else}
                                <span class="ab-icon-placeholder"></span>
                            {/if}
                        </div>
                    {/each}
                {/if}
            </div>

            <div class="ab-footer">
                <div class="ab-footer-left">
                    <button class="ab-btn ghost" on:click={triggerImport}
                        >[ IMPORT ]</button
                    >
                    <button class="ab-btn ghost" on:click={exportAddressBook}
                        >[ EXPORT ]</button
                    >
                    <button class="ab-btn help-btn" on:click={toggleHelp}
                        >?</button
                    >
                </div>
                <div class="ab-footer-right">
                    <button class="ab-btn ghost" on:click={closeAddressBook}
                        >CLOSE</button
                    >
                </div>
            </div>

            {#if showHelp}
                <div
                    class="ab-add-form"
                    style="
                        position: absolute;
                        bottom: 70px;
                        left: 1rem; 
                        right: 1rem;
                        max-height: 320px;
                        overflow-y: auto;
                        background: rgba(10, 25, 18, 0.98); 
                        border: 1px solid var(--color-primary); 
                        border-radius: 8px;
                        box-shadow: 0 -4px 30px rgba(0,0,0,0.8);
                        z-index: 50;
                        padding: 1rem;
                        margin: 0;
                    "
                >
                    <div
                        class="field-label"
                        style="text-align:center; margin-bottom:0.5rem; color:var(--color-primary); font-weight:bold;"
                    >
                        JSON FORMAT GUIDE
                    </div>
                    <code
                        style="display:block; font-size:0.75rem; color:#ddd; white-space:pre-wrap; background:rgba(255,255,255,0.05); padding:0.8rem; border-radius:6px; overflow-x:auto;"
                    >
                        {helpJson}
                    </code>
                    <button
                        class="ab-btn ghost"
                        style="width:100%; margin-top:0.8rem;"
                        on:click={toggleHelp}>CLOSE HELP</button
                    >
                </div>
            {/if}
        </div>
    </div>
{/if}

<style lang="css">
    /* --- VIEW CONTAINER --- */
    .view-hud {
        height: 100%;
        display: flex;
        justify-content: center;
        align-items: flex-start; /* KEY FIX: Align to top */
        padding: 0.5rem;
        padding-top: 0.5rem; /* ULTRA COMPACT (was 1.5rem) */
        padding-bottom: 0.5rem;
        overflow-y: auto;
        overflow-x: hidden;
    }

    /* --- ANIMATIONS --- */
    /* --- ANIMATIONS (In app.css) --- */

    /* --- SHARED STYLES (In app.css) --- */

    /* --- HEADER (Specifics) --- */

    .header-left {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .balance-display {
        text-align: right;
        font-family: var(--font-mono);
        display: flex;
        flex-direction: column;
        justify-content: center;
    }
    .bal-label {
        font-size: 0.6rem;
        color: var(--color-muted);
        text-transform: uppercase;
        letter-spacing: 1px;
        margin-bottom: 2px;
    }
    .balance-display .value {
        font-size: 1.2rem;
        font-weight: 700;
        color: #fff;
    }
    .unit {
        font-size: 0.8rem;
        color: var(--color-primary);
    }

    /* --- BODY --- */

    .grid-row {
        display: flex;
        gap: 0.6rem; /* TIGHT GAP (was 1.2rem) */
    }
    .asset-col {
        flex: 0.4;
    }
    .amount-col {
        flex: 1;
    }

    .field-col,
    .field-row {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
    }

    .label-row {
        display: flex;
        justify-content: space-between;
        align-items: baseline;
    }

    label {
        font-size: 0.7rem;
        text-transform: uppercase;
        letter-spacing: 1px;
        color: var(--color-muted);
        font-family: var(--font-mono);
    }

    .link-btn {
        background: none;
        border: none;
        padding: 0;
        font-size: 0.7rem;
        color: var(--color-primary-dim);
        cursor: pointer;
        opacity: 0.8;
        font-family: var(--font-mono);
    }
    .link-btn:hover {
        opacity: 1;
        color: var(--color-primary);
    }

    /* BRACKETS & INPUTS (Global brackets in app.css) */

    .input-group {
        display: flex;
        gap: 4px;
    }

    .input-glass {
        width: 100%;
        background: rgba(0, 0, 0, 0.3);
        border: none;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.5rem 0.6rem; /* COMPACT INPUTS (was 0.8rem) */
        font-size: 0.9rem;
        outline: none;
        transition: all 0.2s;
        font-family: var(--font-mono);
    }
    .input-glass:focus {
        background: rgba(0, 255, 65, 0.05);
        border-bottom-color: var(--color-primary);
    }

    /* Select dropdown dark theme */
    select.input-glass {
        appearance: none;
        -webkit-appearance: none;
        -moz-appearance: none;
        cursor: pointer;
        padding-right: 2rem;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300ff41' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 0.8rem center;
    }
    select.input-glass option {
        background: #0a0a0a;
        color: #fff;
        padding: 0.5rem;
    }

    .hero-input {
        font-size: 1.4rem; /* COMPACT HERO (was 1.8rem) */
        font-weight: bold;
        color: var(--color-primary);
    }

    /* BUTTONS */
    .icon-btn {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: var(--color-muted);
        font-family: var(--font-mono);
        font-size: 0.7rem;
        padding: 0 0.8rem;
        cursor: pointer;
        min-width: 60px;
    }
    .icon-btn:hover {
        background: rgba(0, 255, 65, 0.1);
        color: #fff;
        border-color: rgba(0, 255, 65, 0.3);
    }
    .star-btn {
        min-width: 40px;
        font-size: 1rem;
        padding: 0;
    }
    .star-btn.active {
        color: #ffbd2e;
        border-color: #ffbd2e;
        text-shadow: 0 0 5px #ffbd2e;
    }

    /* --- FOOTER --- */

    .btn-send-hero {
        width: 100%;
        padding: 1rem;
        font-size: 1.1rem;
        font-weight: 700;
        letter-spacing: 3px;
        font-family: var(--font-mono);
        color: #000;
        background: #00dd38; /* Slightly dimmed green */
        border: none;
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.25);
        cursor: pointer;
        transition: all 0.2s;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 1rem;
    }
    .btn-send-hero:hover:not(:disabled) {
        box-shadow: 0 0 40px rgba(0, 255, 65, 0.5);
        letter-spacing: 5px;
    }
    .bracket {
        font-weight: 300;
        opacity: 0.5;
    }

    .btn-send-hero:disabled {
        background: #333;
        color: #666;
        box-shadow: none;
        cursor: not-allowed;
    }

    .status-readout {
        min-height: 1rem;
        font-size: 0.75rem;
        color: var(--color-primary);
        text-align: center;
        text-transform: uppercase;
        letter-spacing: 1px;
    }

    /* --- CONFIRMATION MODAL --- */
    /* .modal-overlay moved to components.css */
    .utxo-modal {
        background: #0a0a0a;
        border: 1px solid var(--color-primary);
        box-shadow: 0 0 50px rgba(0, 255, 65, 0.1);
        width: 600px;
        max-width: 95vw;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        border-radius: 8px;
    }
    .utxo-list {
        flex: 1;
        overflow-y: auto;
        padding: 1rem;
    }
    .utxo-stats {
        margin-left: auto;
        font-family: var(--font-mono);
        color: #888;
        font-size: 0.8rem;
        display: flex;
        gap: 1rem;
    }
    .utxo-table {
        width: 100%;
        border-collapse: collapse;
        font-family: var(--font-mono);
        font-size: 0.8rem;
    }
    .utxo-table th {
        text-align: left;
        color: #666;
        padding: 0.5rem;
        border-bottom: 1px solid #333;
    }
    .utxo-table td {
        padding: 0.5rem;
        border-bottom: 1px solid #222;
        color: #ccc;
        cursor: pointer;
    }
    .utxo-table tr:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .utxo-table tr.selected {
        background: rgba(0, 255, 65, 0.1);
    }
    .utxo-table tr.selected td {
        color: #fff;
    }
    .amount-cell {
        color: var(--color-primary) !important;
        font-weight: bold;
    }
    .checkbox {
        width: 16px;
        height: 16px;
        border: 1px solid #666;
        border-radius: 2px;
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
    .utxo-table tr.unsafe {
        opacity: 0.6;
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

    .modal-box {
        background: linear-gradient(180deg, #0a0a0a 0%, #121212 100%);
        border: 1px solid rgba(255, 221, 0, 0.4);
        border-radius: 8px;
        padding: 0;
        width: 450px;
        max-width: 90vw;
        box-shadow:
            0 0 40px rgba(255, 221, 0, 0.2),
            0 20px 60px rgba(0, 0, 0, 0.8);
    }
    .modal-header {
        background: rgba(255, 221, 0, 0.1);
        padding: 1rem 1.5rem;
        border-bottom: 1px solid rgba(255, 221, 0, 0.2);
        display: flex;
        align-items: center;
        gap: 0.8rem;
    }
    .modal-header h2 {
        margin: 0;
        font-size: 1rem;
        color: #ffdd00;
        letter-spacing: 2px;
    }
    .warning-icon {
        font-size: 1.2rem;
    }
    .modal-body {
        padding: 1.5rem;
    }
    .tx-detail {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.8rem;
        background: rgba(0, 255, 65, 0.05);
        border-left: 3px solid var(--color-primary);
        margin-bottom: 0.8rem;
        border-radius: 0 4px 4px 0;
    }
    .tx-detail .label {
        color: #888;
        font-size: 0.75rem;
        letter-spacing: 1px;
    }
    .tx-detail .value {
        color: #fff;
        font-weight: bold;
    }
    .tx-detail .value.neon {
        color: var(--color-primary);
        text-shadow: 0 0 8px rgba(0, 255, 65, 0.5);
    }
    .warning-box {
        background: rgba(255, 68, 68, 0.1);
        border: 1px solid rgba(255, 68, 68, 0.3);
        border-radius: 6px;
        padding: 1rem;
        margin-top: 1rem;
    }
    .warning-title {
        display: block;
        color: #ff4444;
        font-weight: bold;
        font-size: 0.8rem;
        letter-spacing: 1px;
        margin-bottom: 0.5rem;
    }
    .warning-box p {
        margin: 0.3rem 0;
        color: #ccc;
        font-size: 0.8rem;
        line-height: 1.4;
    }
    .warning-box strong {
        color: #ff6666;
    }
    .info-box {
        background: rgba(0, 255, 200, 0.08);
        border: 1px solid rgba(0, 255, 200, 0.25);
        border-radius: 6px;
        padding: 0.8rem 1rem;
        margin-top: 0.8rem;
        display: flex;
        align-items: flex-start;
        gap: 0.5rem;
    }
    .info-icon {
        color: #00ffc8;
        font-size: 1rem;
        flex-shrink: 0;
    }
    .info-box p {
        margin: 0;
        color: #aaa;
        font-size: 0.75rem;
        line-height: 1.4;
    }
    .modal-footer {
        display: flex;
        gap: 1rem;
        padding: 1rem 1.5rem;
        background: rgba(0, 0, 0, 0.3);
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .btn-cancel,
    .btn-confirm {
        flex: 1;
        padding: 0.8rem;
        font-size: 0.8rem;
        font-weight: bold;
        letter-spacing: 1px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .btn-cancel {
        background: transparent;
        border: 1px solid #666;
        color: #888;
    }
    .btn-cancel:hover {
        border-color: #ff4444;
        color: #ff4444;
    }
    .btn-confirm {
        background: rgba(0, 255, 65, 0.15);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
    }
    .btn-confirm:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 20px rgba(0, 255, 65, 0.4);
    }

    /* === ADDRESS BOOK POPUP === */
    .ab-overlay {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.85);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 99999;
        padding: 1rem;
        padding-bottom: 15vh; /* Shifts modal up from center */
        animation: abFadeIn 0.2s ease-out;
    }
    @keyframes abFadeIn {
        from {
            opacity: 0;
        }
        to {
            opacity: 1;
        }
    }
    .ab-modal {
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        width: 100%;
        max-width: 550px;
        max-height: 70vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 0 40px rgba(0, 255, 65, 0.1);
        animation: slideUp 0.25s ease-out;
    }
    @keyframes slideUp {
        from {
            opacity: 0;
            transform: translateY(20px) scale(0.98);
        }
        to {
            opacity: 1;
            transform: translateY(0) scale(1);
        }
    }
    .ab-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem 1.2rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .ab-header h3 {
        margin: 0;
        color: var(--color-primary);
        font-size: 0.95rem;
        letter-spacing: 1px;
    }
    .ab-header-actions {
        display: flex;
        gap: 0.8rem;
        align-items: center;
    }
    .ab-add-btn {
        background: transparent;
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        width: 28px;
        height: 28px;
        font-size: 1.2rem;
        line-height: 1;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .ab-add-btn:hover {
        background: var(--color-primary);
        color: #000;
    }
    .book-btn {
        font-size: 1.1rem;
    }
    .ab-clear-btn {
        background: transparent;
        border: 1px solid rgba(255, 68, 68, 0.3);
        color: #ff6666;
        padding: 0.4rem 0.8rem;
        font-size: 0.7rem;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
        font-family: var(--font-mono);
    }
    .ab-clear-btn:hover {
        background: rgba(255, 68, 68, 0.1);
        border-color: #ff6666;
    }
    .ab-list {
        flex: 1;
        overflow-y: auto;
        padding: 0.5rem;
    }
    .ab-empty {
        text-align: center;
        color: #555;
        padding: 2rem;
        font-size: 0.85rem;
    }
    .ab-row {
        display: grid;
        grid-template-columns: 1fr 1.5fr auto auto;
        gap: 0.5rem;
        align-items: center;
        padding: 0.6rem 0.8rem;
        border-radius: 6px;
        transition: background 0.15s;
    }
    .ab-row:hover {
        background: rgba(0, 255, 65, 0.03);
    }
    .ab-label-cell {
        overflow: hidden;
    }
    .ab-label {
        background: none;
        border: none;
        color: #ccc;
        font-size: 0.8rem;
        cursor: pointer;
        padding: 0.3rem 0.5rem;
        border-radius: 4px;
        width: 100%;
        text-align: left;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .ab-label:hover {
        background: rgba(255, 255, 255, 0.05);
        color: var(--color-primary);
    }
    .ab-label-input {
        width: 100%;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid var(--color-primary);
        color: #fff;
        padding: 0.3rem 0.5rem;
        font-size: 0.8rem;
        border-radius: 4px;
        outline: none;
    }
    .ab-address {
        background: none;
        border: none;
        color: var(--color-primary);
        font-size: 0.75rem;
        cursor: pointer;
        padding: 0.3rem 0.5rem;
        border-radius: 4px;
        text-align: left;
        transition: all 0.15s;
    }
    .ab-address:hover {
        background: rgba(0, 255, 65, 0.1);
    }
    .ab-icon-btn {
        background: none;
        border: none;
        cursor: pointer;
        font-size: 1rem;
        opacity: 0.5;
        transition: all 0.15s;
        padding: 0.3rem;
    }
    .ab-icon-btn:hover {
        opacity: 1;
    }
    .ab-icon-btn.locked {
        opacity: 1;
        filter: drop-shadow(0 0 3px var(--color-primary));
    }
    .ab-icon-btn.delete:hover {
        filter: drop-shadow(0 0 5px #ff4444);
    }
    .ab-icon-placeholder {
        width: 1.6rem;
        display: inline-block;
    }
    .ab-footer {
        display: flex;
        justify-content: space-between; /* Split left/right */
        align-items: center;
        padding: 1rem 1.2rem;
        border-top: 1px solid rgba(255, 255, 255, 0.05);
    }
    .ab-footer-left,
    .ab-footer-right {
        display: flex;
        gap: 0.8rem;
    }
    .help-btn {
        width: 30px;
        padding-left: 0;
        padding-right: 0;
        text-align: center;
        border-radius: 50%;
    }
    .ab-btn {
        background: transparent;
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.5rem 1rem;
        font-size: 0.8rem;
        font-family: var(--font-mono);
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.2s;
    }
    .ab-btn:hover {
        background: var(--color-primary);
        color: #000;
    }
    .ab-btn.ghost {
        border-color: #555;
        color: #888;
    }
    .ab-btn.ghost:hover {
        border-color: #fff;
        color: #fff;
        background: transparent;
    }

    /* ADD ADDRESS FORM */
    .ab-add-form {
        padding: 1rem 1.2rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        background: rgba(0, 255, 65, 0.03);
        animation: slideDown 0.2s ease-out;
    }
    @keyframes slideDown {
        from {
            opacity: 0;
            transform: translateY(-10px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
    .ab-form-row {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
        margin-bottom: 0.8rem;
    }
    .ab-form-row label {
        color: #666;
        font-size: 0.7rem;
        letter-spacing: 1px;
    }
    .ab-form-input {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: #fff;
        padding: 0.6rem 0.8rem;
        font-size: 0.85rem;
        border-radius: 4px;
        outline: none;
        transition: border-color 0.2s;
    }
    .ab-form-input:focus {
        border-color: var(--color-primary);
        box-shadow: 0 0 8px rgba(0, 255, 65, 0.15);
    }
    .ab-form-input::placeholder {
        color: #444;
    }
    .ab-form-actions {
        display: flex;
        gap: 0.8rem;
        margin-top: 0.5rem;
    }

    /* NEW STYLES */
    .toggle-adv {
        background: none;
        border: 1px solid #444;
        color: #666;
        font-size: 0.6rem;
        padding: 0.2rem 0.5rem;
        cursor: pointer;
        font-family: var(--font-mono);
        margin-left: 1rem;
        transition: all 0.2s;
    }
    .toggle-adv:hover {
        border-color: #888;
        color: #888;
    }
    .toggle-adv.active {
        border-color: var(--color-primary);
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.2);
    }

    .utxo-control {
        margin-bottom: 0.5rem; /* TIGHT MARGIN */
        background: rgba(0, 255, 65, 0.05);
        padding: 0.2rem 0.5rem; /* SLIM PADDING */
        border-radius: 4px;
        border-left: 2px solid var(--color-primary);
    }

    .fee-control {
        margin-bottom: 0.5rem;
        background: rgba(0, 255, 65, 0.03);
        padding: 0.5rem;
        border-radius: 4px;
        border-left: 2px solid rgba(0, 255, 65, 0.6);
    }
    .fee-control-row {
        display: flex;
        gap: 0.6rem;
        align-items: flex-end;
    }
    .fee-input-col {
        flex: 1;
    }
    .fee-action-col {
        flex-shrink: 0;
    }
    .btn-estimate {
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        font-family: var(--font-mono);
        font-size: 0.65rem;
        padding: 0.5rem 0.8rem;
        cursor: pointer;
        transition: all 0.2s;
        white-space: nowrap;
    }
    .btn-estimate:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.3);
    }
    .btn-estimate:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .fee-rate-info {
        font-size: 0.6rem;
        color: #999;
        margin-top: 0.3rem;
    }
    .relay-context {
        color: #666;
    }
    .relay-context.muted {
        color: #444;
    }
    .fee-estimate-error {
        font-size: 0.65rem;
        color: #ffaa00;
        margin-top: 0.2rem;
        font-family: var(--font-mono);
    }
    .utxo-select-btn {
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid #444;
        color: #ccc;
        width: 100%;
        padding: 0.5rem;
        font-family: var(--font-mono);
        cursor: pointer;
        transition: all 0.2s;
    }
    .utxo-select-btn:hover {
        border-color: var(--color-primary);
        color: #fff;
    }

    .utxo-estimate {
        font-size: 0.6rem;
        color: #666;
        font-family: var(--font-mono);
        margin-top: 0.2rem;
    }

    .frag-warn {
        color: #ffaa00;
        font-weight: bold;
    }

    /* UTXO MODAL STYLES */
    .utxo-modal {
        background: #0a0a0a;
        border: 1px solid var(--color-primary);
        box-shadow: 0 0 50px rgba(0, 255, 65, 0.1);
        width: 800px;
        max-width: 95vw;
        max-height: 80vh;
        display: flex;
        flex-direction: column;
        border-radius: 8px;
    }
    .utxo-list {
        flex: 1;
        overflow-y: auto;
        padding: 1rem;
    }
    .utxo-stats {
        margin-left: auto;
        font-family: var(--font-mono);
        color: #888;
        font-size: 0.8rem;
        display: flex;
        gap: 1rem;
    }
    .utxo-table {
        width: 100%;
        border-collapse: collapse;
        font-family: var(--font-mono);
        font-size: 0.8rem;
    }
    .utxo-table th {
        text-align: left;
        color: #666;
        padding: 0.5rem;
        border-bottom: 1px solid #333;
    }
    .utxo-table td {
        padding: 0.5rem;
        border-bottom: 1px solid #222;
        color: #ccc;
        cursor: pointer;
    }
    .utxo-table tr:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .utxo-table tr.selected {
        background: rgba(0, 255, 65, 0.1);
    }
    .utxo-table tr.selected td {
        color: #fff;
    }
    .amount-cell {
        color: var(--color-primary) !important;
        font-weight: bold;
    }
    .addr-cell {
        font-family: var(--font-mono);
        color: #ddd;
        max-width: 450px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .checkbox {
        width: 16px;
        height: 16px;
        border: 1px solid #666;
        border-radius: 2px;
    }
    .checkbox.checked {
        background: var(--color-primary);
        border-color: var(--color-primary);
        box-shadow: 0 0 5px var(--color-primary);
    }
    .field-label {
        color: #888;
        font-size: 0.8rem;
        font-weight: bold;
    }

    .ab-icon-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #ddd;
        font-size: 1.2rem;
        width: 32px;
        height: 32px;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        border-radius: 4px;
        transition: all 0.2s;
    }
    .ab-icon-btn:hover {
        border-color: var(--color-primary);
        color: #fff;
        background: rgba(0, 255, 65, 0.1);
    }

    /* === RESPONSIVE DESIGN === */
    @media (max-width: 700px) {
        .grid-row {
            flex-direction: column;
            gap: 0.8rem;
        }
        .asset-col,
        .amount-col {
            flex: 1;
        }
        .slab-header {
            flex-direction: column;
            gap: 0.8rem;
            align-items: flex-start;
        }
        .header-left {
            flex-wrap: wrap;
            gap: 0.5rem;
        }
        .balance-display {
            text-align: left;
        }
    }

    @media (max-height: 700px) {
        .view-hud {
            padding: 0.3rem;
            padding-top: 0.3rem;
        }
        .slab-body {
            padding: 0.8rem 1rem;
            gap: 0.8rem;
        }
        .slab-footer {
            padding: 0.8rem 1rem;
        }
        .btn-send-hero {
            padding: 0.8rem 1.5rem;
            font-size: 0.85rem;
        }
    }
</style>
