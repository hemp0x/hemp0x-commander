<script>
    import { createEventDispatcher, onDestroy } from "svelte";
    import { fly, fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { save, open } from "@tauri-apps/plugin-dialog";
    import CryptoJS from "crypto-js";
    import { systemStatus } from "../../stores.js"; // Import Store

    $: tauriReady = $systemStatus.tauriReady;
    export let isProcessing = false;
    export let processingMessage = "";
    // Functions passed from parent
    export let openModal;
    export let closeModal;

    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    // --- WALLET MANAGEMENT ---
    let restorePath = "";

    async function backupWallet() {
        if (!tauriReady) return false;
        try {
            const ts = new Date()
                .toISOString()
                .replace(/[-:T]/g, "")
                .slice(0, 14);
            const filePath = await save({
                title: "Save Wallet Backup",
                defaultPath: `wallet_backup_${ts}.dat`,
                filters: [{ name: "Wallet Data", extensions: ["dat"] }],
            });
            if (!filePath) return false;

            showToast("Backing up wallet...", "info");
            await core.invoke("backup_wallet_to", { path: filePath });
            showToast(`Backup saved to: ${filePath}`, "success");
            return true;
        } catch (err) {
            showToast(`Backup failed: ${err}`, "error");
            return false;
        }
    }

    function askRestartNode() {
        openModal("RESTART NODE?", "A restart is required to apply changes.", [
            {
                label: "RESTART NOW",
                style: "primary",
                onClick: async () => {
                    closeModal();
                    showToast("Restarting node...", "info");
                    try {
                        await core.invoke("stop_node");
                        setTimeout(async () => {
                            await core.invoke("start_node");
                            showToast("Node Restarted", "success");
                        }, 3000);
                    } catch (e) {
                        showToast(
                            "Node Stopped. Please restart manually.",
                            "warning",
                        );
                    }
                },
            },
            {
                label: "LATER",
                style: "ghost",
                onClick: closeModal,
            },
        ]);
    }

    async function restoreWallet() {
        if (!tauriReady) return;

        // Step 1: Browse for file immediately
        try {
            const selected = await open({
                title: "Select Wallet Backup",
                multiple: false,
                filters: [{ name: "Wallet Files", extensions: ["dat", "bak"] }],
            });
            if (!selected) return; // User cancelled browse
            restorePath = selected;
        } catch (err) {
            showToast("Browse failed", "error");
            return;
        }

        // Step 2: Confirm backup of current wallet
        openModal(
            "BACKUP CURRENT WALLET?",
            "Restoring a wallet will overwrite loaded wallet.dat! Backup first?",
            [
                {
                    label: "BACK UP",
                    style: "primary", // Leftmost / Primary
                    onClick: async () => {
                        closeModal(); // Close first to avoid overlap or weird states
                        const backedUp = await backupWallet();
                        if (backedUp) proceedRestore();
                    },
                },
                {
                    label: "SKIP",
                    style: "ghost",
                    onClick: () => {
                        closeModal();
                        proceedRestore();
                    },
                },
                {
                    label: "CANCEL",
                    style: "danger", // Rightmost / Danger
                    onClick: closeModal,
                },
            ],
        );
    }

    async function proceedRestore() {
        try {
            showToast("Restoring wallet...", "info");
            // We use backupExisting: false because we handled it manually (or user skipped)
            // We use restartNode: false because we handle it manually
            await core.invoke("restore_wallet", {
                path: restorePath,
                backupExisting: false,
                restartNode: false,
            });
            showToast("Restore Successful!", "success");
            askRestartNode();
        } catch (err) {
            showToast("Restore Failed: " + err, "error");
        }
    }

    async function createNewWallet() {
        if (!tauriReady) return;
        openModal(
            "BACKUP CURRENT WALLET?",
            "Creating a new wallet will overwrite loaded wallet.dat! Backup first?",
            [
                {
                    label: "BACK UP",
                    style: "primary",
                    onClick: async () => {
                        closeModal();
                        const backedUp = await backupWallet();
                        if (backedUp) proceedNewWallet();
                    },
                },
                {
                    label: "SKIP",
                    style: "ghost",
                    onClick: () => {
                        closeModal();
                        proceedNewWallet();
                    },
                },
                {
                    label: "CANCEL",
                    style: "danger",
                    onClick: closeModal,
                },
            ],
        );
    }

    // --- ENCRYPTION FLOW ---
    let showEncryptModal = false;
    let newEncPass = "";
    let newEncPassConfirm = "";

    async function proceedNewWallet() {
        try {
            isProcessing = true;
            processingMessage = "Stopping Node...";

            // Step 1: Stop Node Explicitly (Improve Performance/Reliability)
            await core.invoke("stop_node");

            // wait 2s for process to unwind
            await new Promise((r) => setTimeout(r, 2000));

            processingMessage = "Creating Wallet Files...";

            // Step 2: Create Wallet (Node is already stopped, so this is just file ops)
            await core.invoke("create_new_wallet", {
                backupExisting: false,
                restartNode: false, // We handle start manually
            });

            processingMessage = "Starting Node...";

            // Step 3: Start Node
            await core.invoke("start_node");

            isProcessing = false;
            showToast("New Wallet Created", "success");

            // Delay slightly to let UI settle, then ask encrypt
            setTimeout(() => {
                askEncryptNewWallet();
            }, 1500);
        } catch (e) {
            isProcessing = false;
            showToast("Create Failed: " + e, "error");
        }
    }

    function askEncryptNewWallet() {
        openModal(
            "ENCRYPT NEW WALLET?",
            "Secure your wallet with a password now? (Recommended)",
            [
                {
                    label: "ENCRYPT",
                    style: "primary",
                    onClick: () => {
                        closeModal();
                        showEncryptModal = true;
                    },
                },
                {
                    label: "LEAVE UNENCRYPTED",
                    style: "danger",
                    onClick: closeModal,
                },
            ],
        );
    }

    async function performWalletEncrypt() {
        if (!newEncPass || newEncPass !== newEncPassConfirm) {
            showToast("Passwords do not match", "error");
            return;
        }
        showEncryptModal = false;

        isProcessing = true;
        processingMessage = "Encrypting Wallet (Node Stopping)...";

        try {
            // 'wallet_encrypt' command in backend calls 'encryptwallet <pass>'
            // This RPC command STOPS the node.
            await core.invoke("wallet_encrypt", { password: newEncPass });

            // Wait a moment for the command to register
            await new Promise((r) => setTimeout(r, 2000));

            isProcessing = false;

            // Clear sensitive password fields
            newEncPass = "";
            newEncPassConfirm = "";

            // Node stopped during encryption; user must restart manually.
            openModal(
                "ENCRYPTION COMPLETE",
                "Your wallet is now encrypted. The node has been stopped for security.\n\nPlease start the node manually from the System tab or restart the app.",
                [
                    {
                        label: "OK",
                        style: "primary",
                        onClick: closeModal,
                    },
                ],
            );
        } catch (e) {
            isProcessing = false;
            showToast("Encryption Failed: " + e, "error");
        }
    }

    // --- SECURITY / PASSWORD CHANGE ---
    let passOld = "";
    let passNew = "";
    let passNewConfirm = "";

    async function changePassword() {
        if (!tauriReady) return;
        if (!passOld || !passNew) {
            showToast("Enter old and new passwords", "error");
            return;
        }
        if (passNew !== passNewConfirm) {
            showToast("New passwords do not match", "error");
            return;
        }
        try {
            const res = await core.invoke("change_wallet_password", {
                old_pass: passOld,
                new_pass: passNew,
            });
            showToast("Password Updated Successfully", "success");
            passOld = "";
            passNew = "";
            passNewConfirm = "";
        } catch (err) {
            showToast("Password Change Failed", "error");
        }
    }

    // --- KEY MANAGEMENT ---
    let showKeyModal = false;
    let keyModalMode = "export"; // "export" or "import"
    let keyList = []; // Array of { address, selected, key (optional), label (optional) }
    let keyListLoading = false;
    let importRescan = true;

    // Encrypted Export State
    let showExportEncryptModal = false;
    let exportEncPass = "";
    let exportEncPassConfirm = "";
    let processingKeys = false;

    // Unlock State
    let showUnlockModal = false;
    let unlockPassword = "";
    let unlockError = "";

    // Hardened export danger flow state
    let showExportDangerModal = false;
    let exportDangerPhrase = "";
    let exportDangerCountdown = 0;
    let exportDangerTimer = null;
    const DANGER_CONFIRM = "SHOW PRIVATE KEY";
    const DANGER_COUNTDOWN = 5;

    function clearExportDangerState() {
        exportDangerPhrase = "";
        exportDangerCountdown = 0;
        if (exportDangerTimer) {
            clearInterval(exportDangerTimer);
            exportDangerTimer = null;
        }
    }

    function closeExportDangerModal() {
        showExportDangerModal = false;
        clearExportDangerState();
        showKeyModal = false;
        showExportEncryptModal = false;
        exportEncPass = "";
        exportEncPassConfirm = "";
        processingKeys = false;
    }

    onDestroy(() => {
        clearExportDangerState();
        exportEncPass = "";
        exportEncPassConfirm = "";
        unlockPassword = "";
        pendingImportData = null;
        keyList = [];
    });

    async function openExportModal() {
        if (!tauriReady) return;
        keyModalMode = "export";
        showKeyModal = true;
        await loadWalletKeys();
    }

    async function openImportModal() {
        if (!tauriReady) return;
        keyModalMode = "import";
        showKeyModal = true;
        keyList = [];
        triggerImport();
    }

    async function loadWalletKeys() {
        if (!tauriReady) return;
        keyListLoading = true;
        try {
            // Get addresses mostly
            // We use 'listaddressgroupings' or similar to find used addresses with balances
            const Groups = await core.invoke("list_address_groupings");
            // Flatten
            const flat = [];
            Groups.forEach((group) => {
                group.forEach((item) => {
                    // item: [address, balance, account?]
                    flat.push({
                        address: item[0],
                        balance: item[1],
                        selected: false,
                        label: "", // could fetch label if needed
                    });
                });
            });
            keyList = flat;
        } catch (e) {
            showToast("Failed to load keys: " + e, "error");
        }
        keyListLoading = false;
    }

    async function triggerImport() {
        try {
            const selected = await open({
                title: "Select Key File",
                multiple: false,
                filters: [{ name: "JSON Key File", extensions: ["json"] }],
            });
            if (!selected) {
                showKeyModal = false; // Cancelled
                return;
            }

            const content = await core.invoke("read_text_file", {
                path: selected,
            });

            // Try parse
            try {
                let data = JSON.parse(content);
                // Check if encrypted
                if (data.encrypted && data.content) {
                    decryptImportedFile(data);
                } else {
                    if (Array.isArray(data)) {
                        keyList = data.map((k) => ({ ...k, selected: true }));
                    } else {
                        throw "Invalid Format";
                    }
                }
            } catch (e) {
                showToast("Invalid JSON: " + e, "error");
                showKeyModal = false;
            }
        } catch (e) {
            showToast("File Read Error", "error");
            showKeyModal = false;
        }
    }

    // Decryption for import
    let pendingImportData = null;
    function decryptImportedFile(data) {
        pendingImportData = data;
        unlockPassword = "";
        unlockingFile = true; // Important: Set BEFORE showing modal
        showUnlockModal = true;
    }

    let unlockingFile = false;

    async function tryUnlockWallet() {
        if (!unlockPassword) return;

        if (unlockingFile && pendingImportData) {
            // Decrypt File
            try {
                const bytes = CryptoJS.AES.decrypt(
                    pendingImportData.content,
                    unlockPassword,
                );
                const decryptedStr = bytes.toString(CryptoJS.enc.Utf8);
                if (!decryptedStr) throw "Wrong Password";
                const keys = JSON.parse(decryptedStr);
                keyList = keys.map((k) => ({ ...k, selected: true }));
                showUnlockModal = false;
                unlockingFile = false;
                pendingImportData = null;
                showToast("File Decrypted", "success");
            } catch (e) {
                unlockError = "Decryption Failed: Wrong Password?";
            }
            return;
        }

        // Standard Wallet Unlock (for Export)
        try {
            unlockError = "";
            await core.invoke("wallet_unlock", {
                password: unlockPassword,
                duration: 60,
            });
            showUnlockModal = false;
            proceedExport();
        } catch (e) {
            unlockError = "Incorrect Passphrase";
        }
    }

    function toggleSelectAll() {
        const allSelected = keyList.every((k) => k.selected);
        keyList = keyList.map((k) => ({ ...k, selected: !allSelected }));
    }

    async function executeExport() {
        const selected = keyList.filter((k) => k.selected);
        if (selected.length === 0) {
            showToast("Select at least one key", "warning");
            return;
        }

        exportDangerPhrase = "";
        exportDangerCountdown = 0;
        if (exportDangerTimer) {
            clearInterval(exportDangerTimer);
            exportDangerTimer = null;
        }
        showExportDangerModal = true;
    }

    function startDangerCountdown() {
        if (exportDangerTimer) return;
        exportDangerCountdown = DANGER_COUNTDOWN;
        exportDangerTimer = setInterval(() => {
            exportDangerCountdown -= 1;
            if (exportDangerCountdown <= 0) {
                exportDangerCountdown = 0;
                clearInterval(exportDangerTimer);
                exportDangerTimer = null;
            }
        }, 1000);
    }

    function resetDangerCountdown() {
        exportDangerCountdown = 0;
        if (exportDangerTimer) {
            clearInterval(exportDangerTimer);
            exportDangerTimer = null;
        }
    }

    $: if (showExportDangerModal && exportDangerPhrase === DANGER_CONFIRM && !exportDangerTimer) {
        startDangerCountdown();
    }
    $: if (showExportDangerModal && exportDangerPhrase !== DANGER_CONFIRM && exportDangerTimer) {
        resetDangerCountdown();
    }

    function onDangerConfirmed() {
        if (exportDangerCountdown > 0) return;
        if (exportDangerPhrase !== DANGER_CONFIRM) return;

        showExportDangerModal = false;
        clearExportDangerState();

        unlockingFile = false;
        showExportEncryptModal = true;
    }

    async function finalizeExport() {
        if (!exportEncPass || exportEncPass.length < 8) {
            showToast("Export encryption password must be at least 8 characters", "error");
            return;
        }
        if (exportEncPass !== exportEncPassConfirm) {
            showToast("Passwords do not match", "error");
            return;
        }
        showExportEncryptModal = false;

        // Check wallet lock by trying to dump one key
        // If fail, show wallet unlock modal.
        // If success, proceed.
        showUnlockModal = true; // Use tryUnlockWallet to gate this
    }

    async function proceedExport() {
        // Wallet is unlocked (or didn't need it).
        processingKeys = true;
        const selected = keyList.filter((k) => k.selected);
        const exportData = [];

        try {
            for (const item of selected) {
                const privKey = await core.invoke("dump_priv_key", {
                    address: item.address,
                });
                exportData.push({
                    address: item.address,
                    privKey: privKey,
                    label: item.label,
                    balance: item.balance,
                });
            }

            // Now encrypt exportData if pass provided
            let finalContent = "";
            let isEncrypted = false;

            if (exportEncPass) {
                const ciphertext = CryptoJS.AES.encrypt(
                    JSON.stringify(exportData),
                    exportEncPass,
                ).toString();
                finalContent = JSON.stringify({
                    encrypted: true,
                    content: ciphertext,
                    version: "1.0",
                });
                isEncrypted = true;
            } else {
                finalContent = JSON.stringify(exportData, null, 2);
            }

            // Save File
            const ts = new Date()
                .toISOString()
                .replace(/[-:T]/g, "")
                .slice(0, 14);
            const path = await save({
                title: "Save Keys",
                defaultPath: `hemp0x_keys_${ts}.json`,
                filters: [{ name: "JSON Key File", extensions: ["json"] }],
            });

            if (path) {
                await core.invoke("write_text_file", {
                    path,
                    content: finalContent,
                });
                showToast("Keys Exported Successfully", "success");
                showKeyModal = false;
            }
        } catch (e) {
            showToast("Export Failed: " + e, "error");
            // If error was lock related, we might want to prompt unlock again, but we just did.
        }
        try {
            await core.invoke("wallet_lock");
        } catch (e) {
            // Some unencrypted wallets do not need a lock operation.
        }
        exportEncPass = "";
        exportEncPassConfirm = "";
        unlockPassword = "";
        processingKeys = false;
    }

    async function executeImport() {
        const selected = keyList.filter((k) => k.selected);
        if (selected.length === 0) return;

        processingKeys = true;
        let successCount = 0;
        let failCount = 0;

        for (const item of selected) {
            try {
                await core.invoke("import_priv_key", {
                    privKey: item.privKey,
                    label: item.label || "",
                    rescan: false,
                });
                successCount++;
            } catch (e) {
                failCount++;
            }
        }

        processingKeys = false;
        showToast(
            `Imported ${successCount} keys. ${failCount} failed.`,
            "info",
        );
        showKeyModal = false;

        if (importRescan && successCount > 0) {
            // Trigger rescan? core.invoke('rescan_blockchain')?
            // Usually 'importprivkey' with true does it.
            // We disabled it for loop speed.
            // We might need a generic 'rescan' command or restart with -reindex.
            showToast(
                "Please restart with -reindex or use console 'rescanblockchain' to see balances.",
                "warning",
            );
        }
    }

    // --- MIGRATION TOOLS ---
    let migrationExportPath = "";
    let migrationExportPrivate = false;
    let migrationExportOverwrite = false;
    let migrationExportPass = "";
    let migrationWorking = false;
    let migrationExportResult = null;
    let migrationRestoreResult = null;
    let migrationError = "";

    let migrationValidatePath = "";
    let migrationValidatePass = "";
    let migrationValidateResult = null;

    let migrationRestorePath = "";
    let migrationRestoreName = "";
    let migrationRestorePass = "";
    let migrationRestoreBirth = "";
    let migrationRestoreConfirm = "";
    const RESTORE_CONFIRM = "RESTORE WALLET";

    async function migrateExport() {
        if (migrationWorking) return;
        migrationError = "";
        migrationExportResult = null;

        if (!migrationExportPath) {
            migrationError = "Select a destination file first";
            return;
        }
        if (migrationExportPrivate && !migrationExportPass) {
            migrationError = "Export passphrase is required for private export";
            return;
        }
        if (migrationExportPrivate && migrationExportPass.length < 8) {
            migrationError = "Export passphrase must be at least 8 characters";
            return;
        }

        if (migrationExportPrivate) {
            openModal(
                "EXPORT PRIVATE MIGRATION PACKAGE?",
                "This encrypted file can restore your wallet if the export passphrase is known. Store the file and passphrase separately and never share either one.",
                [
                    {
                        label: "EXPORT",
                        style: "danger",
                        onClick: () => {
                            closeModal();
                            performMigrationExport();
                        },
                    },
                    {
                        label: "CANCEL",
                        style: "ghost",
                        onClick: closeModal,
                    },
                ],
            );
            return;
        }

        await performMigrationExport();
    }

    async function performMigrationExport() {
        migrationWorking = true;
        try {
            migrationExportResult = await core.invoke("export_wallet_migration", {
                path: migrationExportPath,
                includePrivate: migrationExportPrivate,
                allowOverwrite: migrationExportOverwrite,
                exportPassphrase: migrationExportPass,
            });
            showToast("Migration package exported successfully", "success");
        } catch (e) {
            migrationError = String(e);
            showToast("Export failed: " + e, "error");
        }
        migrationWorking = false;
        migrationExportPass = "";
    }

    async function migrateSelectExportPath() {
        try {
            const selected = await save({
                title: "Save Migration Package",
                defaultPath: "hemp0x_migration.json",
                filters: [{ name: "JSON Migration", extensions: ["json"] }],
            });
            if (selected) {
                migrationExportPath = selected;
            }
        } catch (e) { /* user cancelled */ }
    }

    async function migrateSelectValidatePath() {
        try {
            const selected = await open({
                title: "Select Migration Package",
                multiple: false,
                filters: [{ name: "Migration Files", extensions: ["json"] }],
            });
            if (selected) {
                migrationValidatePath = selected;
                migrationValidateResult = null;
                migrationError = "";
            }
        } catch (e) { /* user cancelled */ }
    }

    async function migrateValidate() {
        if (migrationWorking) return;
        migrationError = "";
        migrationValidateResult = null;

        if (!migrationValidatePath) {
            migrationError = "Select a file first";
            return;
        }

        migrationWorking = true;
        try {
            migrationValidateResult = await core.invoke("validate_wallet_migration", {
                path: migrationValidatePath,
                passphrase: migrationValidatePass,
            });
            showToast("Validation complete", "info");
        } catch (e) {
            migrationError = String(e);
            showToast("Validation failed: " + e, "error");
        }
        migrationWorking = false;
        migrationValidatePass = "";
    }

    async function migrateSelectRestorePath() {
        try {
            const selected = await open({
                title: "Select Migration Package to Restore",
                multiple: false,
                filters: [{ name: "Migration Files", extensions: ["json"] }],
            });
            if (selected) {
                migrationRestorePath = selected;
            }
        } catch (e) { /* user cancelled */ }
    }

    async function migrateRestore() {
        if (migrationWorking) return;
        if (migrationRestoreConfirm !== RESTORE_CONFIRM) {
            showToast("Type RESTORE WALLET to confirm", "warning");
            return;
        }
        if (!migrationRestorePath) {
            migrationError = "Select a migration file first";
            return;
        }
        if (!migrationRestoreName) {
            migrationError = "Enter a wallet name for the restored wallet";
            return;
        }
        if (!migrationRestorePass) {
            migrationError = "Export passphrase is required";
            return;
        }
        if (migrationRestoreBirth !== "" && (!Number.isInteger(Number(migrationRestoreBirth)) || Number(migrationRestoreBirth) < 0)) {
            migrationError = "Birth height must be a non-negative whole number";
            return;
        }

        migrationWorking = true;
        migrationError = "";
        migrationRestoreResult = null;
        try {
            migrationRestoreResult = await core.invoke("restore_wallet_migration", {
                path: migrationRestorePath,
                walletName: migrationRestoreName,
                passphrase: migrationRestorePass,
                birthHeight: migrationRestoreBirth === "" ? null : Number(migrationRestoreBirth),
            });
            showToast("Wallet restored successfully", "success");
        } catch (e) {
            migrationError = String(e);
            showToast("Restore failed: " + e, "error");
        }
        migrationWorking = false;
        migrationRestorePass = "";
        migrationRestoreConfirm = "";
    }

    onDestroy(() => {
        migrationExportPass = "";
        migrationValidatePass = "";
        migrationRestorePass = "";
        migrationRestoreConfirm = "";
    });
</script>

<div class="tool-grid wallet-view">
    <!-- BACKUP COLUMN -->
    <div class="glass-panel panel-soft card">
        <header class="card-header">
            <span class="card-title">WALLET MANAGEMENT</span>
        </header>
        <div class="card-body">
            <p class="desc" style="text-align: center;">
                SAFEGUARD YOUR WALLET.DAT FILE
            </p>
            <button class="cyber-btn wide" on:click={backupWallet}>
                [ BACKUP WALLET.DAT ]
            </button>

            <div class="laser-divider"></div>

            <p class="desc" style="text-align: center;">
                RESTORE OR CREATE WALLET FROM FILE
            </p>

            <div class="btn-row" style="gap: 1rem;">
                <button class="cyber-btn ghost wide" on:click={restoreWallet}
                    >RESTORE</button
                >
                <button
                    class="cyber-btn ghost danger wide"
                    on:click={createNewWallet}>NEW WALLET</button
                >
            </div>

            <!-- Key Management at Bottom -->
            <div style="margin-top: auto; padding-top: 1.5rem;">
                <div class="laser-divider"></div>
                <p
                    class="desc"
                    style="text-align:center; color:#666; margin-bottom:1rem;"
                >
                    EXPORT OR IMPORT PRIVATE KEYS
                </p>
                <div class="btn-row">
                    <button
                        class="cyber-btn ghost wide danger"
                        on:click={openExportModal}
                    >
                        EXPORT KEYS
                    </button>
                    <button
                        class="cyber-btn ghost wide"
                        on:click={openImportModal}
                    >
                        IMPORT KEYS
                    </button>
                </div>
            </div>
        </div>
    </div>

    <!-- SECURITY & KEY MANAGEMENT COLUMN -->
    <div class="glass-panel panel-soft card">
        <header class="card-header">
            <span class="card-title">SECURITY</span>
        </header>
        <div class="card-body">
            <p class="desc">Update encryption key.</p>
            <div class="field-group">
                <input
                    type="password"
                    class="input-glass"
                    placeholder="Current Password"
                    bind:value={passOld}
                />
                <input
                    type="password"
                    class="input-glass"
                    placeholder="New Password"
                    bind:value={passNew}
                />
                <input
                    type="password"
                    class="input-glass"
                    placeholder="Confirm New"
                    bind:value={passNewConfirm}
                />
            </div>
            <button class="cyber-btn wide" on:click={changePassword}>
                [ UPDATE PASSWORD ]
            </button>
        </div>
    </div>
</div>

<!-- ================= MIGRATION TOOLS ================= -->
<div class="glass-panel panel-soft" style="margin: 1rem 0; padding: 0;">
    <header class="card-header">
        <span class="card-title">MIGRATION TOOLS</span>
    </header>
    <div style="padding: 1.5rem; display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1.5rem;">
        <!-- EXPORT -->
        <div style="display:flex; flex-direction:column; gap: 0.75rem;">
            <h4 style="color:var(--color-primary); margin:0; font-size:0.8rem;">EXPORT MIGRATION PACKAGE</h4>
            <p class="desc">Creates a local migration artifact for Webcom/Commander compatibility.</p>
            <button class="cyber-btn ghost wide" on:click={migrateSelectExportPath}>
                {migrationExportPath ? migrationExportPath.split('/').pop().split('\\').pop() : "CHOOSE DESTINATION"}
            </button>
            <label class="toggle" style="font-size:0.75rem; color:#888;">
                <input type="checkbox" bind:checked={migrationExportPrivate} />
                Include private keys (encrypted)
            </label>
            {#if migrationExportPrivate}
                <input type="password" class="input-glass" placeholder="Export Passphrase (min 8 chars)" bind:value={migrationExportPass} style="font-size:0.75rem; padding:0.5rem;"/>
            {/if}
            <label class="toggle" style="font-size:0.75rem; color:#888;">
                <input type="checkbox" bind:checked={migrationExportOverwrite} />
                Allow overwrite existing file
            </label>
            <button class="cyber-btn" on:click={migrateExport} disabled={migrationWorking || !migrationExportPath}>
                {migrationWorking ? "EXPORTING..." : "[ EXPORT ]"}
            </button>
            {#if migrationError}
                <p style="color:#ff5555; font-size:0.7rem; margin:0;">{migrationError}</p>
            {/if}
            {#if migrationExportResult}
                <div style="background:rgba(0,255,65,0.05); padding:0.5rem; border-radius:4px; font-size:0.7rem; max-height:120px; overflow-y:auto;">
                    <p style="color:var(--color-primary); margin:0;">Exported: {migrationExportResult.filename}</p>
                    <p style="color:#888; margin:0.25rem 0;">Chain: {migrationExportResult.chain}</p>
                    <p style="color:#888; margin:0;">Private: {migrationExportResult.private_keys_included}</p>
                </div>
            {/if}
        </div>

        <!-- VALIDATE -->
        <div style="display:flex; flex-direction:column; gap: 0.75rem;">
            <h4 style="color:var(--color-primary); margin:0; font-size:0.8rem;">VALIDATE MIGRATION PACKAGE</h4>
            <p class="desc">Check a migration envelope before restoring.</p>
            <button class="cyber-btn ghost wide" on:click={migrateSelectValidatePath}>
                {migrationValidatePath ? migrationValidatePath.split('/').pop().split('\\').pop() : "SELECT FILE"}
            </button>
            <input type="password" class="input-glass" placeholder="Export Passphrase (if encrypted)" bind:value={migrationValidatePass} style="font-size:0.75rem; padding:0.5rem;"/>
            <button class="cyber-btn" on:click={migrateValidate} disabled={migrationWorking || !migrationValidatePath}>
                {migrationWorking ? "VALIDATING..." : "[ VALIDATE ]"}
            </button>
            {#if migrationValidateResult}
                <div style="background:rgba(0,255,65,0.05); padding:0.5rem; border-radius:4px; font-size:0.7rem; max-height:160px; overflow-y:auto;">
                    <p style="color:{migrationValidateResult.valid ? 'var(--color-primary)' : '#ff5555'}; margin:0;">
                        {migrationValidateResult.valid ? 'VALID' : 'INVALID'}
                    </p>
                    <p style="color:#888; margin:0.25rem 0;">Network: {migrationValidateResult.chain?.network ?? "unknown"} | Matches: {migrationValidateResult.chain?.matches_current_chain ?? "unknown"}</p>
                    <p style="color:#888; margin:0;">Restorable: {migrationValidateResult.restorable}</p>
                    {#if migrationValidateResult.restorable_reason}
                        <p style="color:#888; margin:0;">Reason: {migrationValidateResult.restorable_reason}</p>
                    {/if}
                    {#if migrationValidateResult.warnings?.length}
                        {#each migrationValidateResult.warnings as w}
                            <p style="color:#ffaa00; margin:0; font-size:0.65rem;">{w}</p>
                        {/each}
                    {/if}
                </div>
            {/if}
            {#if migrationError}
                <p style="color:#ff5555; font-size:0.7rem; margin:0;">{migrationError}</p>
            {/if}
        </div>

        <!-- RESTORE -->
        <div style="display:flex; flex-direction:column; gap: 0.75rem;">
            <h4 style="color:var(--color-danger, #ff5555); margin:0; font-size:0.8rem;">RESTORE MIGRATION PACKAGE</h4>
            <p class="desc" style="color:#ffaa00;"><strong>WARNING:</strong> Backup your current wallet first. This creates a new wallet. Restart required.</p>
            <button class="cyber-btn ghost danger wide" on:click={migrateSelectRestorePath}>
                {migrationRestorePath ? migrationRestorePath.split('/').pop().split('\\').pop() : "SELECT FILE"}
            </button>
            <input type="text" class="input-glass" placeholder="New Wallet Name" bind:value={migrationRestoreName} style="font-size:0.75rem; padding:0.5rem;"/>
            <input type="password" class="input-glass" placeholder="Export Passphrase" bind:value={migrationRestorePass} style="font-size:0.75rem; padding:0.5rem;"/>
            <input type="number" min="0" step="1" class="input-glass" placeholder="Birth Height (optional)" bind:value={migrationRestoreBirth} style="font-size:0.75rem; padding:0.5rem;"/>
            <input type="text" class="input-glass" placeholder="Type RESTORE WALLET to confirm" bind:value={migrationRestoreConfirm} style="font-size:0.75rem; padding:0.5rem;"/>
            <button
                class="cyber-btn danger"
                on:click={migrateRestore}
                disabled={migrationWorking || !migrationRestorePath || !migrationRestoreName || !migrationRestorePass || migrationRestoreConfirm !== RESTORE_CONFIRM}
            >
                {migrationWorking ? "RESTORING..." : "[ RESTORE WALLET ]"}
            </button>
            {#if migrationRestoreResult && !migrationError}
                <div style="background:rgba(0,255,65,0.05); padding:0.5rem; border-radius:4px; font-size:0.7rem; max-height:120px; overflow-y:auto;">
                    <p style="color:var(--color-primary); margin:0;">Wallet: {migrationRestoreResult.wallet_name}</p>
                    <p style="color:#888; margin:0.25rem 0;">Use -wallet={migrationRestoreResult.wallet_arg}</p>
                    {#if migrationRestoreResult.warnings?.length}
                        {#each migrationRestoreResult.warnings as w}
                            <p style="color:#ffaa00; margin:0; font-size:0.65rem;">{w}</p>
                        {/each}
                    {/if}
                </div>
            {/if}
            {#if migrationError}
                <p style="color:#ff5555; font-size:0.7rem; margin:0;">{migrationError}</p>
            {/if}
        </div>
    </div>
</div>

<!-- ================= MODALS ================= -->

<!-- ENCRYPTION INPUT MODAL (New Wallet) -->
{#if showEncryptModal}
    <div class="modal-overlay">
        <div class="modal-staged modal-frame">
            <h3
                class="neon-text"
                style="color:var(--color-primary); margin-top:0;"
            >
                SET WALLET PASSWORD
            </h3>
            <p class="modal-text" style="margin-bottom: 1.5rem; color:#bbb;">
                <strong>IMPORTANT:</strong> If you lose your password, you will
                <strong style="color:#ff5555;">LOSE ACCESS</strong>
                to your funds forever.<br />
                Save it securely in a safe place!
            </p>

            <div class="input-group" style="margin-bottom:1rem;">
                <label for="enc-pass">NEW PASSWORD</label>
                <input
                    id="enc-pass"
                    type="password"
                    class="input-glass"
                    bind:value={newEncPass}
                    placeholder="Enter Password"
                />
            </div>

            <div class="input-group" style="margin-bottom:2rem;">
                <label for="enc-pass-confirm">CONFIRM PASSWORD</label>
                <input
                    id="enc-pass-confirm"
                    type="password"
                    class="input-glass"
                    bind:value={newEncPassConfirm}
                    placeholder="Confirm Password"
                />
            </div>

            <div class="modal-actions">
                <button
                    class="cyber-btn primary-glow"
                    on:click={performWalletEncrypt}
                    style="min-height:50px; flex:1;"
                >
                    ENCRYPT
                </button>
                <button
                    class="cyber-btn ghost"
                    on:click={() => (showEncryptModal = false)}
                    style="min-height:50px; flex:1;"
                >
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- EXPORT DANGER CONFIRMATION MODAL -->
{#if showExportDangerModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={closeExportDangerModal}
        on:keydown={(e) => e.key === "Escape" && closeExportDangerModal()}
    >
        <div class="modal-staged modal-frame" style="max-width:480px;">
            <h3 style="color:#ff5555; margin-top:0; text-align:center;">
                HIGH SECURITY WARNING
            </h3>
            <div style="background:rgba(255,0,0,0.08); padding:1rem; border-radius:4px; margin-bottom:1rem;">
                <p style="color:#ff5555; margin:0 0 0.5rem; font-size:0.8rem; text-align:center;">
                    <strong>PRIVATE KEYS GRANT FULL CONTROL OVER YOUR FUNDS</strong>
                </p>
                <p style="color:#ffaa00; margin:0.25rem 0; font-size:0.72rem;">
                    NEVER share private keys with anyone
                </p>
                <p style="color:#ffaa00; margin:0.25rem 0; font-size:0.72rem;">
                    Keys will NOT be auto-copied to clipboard
                </p>
                <p style="color:#ffaa00; margin:0.25rem 0; font-size:0.72rem;">
                    Keys will NOT be stored on disk without your explicit file save
                </p>
            </div>

            <div class="input-group" style="margin-bottom:1rem;">
                <label for="export-danger-phrase" style="font-size:0.75rem; color:#ff5555;">
                    Type <strong>SHOW PRIVATE KEY</strong> to confirm you understand these risks:
                </label>
                <input
                    id="export-danger-phrase"
                    type="text"
                    class="input-glass"
                    style="border-color:rgba(255,85,85,0.5);"
                    bind:value={exportDangerPhrase}
                    placeholder="SHOW PRIVATE KEY"
                />
            </div>

            {#if exportDangerPhrase === DANGER_CONFIRM}
                <div style="text-align:center; margin-bottom:1rem;">
                    <p style="color:#ffaa00; margin:0; font-size:0.8rem;">
                        Please wait {exportDangerCountdown} second(s) before proceeding...
                    </p>
                </div>
            {/if}

            <div class="modal-actions">
                <button
                    class="cyber-btn danger"
                    on:click={onDangerConfirmed}
                    disabled={exportDangerCountdown > 0 || exportDangerPhrase !== DANGER_CONFIRM}
                    style="min-height:50px; flex:1;"
                >
                    {exportDangerCountdown > 0 ? `WAIT (${exportDangerCountdown}s)` : 'EXPORT PRIVATE KEYS'}
                </button>
                <button
                    class="cyber-btn ghost"
                    on:click={closeExportDangerModal}
                    style="min-height:50px; flex:1;"
                >
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- EXPORT ENCRYPTION PASSWORD MODAL -->
{#if showExportEncryptModal}
    <div class="modal-overlay">
        <div class="modal-staged modal-frame" style="max-width:400px;">
            <h3
                class="neon-text"
                style="color:var(--color-primary); margin-top:0;"
            >
                🔒 SET EXPORT PASSWORD
            </h3>
            <p style="color:#aaa; margin-bottom:1rem;">
                Enter a password to encrypt this file.
            </p>

            <div class="input-group" style="margin-bottom:1rem;">
                <label for="exp-pass">ENCRYPTION PASSWORD</label>
                <input
                    id="exp-pass"
                    type="password"
                    class="input-glass"
                    bind:value={exportEncPass}
                    placeholder="Required, minimum 8 characters"
                />
            </div>
            <div class="input-group" style="margin-bottom:1rem;">
                <label for="exp-pass-confirm">CONFIRM</label>
                <input
                    id="exp-pass-confirm"
                    type="password"
                    class="input-glass"
                    bind:value={exportEncPassConfirm}
                    placeholder="Confirm encryption password"
                />
            </div>

            <div class="modal-actions">
                <button class="cyber-btn" on:click={finalizeExport}
                    >CONTINUE</button
                >
                <button
                    class="cyber-btn ghost"
                    on:click={() => (showExportEncryptModal = false)}
                    >CANCEL</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- UNLOCK MODAL -->
{#if showUnlockModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => (showUnlockModal = false)}
        on:keydown={(e) => e.key === "Escape" && (showUnlockModal = false)}
    >
        <div class="modal-staged">
            <div class="modal-header">
                <h3>
                    {unlockingFile ? "🔓 DECRYPT FILE" : "🔐 UNLOCK WALLET"}
                </h3>
            </div>
            <div class="modal-body">
                <p>
                    {unlockingFile
                        ? "Enter password to decrypt file:"
                        : "Enter your wallet passphrase to export keys."}
                </p>
                <div class="input-wrapper brackets">
                    <input
                        type="password"
                        class="input-glass"
                        placeholder="Passphrase"
                        bind:value={unlockPassword}
                        on:keydown={(e) =>
                            e.key === "Enter" && tryUnlockWallet()}
                    />
                </div>
                {#if unlockError}
                    <div
                        class="error-msg"
                        style="color: #ff5555; margin-top: 0.5rem; font-size: 0.8rem;"
                    >
                        {unlockError}
                    </div>
                {/if}
            </div>
            <div class="modal-actions">
                <button class="cyber-btn" on:click={tryUnlockWallet}
                    >UNLOCK</button
                >
                <button
                    class="cyber-btn ghost"
                    on:click={() => (showUnlockModal = false)}>CANCEL</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- KEY LIST MODAL -->
{#if showKeyModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => (showKeyModal = false)}
        on:keydown={(e) => e.key === "Escape" && (showKeyModal = false)}
    >
        <div class="modal-staged wide">
            <div class="modal-header">
                <h3>
                    {keyModalMode === "export"
                        ? "📤 EXPORT PRIVATE KEYS"
                        : "📥 IMPORT PRIVATE KEYS"}
                </h3>
                <button
                    class="btn-close-x"
                    on:click={() => (showKeyModal = false)}>✕</button
                >
            </div>
            <div class="modal-body">
                {#if keyListLoading}
                    <div style="padding: 2rem; text-align: center;">
                        Loading keys...
                    </div>
                {:else}
                    <div
                        class="key-list-controls"
                        style="margin-bottom: 0.5rem; display:flex; justify-content:space-between;"
                    >
                        <button class="text-btn" on:click={toggleSelectAll}
                            >Select All / None</button
                        >
                        <span class="mono"
                            >{keyList.filter((k) => k.selected).length} selected</span
                        >
                    </div>

                    <div class="key-list-scroll key-list-dark">
                        {#each keyList as item}
                            <label class="key-item">
                                <input
                                    type="checkbox"
                                    bind:checked={item.selected}
                                />
                                <div style="flex: 1; min-width: 0;">
                                    <div
                                        class="mono"
                                        style="color: var(--color-primary); font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis;"
                                    >
                                        {item.address}
                                    </div>
                                    {#if item.label}<div
                                            style="font-size: 0.7rem; color: #888;"
                                        >
                                            {item.label}
                                        </div>{/if}
                                </div>
                                <div
                                    class="key-balance mono"
                                    style="text-align: right; flex-shrink: 0; margin-left: 1rem;"
                                >
                                    <div
                                        style="color: #fff; font-size: 0.85rem; font-weight: 600;"
                                    >
                                        {item.balance}
                                    </div>
                                    <div
                                        style="font-size: 0.65rem; color: #666;"
                                    >
                                        HEMP
                                    </div>
                                </div>
                            </label>
                        {/each}
                        {#if keyList.length === 0}
                            <div
                                style="padding: 2rem; text-align: center; color: #666;"
                            >
                                No keys found.
                            </div>
                        {/if}
                    </div>

                    {#if keyModalMode === "import"}
                        <div style="margin-top: 1rem;">
                            <label class="toggle">
                                <input
                                    type="checkbox"
                                    bind:checked={importRescan}
                                />
                                <span
                                    >Rescan blockchain after import (slower but
                                    finds transactions)</span
                                >
                            </label>
                        </div>
                    {/if}
                {/if}
            </div>
            <div class="modal-actions">
                {#if keyModalMode === "export"}
                    <button
                        class="cyber-btn danger"
                        on:click={executeExport}
                        disabled={keyListLoading || processingKeys}
                    >
                        {processingKeys ? "EXPORTING..." : "EXPORT SELECTED"}
                    </button>
                {:else}
                    <button
                        class="cyber-btn"
                        on:click={executeImport}
                        disabled={keyListLoading || processingKeys}
                    >
                        {processingKeys ? "IMPORTING..." : "IMPORT SELECTED"}
                    </button>
                {/if}
                <button
                    class="cyber-btn ghost"
                    on:click={() => (showKeyModal = false)}>CANCEL</button
                >
            </div>
        </div>
    </div>
{/if}

<style>
    /* Scoped styles needed for wallet view, taken from ViewTools */
    .tool-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 1.5rem;
        height: 100%;
        overflow: hidden;
        padding: 1rem;
    }
    .wallet-view {
        grid-template-columns: 1fr 1fr;
    }
    .card {
        display: flex;
        flex-direction: column;
        padding: 0;
        overflow: hidden;
        background: rgba(0, 0, 0, 0.2);
    }
    .card-header {
        background: rgba(0, 255, 65, 0.05);
        padding: 0.8rem 1.2rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
    }
    .card-title {
        font-size: 0.85rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
    }
    .card-body {
        padding: 1.5rem;
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 1rem;
        overflow-y: auto;
    }
    .desc {
        font-size: 0.75rem;
        color: #888;
        line-height: 1.4;
        margin: 0;
    }
    .btn-row {
        display: flex;
        gap: 1rem;
    }
    .wide {
        width: 100%;
    }
    .laser-divider {
        height: 1px;
        background: linear-gradient(
            90deg,
            transparent,
            rgba(0, 255, 65, 0.3),
            transparent
        );
        margin: 0.5rem 0;
    }
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }
    /* Key list styles */
    .key-list-scroll {
        max-height: 300px;
        overflow-y: auto;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
    }
    .key-item {
        display: flex;
        align-items: center;
        gap: 0.8rem;
        padding: 0.6rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        cursor: pointer;
    }
    .key-item:hover {
        background: rgba(255, 255, 255, 0.05);
    }
    .text-btn {
        background: none;
        border: none;
        color: var(--color-primary);
        text-decoration: underline;
        cursor: pointer;
        font-size: 0.7rem;
        padding: 0;
    }
    .input-group label {
        display: block;
        font-size: 0.7rem;
        margin-bottom: 0.3rem;
        color: #888;
    }
    .input-glass {
        width: 100%;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.7rem;
        border-radius: 4px;
        font-family: inherit;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
        outline: none;
    }
</style>
