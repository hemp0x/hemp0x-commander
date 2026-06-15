<script>
    import { createEventDispatcher, onDestroy, onMount } from "svelte";
    import { fly, fade } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import CryptoJS from "crypto-js";
    import { systemStatus } from "../../stores.js"; // Import Store
    import { addToolNotification } from "../stores/notifications.js";


    $: tauriReady = $systemStatus.tauriReady;
    export let isProcessing = false;
    export let processingMessage = "";
    // Functions passed from parent
    export let openModal;
    export let closeModal;

    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info", notify = true) {
        dispatch("toast", { msg, type, notify });
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

            // Refresh header state for the new wallet
            await refreshWalletHeader();

            // Surface the "Finish vault backup" nudge so the user
            // backs up the new wallet into the vault right away.
            showFinishBackupNudge = true;

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
    let unlockAfterWalletUnlock = null;
    let unlockModalPurpose = "key_export";

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

    // One-time light header refresh once Tauri is available. Avoids
    // heavy startup work; just a few metadata-only Tauri calls.
    let headerInitialised = false;
    $: if (tauriReady && !headerInitialised) {
        headerInitialised = true;
        refreshWalletHeader();
    }

    onMount(() => {
        const refreshRuntimeStatus = () => {
            if (!tauriReady || document.hidden) return;
            loadWalletStatus(true);
        };
        const onVisibilityChange = () => refreshRuntimeStatus();
        const timer = window.setInterval(refreshRuntimeStatus, 3000);
        window.addEventListener("focus", refreshRuntimeStatus);
        document.addEventListener("visibilitychange", onVisibilityChange);
        return () => {
            window.clearInterval(timer);
            window.removeEventListener("focus", refreshRuntimeStatus);
            document.removeEventListener("visibilitychange", onVisibilityChange);
        };
    });

    onDestroy(() => {
        clearExportDangerState();
        exportEncPass = "";
        exportEncPassConfirm = "";
        unlockPassword = "";
        unlockAfterWalletUnlock = null;
        pendingImportData = null;
        keyList = [];
        migrationExportPass = "";
        migrationValidatePass = "";
        migrationRestorePass = "";
        migrationRestoreConfirm = "";
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
            const content = await core.invoke("dialog_read_text_file", {
                title: "Select Key File",
                filters: [["JSON Key File", "json"]],
            });
            if (!content) {
                showKeyModal = false;
                return;
            }

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
        unlockModalPurpose = "file_decrypt";
        unlockAfterWalletUnlock = null;
        unlockingFile = true; // Important: Set BEFORE showing modal
        showUnlockModal = true;
    }

    let unlockingFile = false;

    function isWalletUnlockError(err) {
        const text = String(err || "").toLowerCase();
        return text.includes("walletpassphrase")
            || text.includes("please enter the wallet passphrase")
            || /wallet.*locked|passphrase|unlock/i.test(text);
    }

    function requestCoreWalletUnlockForVaultBackup() {
        unlockingFile = false;
        unlockModalPurpose = "vault_backup";
        unlockAfterWalletUnlock = async () => {
            await executeVaultExport();
        };
        unlockPassword = "";
        unlockError = "";
        showUnlockModal = true;
    }

    function closeUnlockModal() {
        showUnlockModal = false;
        unlockPassword = "";
        unlockError = "";
        unlockAfterWalletUnlock = null;
        unlockModalPurpose = "key_export";
        unlockingFile = false;
    }

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
                duration: unlockModalPurpose === "vault_backup" ? 300 : 60,
            });
            showUnlockModal = false;
            const unlockDuration = unlockModalPurpose === "vault_backup" ? 300 : 60;
            if (walletStatus && isRuntimeWalletEncrypted(walletStatus)) {
                walletStatus = {
                    ...walletStatus,
                    unlocked_until: Math.floor(Date.now() / 1000) + unlockDuration,
                };
            }
            const afterUnlock = unlockAfterWalletUnlock;
            unlockAfterWalletUnlock = null;
            const unlockedForVaultBackup = unlockModalPurpose === "vault_backup";
            unlockModalPurpose = "key_export";
            unlockPassword = "";
            if (afterUnlock) {
                await afterUnlock();
            } else {
                proceedExport();
            }
            if (unlockedForVaultBackup) {
                await refreshWalletHeader();
            }
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
        unlockModalPurpose = "key_export";
        unlockAfterWalletUnlock = null;
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
            const result = await core.invoke("dialog_write_text_file", {
                content: finalContent,
                defaultPath: `hemp0x_keys_${ts}.json`,
                title: "Save Keys",
                filters: [["JSON Key File", "json"]],
            });

            if (result) {
                showToast("Keys Exported Successfully", "success", false);
                addToolNotification("Private keys exported", `${selected.length} keys exported to file`, "success");
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

    // --- UNIFIED VAULT WALLET RECORDS ---
    let vaultWalletRecords = [];
    let vaultWalletRecordsLoading = false;
    let vaultWalletRecordsLoaded = false;
    let vaultWalletRecordsError = "";
    let vaultWalletRecordsMsg = "";

    let vaultListPassphrase = "";
    let selectedRecordId = "";

    let vaultImportPath = "";
    let vaultImportLabel = "";
    let vaultImportPassphrase = "";
    let vaultImportMigrationPassphrase = "";
    let vaultImportWorking = false;

    let vaultExportLabel = "";
    let vaultExportPassphrase = "";
    let vaultExportVaultPassphrase = "";
    let vaultExportWorking = false;
    let vaultExportRecoveryMode = "vault_passphrase"; // "vault_passphrase" or "separate_passphrase"
    let vaultExportAdvancedRecovery = false;

    let vaultRestoreRecordId = "";
    let vaultRestoreWalletName = "";
    let vaultRestorePassphrase = "";
    let vaultRestoreVaultPassphrase = "";
    let vaultRestoreBirth = "";
    let vaultRestoreConfirm = "";
    let vaultRestoreWorking = false;
    let vaultRestoreAdvanced = false;
    const VAULT_RESTORE_CONFIRM = "RESTORE WALLET";

    let vaultRemoveRecordId = "";
    let vaultRemovePassphrase = "";
    let vaultRemoveWorking = false;

    // --- VAULT-FIRST UX STATE ---
    let showAdvancedVaultRecords = false;
    let walletStatusError = "";
    let walletStatus = null; // { encrypted, unlocked, walletname, ... } or null
    let walletStatusLoading = false;
    let walletDatExists = null; // null = unknown, true/false from get_data_folder_info
    let vaultOverview = null;   // result of vault_get_vault_overview
    let vaultOverviewLoading = false;
    let vaultOverviewError = "";

    // --- STATE-DRIVEN VIEW STATE (60r) ---
    let activeTab = "dashboard";
    let showAdvancedRecovery = false;
    let vaultTabAutoSelected = false;

    // --- VAULT FILE MANAGER (60s) ---
    let vaultLabelInput = "";
    let vaultLabelSaving = false;
    let vaultLabelMsg = "";
    let vaultArchiveWorking = false;
    let vaultArchiveConfirm = "";
    let showVaultArchiveModal = false;
    const VAULT_ARCHIVE_CONFIRM = "BACK UP VAULT";
    let showVaultManagerPanel = false;

    $: hasVault = vaultOverview?.exists ?? false;
    $: hasBackups = vaultWalletRecords.length > 0;

    // When vault unlocks, load records once. An empty backup list is a
    // valid loaded state, so track it separately to avoid a reload loop.
    $: if (vaultUnlocked && hasVault && !vaultWalletRecordsLoaded && !vaultWalletRecordsLoading) {
        loadVaultWalletRecords();
    }

    // Default tab picker when vault becomes unlocked. Run once per
    // vault session so normal status refreshes do not keep moving the
    // user between tabs or add visible jank when returning to Wallet.
    $: if (vaultUnlocked && hasVault && !vaultTabAutoSelected && vaultWalletRecordsLoaded && walletDatExists !== null) {
        vaultTabAutoSelected = true;
        if (walletDatExists === false && hasBackups) {
            activeTab = "restore";
        } else if (!hasBackups) {
            activeTab = "backup";
        } else {
            activeTab = "dashboard";
        }
    }

    async function importVaultBundle() {
        if (!tauriReady) return;
        try {
            const selected = await open({
                title: "Select Vault Backup to Import",
                multiple: false,
                filters: [{ name: "Vault Bundle", extensions: ["json"] }],
            });
            if (!selected) return;
            let validation = null;
            try {
                validation = await core.invoke("vault_validate_import_bundle", { path: selected });
            } catch (err) {
                showToast("Invalid vault file: " + err, "error");
                return;
            }
            if (hasVault) {
                // Archive-first: do not silently overwrite the current vault.
                openModal(
                    "REPLACE CURRENT VAULT?",
                    `This will replace your current vault with the imported bundle (v${validation.version}, ${validation.network}). Your current vault file will be backed up first so it is preserved on disk.`,
                    [
                        {
                            label: "BACK UP + IMPORT",
                            style: "primary",
                            onClick: async () => {
                                closeModal();
                                await performSwitchImportArchiveFirst(selected);
                            },
                        },
                        {
                            label: "CANCEL",
                            style: "ghost",
                            onClick: closeModal,
                        },
                    ],
                );
            } else {
                await executeVaultBundleImport(selected);
            }
        } catch (err) {
            showToast("Import failed: " + err, "error");
        }
    }

    async function executeVaultBundleImport(path) {
        try {
            const result = await core.invoke("ipfs_vault_import_bundle_replace", {
                path,
                passphrase: null,
            });
            showToast(`Vault imported (v${result.version}, ${result.network}). Unlock with the imported vault passphrase.`, "success");
            await refreshWalletHeader();
        } catch (err) {
            showToast("Vault import failed: " + err, "error");
        }
    }

    // --- VAULT SESSION (60q) ---
    // The Wallet page can use the cached vault unlock session (set by
    // ipfs_unlock_vault or ipfs_vault_setup_and_unlock) so the user
    // does not have to re-enter the vault passphrase on every backup
    // or restore. The cached passphrase itself never leaves the
    // backend; we only see `vaultUnlocked: boolean` over IPC.
    let vaultUnlocked = false;
    let vaultUnlockedLoading = false;

    // Inline create-vault form state
    let createVaultPassphrase = "";
    let createVaultPassphraseConfirm = "";
    let createVaultWorking = false;
    let createVaultError = "";

    // Inline unlock-vault form state
    let unlockVaultPassphrase = "";
    let unlockVaultWorking = false;
    let unlockVaultError = "";

    // "Finish vault backup" post-new-wallet nudge
    let showFinishBackupNudge = false;

    async function loadWalletStatus(background = false) {
        if (!tauriReady) return;
        if (!background) {
            walletStatusLoading = true;
            walletStatusError = "";
        }
        try {
            const res = await core.invoke("rpc_get_wallet_info");
            if (res && res.success) {
                walletStatus = res.data;
            } else {
                walletStatus = null;
                walletStatusError = res?.error || "Wallet info unavailable";
            }
        } catch (err) {
            walletStatus = null;
            walletStatusError = String(err);
        }
        if (!background) {
            walletStatusLoading = false;
        }
    }

    function isRuntimeWalletEncrypted(status) {
        return status && Object.prototype.hasOwnProperty.call(status, "unlocked_until");
    }

    function isRuntimeWalletUnlocked(status) {
        const unlockedUntil = Number(status?.unlocked_until ?? 0);
        return isRuntimeWalletEncrypted(status) && unlockedUntil > 0;
    }

    async function loadWalletDatExists() {
        if (!tauriReady) return;
        try {
            const info = await core.invoke("get_data_folder_info");
            walletDatExists = !!info?.wallet_exists;
        } catch (err) {
            walletDatExists = null;
        }
    }

    async function loadVaultOverview() {
        if (!tauriReady) return;
        vaultOverviewLoading = true;
        vaultOverviewError = "";
        try {
            vaultOverview = await core.invoke("vault_get_vault_overview");
        } catch (err) {
            vaultOverview = null;
            vaultOverviewError = String(err);
        }
        vaultOverviewLoading = false;
    }

    async function refreshWalletHeader() {
        await Promise.all([
            loadWalletStatus(),
            loadWalletDatExists(),
            loadVaultOverview(),
            loadVaultUnlockStatus(),
        ]);
    }

    async function loadVaultUnlockStatus() {
        if (!tauriReady) return;
        vaultUnlockedLoading = true;
        try {
            const res = await core.invoke("ipfs_vault_unlock_status");
            vaultUnlocked = !!(res && res.unlocked);
        } catch (err) {
            // Non-fatal; treat as locked.
            vaultUnlocked = false;
        }
        vaultUnlockedLoading = false;
    }

    async function executeCreateVault() {
        if (!createVaultPassphrase || createVaultPassphrase.length < 8) {
            createVaultError = "Vault passphrase must be at least 8 characters.";
            return;
        }
        if (createVaultPassphrase !== createVaultPassphraseConfirm) {
            createVaultError = "Passphrases do not match.";
            return;
        }
        createVaultWorking = true;
        createVaultError = "";
        try {
            await core.invoke("ipfs_vault_setup_and_unlock", {
                passphrase: createVaultPassphrase,
            });
            createVaultPassphrase = "";
            createVaultPassphraseConfirm = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await refreshWalletHeader();
            showToast("Vault created and unlocked for this session.", "success");
        } catch (err) {
            createVaultError = String(err);
        }
        createVaultWorking = false;
    }

    async function executeUnlockVault() {
        if (!unlockVaultPassphrase) {
            unlockVaultError = "Enter your vault passphrase.";
            return;
        }
        unlockVaultWorking = true;
        unlockVaultError = "";
        try {
            const ok = await core.invoke("ipfs_unlock_vault", {
                passphrase: unlockVaultPassphrase,
            });
            if (ok) {
                unlockVaultPassphrase = "";
                await refreshWalletHeader();
                await loadVaultWalletRecords();
                showToast("Vault unlocked for this session.", "success");
            } else {
                unlockVaultError = "Incorrect passphrase.";
            }
        } catch (err) {
            unlockVaultError = String(err);
        }
        unlockVaultWorking = false;
    }

    async function executeLockVault() {
        try {
            await core.invoke("ipfs_lock_vault");
            vaultUnlocked = false;
            // Also clear any local cached unlock that the Wallet page
            // was keeping for the list. The user is locking on purpose.
            vaultListPassphrase = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            showToast("Vault locked.", "info");
        } catch (err) {
            showToast("Lock failed: " + err, "error");
        }
    }

    // --- VAULT FILE MANAGER HELPERS (60s) ---

    function vaultBasenameFromPath(p) {
        if (!p) return "vault.json";
        const parts = String(p).split(/[/\\]/).filter(Boolean);
        return parts.length ? parts[parts.length - 1] : String(p);
    }

    function formatBytes(n) {
        const num = Number(n);
        if (!num || num < 0) return "—";
        if (num < 1024) return `${num} B`;
        if (num < 1024 * 1024) return `${(num / 1024).toFixed(1)} KB`;
        return `${(num / (1024 * 1024)).toFixed(2)} MB`;
    }

    function formatUnixShort(ts) {
        const n = Number(ts);
        if (!n) return "—";
        try {
            return new Date(n * 1000).toLocaleString();
        } catch {
            return "—";
        }
    }

    async function saveVaultLabel() {
        vaultLabelSaving = true;
        vaultLabelMsg = "";
        try {
            const res = await core.invoke("vault_set_vault_label", {
                label: vaultLabelInput,
            });
            vaultLabelMsg = res?.active_label
                ? `Saved as “${res.active_label}”.`
                : "Label cleared.";
            // Sync the input box with the cleaned value the backend kept.
            vaultLabelInput = res?.active_label ?? "";
            await refreshWalletHeader();
        } catch (err) {
            vaultLabelMsg = `Could not save label: ${err}`;
        }
        vaultLabelSaving = false;
    }

    // Keep the label input in sync with whatever the backend has stored
    // for the current vault. Runs whenever the overview changes.
    $: {
        if (vaultOverview && vaultOverview.display_label !== undefined) {
            const fromBackend = vaultOverview.display_label ?? "";
            if (!vaultLabelInput || vaultLabelInput === vaultLabelLastSynced) {
                vaultLabelInput = fromBackend;
                vaultLabelLastSynced = fromBackend;
            }
        }
    }
    let vaultLabelLastSynced = "";

    function promptArchiveVault() {
        vaultArchiveConfirm = "";
        showVaultArchiveModal = true;
    }

    async function executeArchiveVault() {
        if (vaultArchiveConfirm !== VAULT_ARCHIVE_CONFIRM) {
            showToast(`Type ${VAULT_ARCHIVE_CONFIRM} to confirm.`, "warning");
            return;
        }
        vaultArchiveWorking = true;
        try {
            const res = await core.invoke("vault_archive_current_vault");
            const fileName = vaultBasenameFromPath(res?.archive_path);
            showToast(
                `Vault backed up as ${fileName}. You can create a new vault or import a different one.`,
                "success",
            );
            showVaultArchiveModal = false;
            vaultArchiveConfirm = "";
            // Drop any UI state that pointed at the now-archived vault.
            vaultUnlocked = false;
            vaultListPassphrase = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultLabelInput = "";
            vaultLabelLastSynced = "";
            await refreshWalletHeader();
        } catch (err) {
            showToast("Archive failed: " + err, "error");
        }
        vaultArchiveWorking = false;
    }

    async function createNewVaultAfterArchive(passphrase) {
        try {
            const res = await core.invoke("vault_archive_current_vault");
            const fileName = vaultBasenameFromPath(res?.archive_path);
            showToast(`Previous vault saved as ${fileName}.`, "info");
            await core.invoke("ipfs_vault_setup_and_unlock", { passphrase });
            showToast("New vault created and unlocked for this session.", "success");
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await refreshWalletHeader();
        } catch (err) {
            showToast("Could not create new vault: " + err, "error");
        }
    }

    function promptCreateNewVaultFromLocked() {
        // Collect the new passphrase first, then archive + create.
        openModal(
            "CREATE NEW VAULT",
            hasVault
                ? "Commander will save the current vault first, then create a new active vault."
                : "Create a new encrypted vault for wallet backups and optional app secrets.",
            [
                {
                    label: "CONTINUE",
                    style: "primary",
                    onClick: () => {
                        closeModal();
                        showCreateVaultForm = true;
                    },
                },
                { label: "CANCEL", style: "ghost", onClick: closeModal },
            ],
        );
    }

    let showCreateVaultForm = false;
    let createVaultFormPass = "";
    let createVaultFormPassConfirm = "";
    let createVaultFormWorking = false;
    let createVaultFormError = "";

    async function submitCreateVaultForm() {
        if (createVaultFormPass.length < 8) {
            createVaultFormError = "Vault passphrase must be at least 8 characters.";
            return;
        }
        if (createVaultFormPass !== createVaultFormPassConfirm) {
            createVaultFormError = "Passphrases do not match.";
            return;
        }
        createVaultFormWorking = true;
        createVaultFormError = "";
        try {
            if (hasVault) {
                await createNewVaultAfterArchive(createVaultFormPass);
            } else {
                await core.invoke("ipfs_vault_setup_and_unlock", {
                    passphrase: createVaultFormPass,
                });
                showToast("Vault created and unlocked for this session.", "success");
                vaultWalletRecords = [];
                vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await refreshWalletHeader();
            }
            showCreateVaultForm = false;
            createVaultFormPass = "";
            createVaultFormPassConfirm = "";
        } catch (err) {
            createVaultFormError = String(err);
        }
        createVaultFormWorking = false;
    }

    // Switch/import with archive-first behavior. If a vault currently
    // exists, we archive it before importing the selected bundle so the
    // user does not silently lose it.
    let switchImportArchiveArmed = false;
    let switchImportPendingPath = "";

    async function switchOrImportVault() {
        if (!tauriReady) return;
        try {
            const selected = await open({
                title: "Select Vault Bundle to Switch / Import",
                multiple: false,
                filters: [{ name: "Vault Bundle", extensions: ["json"] }],
            });
            if (!selected) return;
            let validation = null;
            try {
                validation = await core.invoke("vault_validate_import_bundle", { path: selected });
            } catch (err) {
                showToast("Invalid vault file: " + err, "error");
                return;
            }
            if (hasVault) {
                openModal(
                    "REPLACE CURRENT VAULT?",
                    `The selected vault will become the active vault. Commander will save the current vault first so it can be imported again later.`,
                    [
                        {
                            label: "BACK UP + IMPORT",
                            style: "primary",
                            onClick: async () => {
                                closeModal();
                                await performSwitchImportArchiveFirst(selected);
                            },
                        },
                        { label: "CANCEL", style: "ghost", onClick: closeModal },
                    ],
                );
            } else {
                await performSwitchImportArchiveFirst(selected);
            }
        } catch (err) {
            showToast("Import failed: " + err, "error");
        }
    }

    async function performSwitchImportArchiveFirst(path) {
        try {
            if (hasVault) {
                const archiveRes = await core.invoke("vault_archive_current_vault");
                const fileName = vaultBasenameFromPath(archiveRes?.archive_path);
                showToast(`Current vault saved as ${fileName}.`, "info");
            }
            const result = await core.invoke("ipfs_vault_import_bundle_replace", {
                path,
                passphrase: null,
            });
            showToast(
                `Vault imported (v${result.version}, ${result.network}). Unlock with the imported vault passphrase.`,
                "success",
            );
            vaultUnlocked = false;
            vaultListPassphrase = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await refreshWalletHeader();
        } catch (err) {
            showToast("Switch / import failed: " + err, "error");
        }
    }

    function selectRecord(recordId) {
        selectedRecordId = recordId;
        vaultRestoreRecordId = recordId;
        vaultRemoveRecordId = recordId;
        // Friendly defaults when selecting a backup to restore
        if (vaultRestoreWalletName === "" || vaultRestoreWalletName === "default") {
            const rec = vaultWalletRecords.find((r) => r.record_id === recordId);
            const hint = rec?.metadata?.wallet_name_hint;
            if (hint && typeof hint === "string") {
                vaultRestoreWalletName = hint;
            }
        }
    }

    async function loadVaultWalletRecords() {
        vaultWalletRecordsLoading = true;
        vaultWalletRecordsError = "";
        try {
            // Prefer the cached vault session if unlocked; fall back to
            // the explicit per-call passphrase field for users who
            // deliberately keep the vault locked and re-enter it
            // each time.
            const explicit = vaultListPassphrase && !vaultUnlocked ? vaultListPassphrase : null;
            vaultWalletRecords = await core.invoke(
                "ipfs_vault_list_wallet_migration_records",
                { vaultPassphrase: explicit },
            );
        } catch (err) {
            vaultWalletRecordsError = String(err);
            vaultWalletRecords = [];
        }
        vaultWalletRecordsLoading = false;
        vaultWalletRecordsLoaded = true;
    }

    async function vaultImportMigrationFile() {
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const selected = await open({
                title: "Select Migration Envelope File",
                multiple: false,
                filters: [{ name: "Migration Files", extensions: ["json"] }],
            });
            if (!selected) return;
            vaultImportPath = selected;
        } catch (err) {
            vaultWalletRecordsError = String(err);
        }
    }

    async function executeVaultImport() {
        if (!vaultImportPath || !vaultImportLabel) {
            vaultWalletRecordsError = "File and label are required.";
            return;
        }
        // If the vault is unlocked, no per-call vault passphrase is
        // required. Otherwise the user must supply one explicitly.
        const explicit = vaultUnlocked ? null : (vaultImportPassphrase || null);
        if (!vaultUnlocked && !vaultImportPassphrase) {
            vaultWalletRecordsError = "Vault passphrase is required (or unlock the vault first).";
            return;
        }
        vaultImportWorking = true;
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const result = await core.invoke(
                "ipfs_vault_import_wallet_migration_record_from_path",
                {
                    path: vaultImportPath,
                    label: vaultImportLabel,
                    migrationPassphrase: vaultImportMigrationPassphrase || null,
                    vaultPassphrase: explicit,
                },
            );
            vaultWalletRecordsMsg = `Imported: ${result.label} (${result.record_id})`;
            vaultImportPath = "";
            vaultImportLabel = "";
            vaultImportPassphrase = "";
            vaultImportMigrationPassphrase = "";
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await loadVaultWalletRecords();
        } catch (err) {
            vaultWalletRecordsError = String(err);
        }
        vaultImportWorking = false;
    }

    async function executeVaultExport() {
        if (!vaultExportLabel) {
            vaultWalletRecordsError = "Label is required.";
            return;
        }
        if (vaultExportAdvancedRecovery && !vaultExportPassphrase) {
            vaultWalletRecordsError = "Wallet backup passphrase is required with separate recovery password.";
            return;
        }
        const explicit = vaultUnlocked ? null : (vaultExportVaultPassphrase || null);
        if (!vaultUnlocked && !vaultExportVaultPassphrase) {
            vaultWalletRecordsError = "Vault passphrase is required (or unlock the vault first).";
            return;
        }
        vaultExportWorking = true;
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const recoveryMode = vaultExportAdvancedRecovery ? "separate_passphrase" : "vault_passphrase";
            const result = await core.invoke(
                "ipfs_vault_export_current_wallet_migration_record",
                {
                    label: vaultExportLabel,
                    migrationPassphrase: vaultExportAdvancedRecovery ? vaultExportPassphrase : "",
                    vaultPassphrase: explicit,
                    recoveryMode,
                },
            );
            vaultWalletRecordsMsg = `Exported: ${result.label} (${result.record_id})`;
            vaultExportLabel = "";
            vaultExportPassphrase = "";
            vaultExportVaultPassphrase = "";
            vaultExportAdvancedRecovery = false;
            vaultExportRecoveryMode = "vault_passphrase";
            showFinishBackupNudge = false;
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await loadVaultWalletRecords();
        } catch (err) {
            if (isWalletUnlockError(err)) {
                vaultWalletRecordsError = "Core wallet unlock required before backing up to the vault.";
                requestCoreWalletUnlockForVaultBackup();
            } else {
            vaultWalletRecordsError = String(err);
            }
        }
        vaultExportWorking = false;
    }

    async function executeVaultRestore() {
        if (vaultRestoreConfirm !== VAULT_RESTORE_CONFIRM) {
            vaultWalletRecordsError = "Type RESTORE WALLET to confirm.";
            return;
        }
        if (!vaultRestoreRecordId || !vaultRestoreWalletName) {
            vaultWalletRecordsError = "Select a backup and set a wallet name.";
            return;
        }
        const selectedRec = vaultWalletRecords.find((r) => r.record_id === vaultRestoreRecordId);
        const recRecoveryMode = selectedRec?.metadata?.recovery_mode;
        const needsMigrationPass = !recRecoveryMode || recRecoveryMode !== "vault_passphrase";
        if (needsMigrationPass && !vaultRestorePassphrase) {
            vaultWalletRecordsError = "Wallet backup passphrase is required for this record.";
            return;
        }
        const explicit = vaultUnlocked ? null : (vaultRestoreVaultPassphrase || null);
        if (!vaultUnlocked && !vaultRestoreVaultPassphrase) {
            vaultWalletRecordsError = "Vault passphrase is required (or unlock the vault first).";
            return;
        }
        vaultRestoreWorking = true;
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const result = await core.invoke(
                "ipfs_vault_restore_wallet_migration_record",
                {
                    recordId: vaultRestoreRecordId,
                    walletName: vaultRestoreWalletName,
                    migrationPassphrase: needsMigrationPass ? vaultRestorePassphrase : "",
                    birthHeight: vaultRestoreBirth ? Number(vaultRestoreBirth) : null,
                    vaultPassphrase: explicit,
                },
            );
            vaultWalletRecordsMsg = `Wallet restored: ${result.wallet_name}. Restart the node to use it.`;
            vaultRestoreRecordId = "";
            vaultRestoreWalletName = "";
            vaultRestorePassphrase = "";
            vaultRestoreVaultPassphrase = "";
            vaultRestoreBirth = "";
            vaultRestoreConfirm = "";
            walletDatExists = null;
            await loadWalletDatExists();
        } catch (err) {
            vaultWalletRecordsError = String(err);
        }
        vaultRestoreWorking = false;
    }

    async function executeVaultRemove() {
        if (!vaultRemoveRecordId) {
            vaultWalletRecordsError = "Select a backup record first.";
            return;
        }
        const explicit = vaultUnlocked ? null : (vaultRemovePassphrase || null);
        if (!vaultUnlocked && !vaultRemovePassphrase) {
            vaultWalletRecordsError = "Vault passphrase is required (or unlock the vault first).";
            return;
        }
        vaultRemoveWorking = true;
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const result = await core.invoke(
                "ipfs_vault_remove_wallet_migration_record",
                {
                    recordId: vaultRemoveRecordId,
                    vaultPassphrase: explicit,
                },
            );
            vaultWalletRecordsMsg = `Removed: ${result.label} (${result.record_id})`;
            vaultRemoveRecordId = "";
            vaultRemovePassphrase = "";
            selectedRecordId = "";
            vaultWalletRecordsLoaded = false;
            vaultTabAutoSelected = false;
            await loadVaultWalletRecords();
        } catch (err) {
            vaultWalletRecordsError = String(err);
        }
        vaultRemoveWorking = false;
    }

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
            showToast("Migration package exported successfully", "success", false);
            addToolNotification(
                "Wallet migration exported",
                migrationExportPrivate ? "Private migration package exported" : "Public migration package exported",
                "success",
            );
        } catch (e) {
            migrationError = String(e);
            showToast("Export failed: " + e, "error", false);
            addToolNotification("Wallet migration export failed", String(e).substring(0, 200), "error");
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
            showToast("Validation complete", "info", false);
            addToolNotification("Wallet migration validated", "", "success");
        } catch (e) {
            migrationError = String(e);
            showToast("Validation failed: " + e, "error", false);
            addToolNotification("Wallet migration validation failed", String(e).substring(0, 200), "error");
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
            showToast("Wallet restored successfully", "success", false);
            addToolNotification("Wallet migration restored", "", "success");
        } catch (e) {
            migrationError = String(e);
            showToast("Restore failed: " + e, "error", false);
            addToolNotification("Wallet migration restore failed", String(e).substring(0, 200), "error");
        }
        migrationWorking = false;
        migrationRestorePass = "";
        migrationRestoreConfirm = "";
    }

</script>

<!-- ================= STATE-DRIVEN WALLET PAGE (60r) ================= -->
{#if !tauriReady || vaultOverviewLoading || (vaultOverview === null && !vaultOverviewError)}
    <div class="glass-panel panel-soft" style="margin:1rem 0; padding:1.5rem; text-align:center;">
        <p style="color:#888; font-size:0.8rem;">Loading wallet status…</p>
    </div>
{:else if vaultOverviewError}
    <div class="glass-panel panel-soft" style="margin:1rem 0; padding:1.5rem; text-align:center;">
        <h2 style="color:#ffaa00; font-size:0.9rem; margin:0 0 0.5rem;">WALLET STATUS UNAVAILABLE</h2>
        <p style="color:#888; font-size:0.75rem; margin:0 0 1rem;">{vaultOverviewError}</p>
        <button class="cyber-btn ghost small" on:click={refreshWalletHeader}>RETRY</button>
    </div>
{:else if !hasVault}
    <!-- 1. NO VAULT / FIRST SETUP -->
    <div in:fly={{ y: 12, duration: 250 }}>
        <div class="glass-panel panel-soft" style="margin:0.5rem 0; padding:0;">
            <div style="padding:1.25rem 1.5rem 0.75rem; text-align:center;">
                <div style="font-size:1.4rem; margin-bottom:0.35rem; opacity:0.8;">🔐</div>
                <h2 style="color:var(--color-primary); font-size:1rem; margin:0 0 0.35rem; letter-spacing:1px;">WALLET SETUP</h2>
                <p style="color:#888; font-size:0.75rem; max-width:480px; margin:0 auto 0.5rem; line-height:1.5;">
                    Create a secure encrypted Vault — your portable Commander wallet container.
                </p>
                <p style="color:#555; font-size:0.65rem; max-width:480px; margin:0 auto;">
                    Core creates runtime wallet files on disk for compatibility.
                </p>
            </div>

            <div style="padding:0 1.25rem 1rem; display:flex; flex-direction:column; gap:0.6rem; max-width:480px; margin:0 auto;">
                <!-- Create -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">CREATE NEW</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Set a strong passphrase. This encrypts your local Vault file.
                    </p>
                    <input
                        type="password"
                        class="input-glass"
                        placeholder="New passphrase (min 8 chars)"
                        bind:value={createVaultPassphrase}
                        style="font-size:0.75rem; padding:0.45rem; margin-bottom:0.3rem; width:100%; box-sizing:border-box;"
                    />
                    <input
                        type="password"
                        class="input-glass"
                        placeholder="Confirm passphrase"
                        bind:value={createVaultPassphraseConfirm}
                        style="font-size:0.75rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;"
                    />
                    <p class="desc" style="margin:0 0 0.5rem; color:#ff8888; font-size:0.6rem; line-height:1.4;">
                        <strong>Important:</strong> If you lose your passphrase, your encrypted data cannot be recovered.
                    </p>
                    <button
                        class="cyber-btn small wide"
                        on:click={executeCreateVault}
                        disabled={createVaultWorking || !createVaultPassphrase || createVaultPassphrase.length < 8 || createVaultPassphrase !== createVaultPassphraseConfirm}
                    >
                        {createVaultWorking ? "CREATING…" : "CREATE"}
                    </button>
                    {#if createVaultError}
                        <p style="color:#ff5555; font-size:0.7rem; margin:0.3rem 0 0;">{createVaultError}</p>
                    {/if}
                </div>

                <!-- Import -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">IMPORT BACKUP</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Restore an existing <code>vault.json</code> bundle from a file.
                    </p>
                    <button class="cyber-btn ghost small wide" on:click={importVaultBundle}>
                        IMPORT FROM FILE
                    </button>
                </div>

                <!-- Advanced Recovery (collapsed) -->
                <button
                    class="toggle"
                    style="background:none; border:none; color:#666; font-size:0.65rem; padding:0.25rem 0; cursor:pointer; text-align:center; letter-spacing:0.5px;"
                    on:click={() => (showAdvancedRecovery = !showAdvancedRecovery)}
                >
                    {showAdvancedRecovery ? "▲" : "▼"} ADVANCED RECOVERY
                </button>
                {#if showAdvancedRecovery}
                    <div style="display:flex; flex-direction:column; gap:0.4rem;">
                        <button class="cyber-btn ghost tiny wide" on:click={restoreWallet}>
                            RESTORE LEGACY WALLET.DAT
                        </button>
                        <div class="btn-row" style="gap:0.4rem;">
                            <button class="cyber-btn ghost tiny wide" on:click={openExportModal}>EXPORT KEYS</button>
                            <button class="cyber-btn ghost tiny wide" on:click={openImportModal}>IMPORT KEYS</button>
                        </div>
                    </div>
                {/if}

                <!-- Short info -->
                <div style="background:rgba(0,0,0,0.15); border:1px solid rgba(255,255,255,0.04); border-radius:5px; padding:0.6rem 0.85rem; margin-top:0.15rem;">
                    <div style="font-size:0.6rem; color:#666; display:flex; flex-direction:column; gap:0.25rem; line-height:1.4;">
                        <span><strong style="color:#888;">Encrypted Vault</strong> — recommended portable encrypted backup</span>
                        <span><strong style="color:#888;">wallet.dat</strong> — legacy Core runtime wallet</span>
                        <span><strong style="color:#888;">Private key import/export</strong> — advanced single-key recovery</span>
                    </div>
                </div>
            </div>
        </div>
    </div>
{:else if hasVault && !vaultUnlocked}
    <!-- 3. LOCKED -->
    <div in:fly={{ y: 12, duration: 250 }}>
        <div class="glass-panel panel-soft" style="margin:0.5rem 0; padding:0;">
            <div style="padding:1.25rem; text-align:center;">
                <div style="font-size:1.4rem; margin-bottom:0.35rem; opacity:0.8;">🔒</div>
                <h2 style="color:var(--color-primary); font-size:1rem; margin:0 0 0.35rem; letter-spacing:1px;">SECURE VAULT LOCKED</h2>
                <p style="color:#888; font-size:0.75rem; max-width:420px; margin:0 auto 1.25rem; line-height:1.5;">
                    Unlock to view and manage your wallet backups. Your passphrase is held in memory only for this session.
                </p>

                <div style="max-width:340px; margin:0 auto; display:flex; flex-direction:column; gap:0.5rem;">
                    <input
                        type="password"
                        class="input-glass"
                        placeholder="Enter your passphrase"
                        bind:value={unlockVaultPassphrase}
                        on:keydown={(e) => e.key === 'Enter' && executeUnlockVault()}
                        style="font-size:0.8rem; padding:0.55rem;"
                    />
                    <button
                        class="cyber-btn small wide"
                        on:click={executeUnlockVault}
                        disabled={unlockVaultWorking || !unlockVaultPassphrase}
                    >
                        {unlockVaultWorking ? "UNLOCKING…" : "UNLOCK"}
                    </button>
                    {#if unlockVaultError}
                        <p style="color:#ff5555; font-size:0.7rem; margin:0.15rem 0 0;">{unlockVaultError}</p>
                    {/if}
                </div>

                <div style="margin-top:1.25rem; display:flex; justify-content:center; gap:0.4rem; flex-wrap:wrap;">
                    <button class="cyber-btn ghost tiny" on:click={importVaultBundle}>SWITCH / IMPORT</button>
                    <button class="cyber-btn ghost tiny" on:click={promptCreateNewVaultFromLocked}>CREATE NEW</button>
                    <button class="cyber-btn ghost tiny" on:click={promptArchiveVault}>SAVE VAULT</button>
                </div>
                {#if showAdvancedRecovery}
                    <div style="max-width:340px; margin:0.5rem auto 0; display:flex; flex-direction:column; gap:0.35rem;">
                        <button class="cyber-btn ghost tiny wide" on:click={restoreWallet}>RESTORE LEGACY WALLET.DAT</button>
                        <div class="btn-row" style="gap:0.35rem;">
                            <button class="cyber-btn ghost tiny wide" on:click={openExportModal}>EXPORT KEYS</button>
                            <button class="cyber-btn ghost tiny wide" on:click={openImportModal}>IMPORT KEYS</button>
                        </div>
                    </div>
                {/if}
                <div style="margin-top:0.6rem;">
                    <button
                        class="toggle"
                        style="background:none; border:none; color:#666; font-size:0.65rem; padding:0; cursor:pointer; letter-spacing:0.5px;"
                        on:click={() => (showAdvancedRecovery = !showAdvancedRecovery)}
                    >
                        {showAdvancedRecovery ? "▲" : "▼"} ADVANCED RECOVERY
                    </button>
                </div>

                <!-- Active File card -->
                <div style="max-width:480px; margin:1.25rem auto 0; text-align:left; background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.7rem 0.85rem;">
                    <div style="display:flex; align-items:center; justify-content:space-between; gap:0.5rem; margin-bottom:0.3rem;">
                        <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem; letter-spacing:0.5px;">ACTIVE FILE</h4>
                        <button
                            class="toggle"
                            style="background:none; border:none; color:#888; font-size:0.6rem; padding:0; cursor:pointer;"
                            on:click={() => (showVaultManagerPanel = !showVaultManagerPanel)}
                        >
                            {showVaultManagerPanel ? "▲" : "▼"} DETAILS
                        </button>
                    </div>
                    <div style="display:flex; align-items:center; gap:0.4rem; flex-wrap:wrap; font-size:0.7rem; color:#aaa;">
                        <span style="color:#888;">Loaded:</span>
                        <span title={vaultOverview?.vault_path ?? ""} style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:200px;">
                            {vaultBasenameFromPath(vaultOverview?.vault_path)}
                        </span>
                        {#if vaultOverview?.display_label}
                            <span style="color:var(--color-primary);">"{vaultOverview.display_label}"</span>
                        {/if}
                        <span style="color:#666;">•</span>
                        <span>{formatUnixShort(vaultOverview?.file_modified || vaultOverview?.modified)}</span>
                    </div>
                    {#if showVaultManagerPanel}
                        <div style="margin-top:0.5rem; border-top:1px dashed rgba(255,255,255,0.06); padding-top:0.5rem; display:flex; flex-direction:column; gap:0.4rem;">
                            <div style="display:grid; grid-template-columns:auto 1fr; gap:0.2rem 0.5rem; font-size:0.65rem; color:#aaa;">
                                {#if vaultOverview?.network}
                                    <span style="color:#888;">Network</span>
                                    <span>{vaultOverview.network} • v{vaultOverview.bundle_version}</span>
                                {/if}
                                <span style="color:#888;">Size</span>
                                <span>{formatBytes(vaultOverview?.file_size)}</span>
                            </div>
                            <div>
                                <label for="vault-label-locked" style="font-size:0.6rem; color:#888;">DISPLAY NAME</label>
                                <div style="display:flex; gap:0.35rem; margin-top:0.15rem;">
                                    <input
                                        id="vault-label-locked"
                                        type="text"
                                        class="input-glass"
                                        placeholder="e.g. Main Backup"
                                        bind:value={vaultLabelInput}
                                        style="font-size:0.65rem; padding:0.35rem; flex:1;"
                                    />
                                    <button
                                        class="cyber-btn tiny"
                                        on:click={saveVaultLabel}
                                        disabled={vaultLabelSaving || (vaultLabelInput === (vaultOverview?.display_label ?? ""))}
                                    >
                                        {vaultLabelSaving ? "…" : "SAVE"}
                                    </button>
                                </div>
                                {#if vaultLabelMsg}
                                    <p style="color:#888; font-size:0.55rem; margin:0.15rem 0 0;">{vaultLabelMsg}</p>
                                {/if}
                            </div>
                        </div>
                    {/if}
                </div>

                <!-- Page-level help panel -->
                <div style="max-width:480px; margin:0.75rem auto 0; text-align:left; background:rgba(0,0,0,0.15); border:1px solid rgba(255,255,255,0.04); border-radius:5px; padding:0.65rem 0.85rem;">
                    <div style="display:flex; align-items:center; gap:0.35rem; margin-bottom:0.3rem;">
                        <span style="font-size:0.8rem; opacity:0.7;">ℹ️</span>
                        <h4 style="color:var(--color-primary); margin:0; font-size:0.65rem; letter-spacing:0.5px;">ABOUT YOUR SECURE VAULT</h4>
                    </div>
                    <div style="font-size:0.65rem; color:#aaa; line-height:1.55; display:flex; flex-direction:column; gap:0.3rem;">
                        <p style="margin:0;">Your secure vault is your portable encrypted Commander wallet container. It holds wallet recovery backups and optional app credentials. Core manages runtime wallet files for compatibility.</p>
                        <p style="margin:0;"><strong style="color:#ffaa00;">Important:</strong> Your passphrase protects everything inside. If you lose it, your encrypted data cannot be recovered. Save it in a safe location.</p>
                        <p style="margin:0; color:#888;">Use <strong>Save Vault</strong> to create a dated backup of the vault file before switching or creating a new one.</p>
                    </div>
                </div>
            </div>
        </div>
    </div>
{:else}
    <!-- 4. UNLOCKED -->
    <div in:fly={{ y: 12, duration: 250 }}>
        <!-- Compact top status bar -->
        <div class="glass-panel panel-soft" style="margin:1rem 0; padding:0;">
            <header class="card-header" style="display:flex; align-items:center; justify-content:space-between; gap:0.5rem; flex-wrap:wrap;">
                <div style="display:flex; align-items:center; gap:0.6rem; flex-wrap:wrap;">
                    <span class="card-title">WALLET</span>
                    {#if walletStatus}
                        <span style="font-size:0.7rem; color:#aaa;">
                            {walletStatus.walletname ?? "default"}
                            <span style="color:#666;">|</span>
                            {#if isRuntimeWalletEncrypted(walletStatus)}
                                {#if isRuntimeWalletUnlocked(walletStatus)}
                                    <span style="color:var(--color-primary);">Unlocked</span>
                                {:else}
                                    <span style="color:#ffaa00;">Locked</span>
                                {/if}
                            {:else}
                                <span style="color:#888;">Unencrypted</span>
                            {/if}
                        </span>
                    {:else}
                        <span style="font-size:0.7rem; color:#666;">Loading…</span>
                    {/if}
                    <span style="font-size:0.65rem; color:#666;">•</span>
                    <span style="font-size:0.7rem; color:var(--color-primary);">Vault Unlocked</span>
                    <span style="font-size:0.65rem; color:#666;">•</span>
                    <span style="font-size:0.7rem; color:#aaa;">
                        {hasBackups ? `${vaultWalletRecords.length} backup${vaultWalletRecords.length === 1 ? "" : "s"}` : "No backups yet"}
                    </span>
                    {#if walletDatExists !== null}
                        <span style="font-size:0.65rem; color:#666;">•</span>
                        <span style="font-size:0.7rem; color:{walletDatExists ? '#aaa' : '#ffaa00'};">
                            Runtime {walletDatExists ? "ready" : "missing"}
                        </span>
                    {/if}
                </div>
                <button
                    class="cyber-btn ghost small"
                    on:click={refreshWalletHeader}
                    disabled={walletStatusLoading}
                >
                    {walletStatusLoading ? "…" : "REFRESH"}
                </button>
            </header>

            <!-- Recovery banners inside top panel -->
            <div style="padding:0.4rem 0.85rem;">
                {#if showFinishBackupNudge}
                    <div style="background:rgba(0,255,65,0.08); border:1px solid rgba(0,255,65,0.25); border-radius:4px; padding:0.5rem 0.75rem; color:var(--color-primary); font-size:0.7rem; display:flex; align-items:center; justify-content:space-between; gap:0.5rem; margin-bottom:0.5rem;">
                        <span>New runtime wallet created. Back up to vault so you can recover if wallet files are lost.</span>
                        <button class="cyber-btn ghost tiny" on:click={() => (showFinishBackupNudge = false)}>DISMISS</button>
                    </div>
                {/if}
                {#if walletDatExists === false && hasBackups}
                    <div style="background:rgba(255,170,0,0.08); border:1px solid rgba(255,170,0,0.3); border-radius:4px; padding:0.5rem 0.75rem; color:#ffaa00; font-size:0.7rem; margin-bottom:0.5rem;">
                        <strong>Runtime wallet missing.</strong> Pick a backup in the <strong>Restore</strong> tab to provision Core from your vault.
                    </div>
                {:else if walletDatExists === false && !hasBackups}
                    <div style="background:rgba(255,85,85,0.08); border:1px solid rgba(255,85,85,0.3); border-radius:4px; padding:0.5rem 0.75rem; color:#ff8888; font-size:0.7rem; margin-bottom:0.5rem;">
                        <strong>No runtime wallet and no backups.</strong> Create a new wallet or import a vault backup. See <strong>Advanced</strong>.
                    </div>
                {:else if walletDatExists === true && !hasBackups}
                    <div style="background:rgba(0,255,65,0.05); border:1px solid rgba(0,255,65,0.15); border-radius:4px; padding:0.5rem 0.75rem; color:#aaa; font-size:0.7rem; margin-bottom:0.5rem;">
                        No vault backups yet. Back up your current wallet in the <strong>Backup</strong> tab.
                    </div>
                {/if}
            </div>

            <!-- Active File row (60s) -->
            <div style="padding:0.25rem 1rem 0.6rem; display:flex; align-items:center; justify-content:space-between; gap:0.5rem; flex-wrap:wrap; font-size:0.65rem; color:#888; border-top:1px dashed rgba(255,255,255,0.06);">
                <div style="display:flex; align-items:center; gap:0.4rem; flex-wrap:wrap; min-width:0;">
                    <span style="color:#888; letter-spacing:0.5px;">ACTIVE FILE</span>
                    <span style="color:#666;">Loaded:</span>
                    <span title={vaultOverview?.vault_path ?? ""} style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:220px;">
                        {vaultBasenameFromPath(vaultOverview?.vault_path)}
                    </span>
                    {#if vaultOverview?.display_label}
                        <span style="color:var(--color-primary);">"{vaultOverview.display_label}"</span>
                    {/if}
                    {#if vaultOverview?.network}
                        <span>• {vaultOverview.network} / v{vaultOverview.bundle_version}</span>
                    {/if}
                </div>
                <div style="display:flex; gap:0.35rem; flex-wrap:wrap;">
                    <button class="cyber-btn ghost tiny" on:click={switchOrImportVault}>SWITCH / IMPORT</button>
                    <button class="cyber-btn ghost tiny" on:click={promptCreateNewVaultFromLocked}>NEW</button>
                    <button class="cyber-btn ghost tiny" on:click={promptArchiveVault}>SAVE VAULT</button>
                </div>
            </div>
            <div style="padding:0 1rem 0.5rem; font-size:0.55rem; color:#555; text-align:right;">
                <span title="Save Current Vault creates a dated backup of the vault.json file">Save Current Vault backs up the vault file. To back up wallet data, use the Backup tab.</span>
            </div>
        </div>

        <!-- Tab bar -->
        <div style="display:flex; gap:0.25rem; margin:0.75rem 0 0.5rem; flex-wrap:wrap;">
            {#each [{k:"dashboard",l:"Overview"},{k:"backup",l:"Backup"},{k:"restore",l:"Restore"},{k:"security",l:"Security"},{k:"advanced",l:"Advanced"}] as tab}
                <button
                    class="cyber-btn {activeTab === tab.k ? 'primary-glow' : 'ghost'} tiny"
                    style="text-transform:uppercase; letter-spacing:0.5px; font-size:0.7rem;"
                    on:click={() => (activeTab = tab.k)}
                >
                    {tab.l}
                </button>
            {/each}
        </div>

        <!-- DASHBOARD TAB -->
        {#if activeTab === "dashboard"}
            <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
                <div style="padding:1.25rem;">
                    <div style="display:flex; flex-direction:column; gap:0.6rem; font-size:0.75rem; color:#aaa;">
                        <div style="display:flex; gap:0.5rem; align-items:center;">
                            <span style="color:#888; min-width:6rem;">Vault</span>
                            <span style="color:var(--color-primary);">Unlocked</span>
                        </div>
                        <div style="display:flex; gap:0.5rem; align-items:center;">
                            <span style="color:#888; min-width:6rem;">Runtime wallet</span>
                            {#if walletStatusError}
                                <span style="color:#ffaa00;">Unavailable</span>
                            {:else if walletStatus}
                                <span>
                                    {walletStatus.walletname ?? "default"}
                                    <span style="color:#666;">|</span>
                                    {#if isRuntimeWalletEncrypted(walletStatus)}
                                        {#if isRuntimeWalletUnlocked(walletStatus)}
                                            <span style="color:var(--color-primary);">Unlocked</span>
                                        {:else}
                                            <span style="color:#ffaa00;">Locked</span>
                                        {/if}
                                    {:else}
                                        <span style="color:#888;">Unencrypted</span>
                                    {/if}
                                </span>
                            {:else}
                                <span style="color:#666;">Loading…</span>
                            {/if}
                        </div>
                        <div style="display:flex; gap:0.5rem; align-items:center;">
                            <span style="color:#888; min-width:6rem;">Vault backups</span>
                            {#if hasBackups}
                                <span>Last backup: <strong style="color:var(--color-primary);">{vaultWalletRecords[0]?.label}</strong>
                                    {#if vaultWalletRecords[0]?.modified}
                                        <span style="color:#666; font-size:0.65rem;"> • {new Date(vaultWalletRecords[0].modified * 1000).toLocaleString()}</span>
                                    {/if}
                                </span>
                            {:else}
                                <span style="color:#ffaa00;">No backups yet</span>
                            {/if}
                        </div>
                        <div style="display:flex; gap:0.5rem; align-items:center;">
                            <span style="color:#888; min-width:6rem;">Runtime files</span>
                            {#if walletDatExists === true}
                                <span style="color:#aaa;">Present (compatibility)</span>
                            {:else if walletDatExists === false}
                                <span style="color:#ffaa00;">Missing — restore from vault</span>
                            {:else}
                                <span style="color:#666;">Loading…</span>
                            {/if}
                        </div>
                    </div>

                    <div class="laser-divider" style="margin:1rem 0;"></div>

                    <!-- Primary next action -->
                    {#if !hasBackups}
                        <button class="cyber-btn primary-glow wide" on:click={() => (activeTab = "backup")}>
                            BACK UP WALLET
                        </button>
                    {:else if walletDatExists === false && hasBackups}
                        <button class="cyber-btn primary-glow wide" on:click={() => (activeTab = "restore")}>
                            RESTORE FROM VAULT
                        </button>
                    {:else}
                        <div class="btn-row" style="gap:0.6rem;">
                            <button class="cyber-btn primary-glow wide" on:click={() => (activeTab = "backup")}>
                                BACK UP WALLET
                            </button>
                            <button class="cyber-btn ghost wide" on:click={() => (activeTab = "restore")}>
                                RESTORE WALLET
                            </button>
                        </div>
                    {/if}
                </div>
            </div>
        {/if}

        <!-- BACKUP TAB -->
        {#if activeTab === "backup"}
            <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
                <div style="padding:1.25rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.3rem; font-size:0.8rem;">BACK UP WALLET TO VAULT</h4>
                    <p class="desc" style="margin:0 0 0.75rem; font-size:0.7rem;">
                        Creates an encrypted wallet backup record inside your Vault.
                        The Core wallet password is not stored.
                    </p>
                    {#if showFinishBackupNudge}
                        <div style="background:rgba(0,255,65,0.08); border:1px solid rgba(0,255,65,0.25); border-radius:4px; padding:0.5rem 0.75rem; color:var(--color-primary); font-size:0.7rem; margin-bottom:0.75rem;">
                            Finish backup for the wallet you just created.
                        </div>
                    {/if}
                    <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem; margin-bottom:0.5rem;">
                        <div style="font-size:0.6rem; color:#888; margin-bottom:0.3rem; letter-spacing:0.3px;">BACKUP DETAILS</div>
                        <div style="display:flex; flex-direction:column; gap:0.5rem;">
                            <input
                                type="text"
                                class="input-glass"
                                placeholder="Label (e.g. Main wallet)"
                                bind:value={vaultExportLabel}
                                style="font-size:0.75rem; padding:0.5rem;"
                            />
                            <div style="display:flex; align-items:center; gap:0.4rem; font-size:0.7rem; color:#aaa;">
                                <span style="color:#888;">Recovery:</span>
                                <span style="color:var(--color-primary);">{vaultExportAdvancedRecovery ? "Separate password" : "Vault passphrase"}</span>
                            </div>
                            <label class="toggle" style="font-size:0.65rem; color:#888; display:flex; align-items:center; gap:0.3rem; cursor:pointer;">
                                <input type="checkbox" bind:checked={vaultExportAdvancedRecovery} />
                                Use separate recovery password
                            </label>
                            {#if vaultExportAdvancedRecovery}
                                <input
                                    type="password"
                                    class="input-glass"
                                    placeholder="Wallet backup passphrase"
                                    bind:value={vaultExportPassphrase}
                                    style="font-size:0.75rem; padding:0.5rem;"
                                />
                                <p class="desc" style="margin:0.25rem 0 0; color:#888; font-size:0.6rem;">
                                    Use this only if you want this backup to require a different recovery password than your vault.
                                </p>
                            {:else}
                                <p style="color:#888; font-size:0.65rem; margin:0;">
                                    The vault passphrase protects the backup record. Recovery stays simple — unlock the vault to restore.
                                </p>
                            {/if}
                        </div>
                    </div>
                    <button
                        class="cyber-btn primary-glow wide"
                        on:click={executeVaultExport}
                        disabled={vaultExportWorking || !vaultExportLabel || (vaultExportAdvancedRecovery && !vaultExportPassphrase)}
                    >
                        {vaultExportWorking ? "BACKING UP…" : "BACK UP WALLET TO VAULT"}
                    </button>
                    {#if vaultWalletRecordsMsg}
                        <p style="color:var(--color-primary); font-size:0.7rem; margin:0.5rem 0 0;">{vaultWalletRecordsMsg}</p>
                    {/if}
                    {#if vaultWalletRecordsError}
                        <p style="color:#ff5555; font-size:0.7rem; margin:0.3rem 0 0;">{vaultWalletRecordsError}</p>
                    {/if}
                </div>
            </div>
        {/if}

        <!-- RESTORE TAB -->
        {#if activeTab === "restore"}
            {@const selectedRec = vaultWalletRecords.find(r => r.record_id === selectedRecordId)}
            {@const recRecoveryMode = selectedRec?.metadata?.recovery_mode}
            {@const isVaultPassphraseRecovery = recRecoveryMode === "vault_passphrase"}
            <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
                <div style="padding:1.25rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.8rem;">VAULT BACKUPS</h4>
                    {#if vaultWalletRecordsLoading}
                        <p class="desc" style="color:#888;">Loading backups…</p>
                    {:else if vaultWalletRecordsError}
                        <p style="color:#ff5555; font-size:0.7rem;">{vaultWalletRecordsError}</p>
                    {:else if vaultWalletRecords.length === 0}
                        <div style="text-align:center; padding:1.5rem 0;">
                            <p class="desc" style="color:#888; margin:0 0 0.75rem;">
                                No wallet backups stored yet.
                                The vault is your portable encrypted Commander wallet container — back up from the Backup tab to create your first recovery record.
                            </p>
                            <button class="cyber-btn ghost small" on:click={() => (activeTab = "backup")}>
                                CREATE YOUR FIRST BACKUP
                            </button>
                        </div>
                    {:else}
                        <div style="max-height:220px; overflow-y:auto; margin-bottom:0.75rem;">
                            {#each vaultWalletRecords as rec}
                                <div
                                    class="vault-record-row"
                                    class:selected={selectedRecordId === rec.record_id}
                                    style="display:flex; align-items:center; gap:0.5rem; padding:0.5rem 0.6rem; border:1px solid {selectedRecordId === rec.record_id ? 'rgba(0,255,65,0.4)' : 'rgba(255,255,255,0.04)'}; border-radius:4px; margin-bottom:0.3rem; font-size:0.7rem; cursor:pointer; background:{selectedRecordId === rec.record_id ? 'rgba(0,255,65,0.05)' : 'transparent'};"
                                    on:click={() => selectRecord(rec.record_id)}
                                    on:keydown={(e) => e.key === 'Enter' && selectRecord(rec.record_id)}
                                    role="button"
                                    tabindex="0"
                                >
                                    <div style="flex:1; min-width:0;">
                                        <div style="color:var(--color-primary); font-weight:600; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">{rec.label}</div>
                                        <div style="color:#888; font-size:0.6rem; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;" title={rec.record_id}>
                                            {rec.metadata?.source === "core-next-exportwalletmigration" ? "Active wallet export" : rec.metadata?.source ?? "Imported file"}
                                            {#if rec.modified} • {new Date(rec.modified * 1000).toLocaleString()}{/if}
                                        </div>
                                    </div>
                                    <div style="display:flex; gap:0.25rem;">
                                        {#if rec.metadata?.recovery_mode === "vault_passphrase"}
                                            <span style="color:var(--color-primary); font-size:0.6rem; border:1px solid rgba(0,255,65,0.3); padding:1px 4px; border-radius:3px;" title="Recovery uses vault passphrase">VAULT RECOVERY</span>
                                        {:else if rec.metadata?.recovery_mode === "separate_passphrase"}
                                            <span style="color:#ffaa00; font-size:0.6rem; border:1px solid rgba(255,170,0,0.3); padding:1px 4px; border-radius:3px;" title="Recovery requires separate password">SEPARATE PW</span>
                                        {/if}
                                        {#if rec.metadata?.restorable}
                                            <span style="color:var(--color-primary); font-size:0.6rem; border:1px solid rgba(0,255,65,0.4); padding:1px 4px; border-radius:3px;">RESTORABLE</span>
                                        {:else}
                                            <span style="color:#888; font-size:0.6rem; border:1px solid rgba(255,255,255,0.1); padding:1px 4px; border-radius:3px;">PUBLIC ONLY</span>
                                        {/if}
                                        {#if rec.metadata?.private_keys_included}
                                            <span style="color:#ffaa00; font-size:0.6rem; border:1px solid rgba(255,170,0,0.4); padding:1px 4px; border-radius:3px;">PRIVATE</span>
                                        {/if}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {/if}

                    {#if selectedRecordId}
                        <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                            <h4 style="color:var(--color-danger, #ff5555); margin:0 0 0.5rem; font-size:0.75rem;">RESTORE SELECTED BACKUP</h4>
                            <p class="desc" style="color:#ffaa00; margin-bottom:0.5rem;">
                                <strong>WARNING:</strong> This creates a new Core runtime wallet. Back up your current wallet first. Restart is required.
                            </p>
                            <input
                                type="text"
                                class="input-glass"
                                placeholder="New wallet name (e.g. main)"
                                bind:value={vaultRestoreWalletName}
                                style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;"
                            />
                            {#if isVaultPassphraseRecovery}
                                <div style="background:rgba(0,255,65,0.05); border:1px solid rgba(0,255,65,0.15); border-radius:4px; padding:0.5rem 0.75rem; margin-bottom:0.5rem;">
                                    <p style="margin:0; font-size:0.65rem; color:var(--color-primary);">
                                        Uses the unlocked vault passphrase for recovery. No additional password needed.
                                    </p>
                                </div>
                            {:else}
                                <input
                                    type="password"
                                    class="input-glass"
                                    placeholder={recRecoveryMode ? "Backup recovery password" : "Backup recovery password (legacy/imported records may have their own)"}
                                    bind:value={vaultRestorePassphrase}
                                    style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;"
                                />
                            {/if}
                            <input
                                type="text"
                                class="input-glass"
                                placeholder="Type RESTORE WALLET to confirm"
                                bind:value={vaultRestoreConfirm}
                                style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;"
                            />
                            <button
                                class="toggle"
                                style="background:none; border:none; color:var(--color-primary); font-size:0.7rem; padding:0.2rem 0; cursor:pointer; text-align:left; margin-bottom:0.4rem;"
                                on:click={() => (vaultRestoreAdvanced = !vaultRestoreAdvanced)}
                            >
                                {vaultRestoreAdvanced ? "▼" : "▶"} Advanced restore options
                            </button>
                            {#if vaultRestoreAdvanced}
                                <input
                                    type="number"
                                    min="0"
                                    step="1"
                                    class="input-glass"
                                    placeholder="Birth height (optional)"
                                    bind:value={vaultRestoreBirth}
                                    style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;"
                                />
                                <p class="desc" style="margin: -0.2rem 0 0.4rem; color:#888; font-size:0.65rem;">
                                    Optional. Only use if support asks you to speed up recovery scanning.
                                </p>
                            {/if}
                            <button
                                class="cyber-btn danger small wide"
                                on:click={executeVaultRestore}
                                disabled={vaultRestoreWorking || !vaultRestoreRecordId || !vaultRestoreWalletName || (!isVaultPassphraseRecovery && !vaultRestorePassphrase) || vaultRestoreConfirm !== VAULT_RESTORE_CONFIRM}
                            >
                                {vaultRestoreWorking ? "RESTORING…" : "RESTORE RUNTIME WALLET FROM VAULT"}
                            </button>
                            <p class="desc" style="margin:0.3rem 0 0; color:#666; font-size:0.6rem;">
                                Restores into a Core runtime wallet on disk. Restart the node after restore.
                            </p>
                        </div>
                    {:else}
                        <p class="desc" style="margin:0.5rem 0 0; color:#888; font-size:0.65rem;">Select a backup above to begin restore.</p>
                    {/if}
                </div>
            </div>
        {/if}

        <!-- SECURITY TAB -->
        {#if activeTab === "security"}
            <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
                <div style="padding:1.25rem; display:flex; flex-direction:column; gap:1rem;">
                    <!-- Wallet encryption change -->
                    <div>
                        <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.8rem;">WALLET ENCRYPTION</h4>
                        <p class="desc" style="margin:0 0 0.5rem;">Change the Core wallet encryption passphrase.</p>
                        <div class="field-group" style="gap:0.4rem; margin-bottom:0.6rem;">
                            <input type="password" class="input-glass" placeholder="Current Password" bind:value={passOld} style="font-size:0.75rem; padding:0.5rem;" />
                            <input type="password" class="input-glass" placeholder="New Password" bind:value={passNew} style="font-size:0.75rem; padding:0.5rem;" />
                            <input type="password" class="input-glass" placeholder="Confirm New" bind:value={passNewConfirm} style="font-size:0.75rem; padding:0.5rem;" />
                        </div>
                        <button class="cyber-btn small wide" on:click={changePassword}>
                            UPDATE PASSWORD
                        </button>
                    </div>

                    <div class="laser-divider"></div>

                    <!-- Vault lock -->
                    <div>
                        <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.8rem;">VAULT SESSION</h4>
                        <button class="cyber-btn ghost small wide" on:click={executeLockVault}>
                            LOCK VAULT
                        </button>
                        <p class="desc" style="margin:0.4rem 0 0; color:#888; font-size:0.65rem;">
                            Locking clears the cached passphrase from memory. You will need to enter it again to use backups.
                        </p>
                    </div>

                    <div class="laser-divider"></div>

                    <!-- Future enhancements note -->
                    <div>
                        <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.8rem;">FUTURE ENHANCEMENTS</h4>
                        <p class="desc" style="margin:0; color:#888; font-size:0.65rem;">
                            PIN, biometric, and multi-slot unlock are reserved for future releases. They are not implemented yet.
                        </p>
                    </div>
                </div>
            </div>
        {/if}

        <!-- ADVANCED TAB -->
        {#if activeTab === "advanced"}
            <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
                <div style="padding:1.25rem; display:flex; flex-direction:column; gap:1.25rem;">

                    <!-- wallet.dat legacy -->
                    <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                        <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.75rem;">RUNTIME WALLET FILES (COMPATIBILITY)</h4>
                        <p class="desc" style="margin:0 0 0.6rem;">Direct wallet.dat backup, restore, and new wallet actions. Kept for compatibility and advanced recovery.</p>
                        <div class="btn-row" style="gap:0.5rem; flex-wrap:wrap;">
                            <button class="cyber-btn ghost tiny" on:click={backupWallet}>BACKUP WALLET.DAT</button>
                            <button class="cyber-btn ghost tiny" on:click={restoreWallet}>RESTORE FROM FILE</button>
                            <button class="cyber-btn ghost tiny danger" on:click={createNewWallet}>NEW WALLET</button>
                        </div>
                        <div class="laser-divider" style="margin:0.6rem 0;"></div>
                        <div class="btn-row" style="gap:0.5rem; flex-wrap:wrap;">
                            <button class="cyber-btn ghost tiny danger" on:click={openExportModal}>EXPORT KEYS</button>
                            <button class="cyber-btn ghost tiny" on:click={openImportModal}>IMPORT KEYS</button>
                        </div>
                    </div>

                    <!-- Core migration envelope -->
                    <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                        <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.75rem;">CORE WALLET MIGRATION (LOW-LEVEL ENVELOPE)</h4>
                        <p class="desc" style="margin:0 0 0.75rem;">Direct Core Next migration envelope export / validate / restore from file. For technical recovery and cross-tool interop.</p>
                        <div style="display:grid; grid-template-columns: repeat(3, 1fr); gap: 1rem;">
                            <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem;">EXPORT</h4>
                                <button class="cyber-btn ghost tiny" on:click={migrateSelectExportPath}>
                                    {migrationExportPath ? migrationExportPath.split('/').pop().split('\\').pop() : "CHOOSE DESTINATION"}
                                </button>
                                <label class="toggle" style="font-size:0.65rem; color:#888;">
                                    <input type="checkbox" bind:checked={migrationExportPrivate} />
                                    Include private keys (encrypted)
                                </label>
                                {#if migrationExportPrivate}
                                    <input type="password" class="input-glass" placeholder="Export passphrase (min 8)" bind:value={migrationExportPass} style="font-size:0.7rem; padding:0.4rem;"/>
                                {/if}
                                <label class="toggle" style="font-size:0.65rem; color:#888;">
                                    <input type="checkbox" bind:checked={migrationExportOverwrite} />
                                    Allow overwrite
                                </label>
                                <button class="cyber-btn tiny" on:click={migrateExport} disabled={migrationWorking || !migrationExportPath}>
                                    {migrationWorking ? "…" : "EXPORT"}
                                </button>
                                {#if migrationError && !migrationValidateResult && !migrationRestoreResult}
                                    <p style="color:#ff5555; font-size:0.65rem; margin:0;">{migrationError}</p>
                                {/if}
                                {#if migrationExportResult}
                                    <div style="background:rgba(0,255,65,0.05); padding:0.4rem; border-radius:4px; font-size:0.65rem;">
                                        <p style="color:var(--color-primary); margin:0;">Exported: {migrationExportResult.filename}</p>
                                        <p style="color:#888; margin:0.2rem 0;">Chain: {migrationExportResult.chain}</p>
                                    </div>
                                {/if}
                            </div>
                            <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem;">VALIDATE</h4>
                                <button class="cyber-btn ghost tiny" on:click={migrateSelectValidatePath}>
                                    {migrationValidatePath ? migrationValidatePath.split('/').pop().split('\\').pop() : "SELECT FILE"}
                                </button>
                                <input type="password" class="input-glass" placeholder="Export passphrase (if encrypted)" bind:value={migrationValidatePass} style="font-size:0.7rem; padding:0.4rem;"/>
                                <button class="cyber-btn tiny" on:click={migrateValidate} disabled={migrationWorking || !migrationValidatePath}>
                                    {migrationWorking ? "…" : "VALIDATE"}
                                </button>
                                {#if migrationValidateResult}
                                    <div style="background:rgba(0,255,65,0.05); padding:0.4rem; border-radius:4px; font-size:0.65rem; max-height:140px; overflow-y:auto;">
                                        <p style="color:{migrationValidateResult.valid ? 'var(--color-primary)' : '#ff5555'}; margin:0;">{migrationValidateResult.valid ? 'VALID' : 'INVALID'}</p>
                                        <p style="color:#888; margin:0.2rem 0;">Network: {migrationValidateResult.chain?.network ?? "unknown"} | Restorable: {migrationValidateResult.restorable}</p>
                                        {#if migrationValidateResult.warnings?.length}
                                            {#each migrationValidateResult.warnings as w}
                                                <p style="color:#ffaa00; margin:0; font-size:0.6rem;">{w}</p>
                                            {/each}
                                        {/if}
                                    </div>
                                {/if}
                            </div>
                            <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                <h4 style="color:var(--color-danger, #ff5555); margin:0; font-size:0.7rem;">RESTORE FROM FILE</h4>
                                <button class="cyber-btn ghost tiny danger" on:click={migrateSelectRestorePath}>
                                    {migrationRestorePath ? migrationRestorePath.split('/').pop().split('\\').pop() : "SELECT FILE"}
                                </button>
                                <input type="text" class="input-glass" placeholder="New wallet name" bind:value={migrationRestoreName} style="font-size:0.7rem; padding:0.4rem;"/>
                                <input type="password" class="input-glass" placeholder="Export passphrase" bind:value={migrationRestorePass} style="font-size:0.7rem; padding:0.4rem;"/>
                                <input type="number" min="0" step="1" class="input-glass" placeholder="Birth height (optional)" bind:value={migrationRestoreBirth} style="font-size:0.7rem; padding:0.4rem;"/>
                                <input type="text" class="input-glass" placeholder="Type RESTORE WALLET to confirm" bind:value={migrationRestoreConfirm} style="font-size:0.7rem; padding:0.4rem;"/>
                                <button class="cyber-btn danger tiny" on:click={migrateRestore} disabled={migrationWorking || !migrationRestorePath || !migrationRestoreName || !migrationRestorePass || migrationRestoreConfirm !== RESTORE_CONFIRM}>
                                    {migrationWorking ? "…" : "RESTORE"}
                                </button>
                                {#if migrationRestoreResult && !migrationError}
                                    <div style="background:rgba(0,255,65,0.05); padding:0.4rem; border-radius:4px; font-size:0.65rem;">
                                        <p style="color:var(--color-primary); margin:0;">Wallet: {migrationRestoreResult.wallet_name}</p>
                                    </div>
                                {/if}
                                {#if migrationError && (migrationRestorePath || migrationRestoreConfirm)}
                                    <p style="color:#ff5555; font-size:0.65rem; margin:0;">{migrationError}</p>
                                {/if}
                            </div>
                        </div>
                    </div>

                    <!-- Advanced vault records -->
                    <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                        <button
                            class="toggle"
                            style="background:none; border:none; color:var(--color-primary); font-size:0.75rem; padding:0; cursor:pointer; text-align:left;"
                            on:click={() => (showAdvancedVaultRecords = !showAdvancedVaultRecords)}
                        >
                            {showAdvancedVaultRecords ? "▼" : "▶"} ADVANCED VAULT RECORDS
                        </button>
                        {#if showAdvancedVaultRecords}
                            <div style="margin-top:0.75rem; display:grid; grid-template-columns: 1fr 1fr; gap: 1rem;">
                                <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                    <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem;">IMPORT MIGRATION FILE INTO VAULT</h4>
                                    <p class="desc">Import an existing envelope file into the vault.</p>
                                    <button class="cyber-btn ghost tiny" on:click={vaultImportMigrationFile}>
                                        {vaultImportPath ? vaultImportPath.split('/').pop().split('\\').pop() : "SELECT FILE"}
                                    </button>
                                    <input type="text" class="input-glass" placeholder="Label" bind:value={vaultImportLabel} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <input type="password" class="input-glass" placeholder="Envelope passphrase (if encrypted)" bind:value={vaultImportMigrationPassphrase} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <input type="password" class="input-glass" placeholder="Vault passphrase" bind:value={vaultImportPassphrase} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <button class="cyber-btn tiny" on:click={executeVaultImport} disabled={vaultImportWorking || !vaultImportPath || !vaultImportLabel || !vaultImportPassphrase}>
                                        {vaultImportWorking ? "…" : "IMPORT"}
                                    </button>
                                </div>
                                <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                    <h4 style="color:var(--color-danger, #ff5555); margin:0; font-size:0.7rem;">REMOVE VAULT BACKUP RECORD</h4>
                                    <p class="desc">Delete a stored backup. Does not affect wallet.dat.</p>
                                    {#if selectedRecordId}
                                        <p class="desc" style="margin:0; color:#ffaa00;">Selected: {selectedRecordId}</p>
                                    {/if}
                                    <input type="text" class="input-glass" placeholder="Record ID" bind:value={vaultRemoveRecordId} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <input type="password" class="input-glass" placeholder="Vault passphrase" bind:value={vaultRemovePassphrase} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <button class="cyber-btn ghost danger tiny" on:click={executeVaultRemove} disabled={vaultRemoveWorking || !vaultRemoveRecordId || !vaultRemovePassphrase}>
                                        {vaultRemoveWorking ? "…" : "REMOVE"}
                                    </button>
                                </div>
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        {/if}
    </div>
{/if}
<!-- ================= MODALS ================= -->

<!-- VAULT BACKUP CONFIRMATION MODAL (60t) -->
{#if showVaultArchiveModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => (showVaultArchiveModal = false)}
        on:keydown={(e) => e.key === "Escape" && (showVaultArchiveModal = false)}
    >
        <div class="modal-staged modal-frame" style="max-width:480px;">
            <h3 style="color:#ffaa00; margin-top:0; text-align:center;">
                BACK UP CURRENT VAULT?
            </h3>
            <p style="color:#aaa; font-size:0.75rem; line-height:1.5;">
                Commander will move <code style="color:var(--color-primary);">{vaultBasenameFromPath(vaultOverview?.vault_path)}</code>
                into the backup folder. The file is preserved on disk and can be re-imported later with <strong>Switch / Import Vault</strong>.
            </p>
            <p style="color:#888; font-size:0.7rem; line-height:1.4; margin-top:0.4rem;">
                This is the safe replacement for manually renaming
                <code>vault.json</code> from a terminal.
            </p>
            <div class="input-group" style="margin:0.75rem 0 1rem;">
                <label for="vault-archive-confirm" style="font-size:0.7rem; color:#ffaa00;">
                    Type <strong>{VAULT_ARCHIVE_CONFIRM}</strong> to confirm:
                </label>
                <input
                    id="vault-archive-confirm"
                    type="text"
                    class="input-glass"
                    bind:value={vaultArchiveConfirm}
                    placeholder={VAULT_ARCHIVE_CONFIRM}
                />
            </div>
            <div class="modal-actions">
                <button
                    class="cyber-btn danger"
                    on:click={executeArchiveVault}
                    disabled={vaultArchiveWorking || vaultArchiveConfirm !== VAULT_ARCHIVE_CONFIRM}
                    style="min-height:50px; flex:1;"
                >
                    {vaultArchiveWorking ? "BACKING UP…" : "BACK UP VAULT"}
                </button>
                <button
                    class="cyber-btn ghost"
                    on:click={() => { showVaultArchiveModal = false; vaultArchiveConfirm = ""; }}
                    style="min-height:50px; flex:1;"
                >
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- CREATE VAULT FORM (60s) — used when user picks Create New Vault from the
     locked state, the unlocked Vault File bar, or after Archive. -->
{#if showCreateVaultForm}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => (showCreateVaultForm = false)}
        on:keydown={(e) => e.key === "Escape" && (showCreateVaultForm = false)}
    >
        <div class="modal-staged modal-frame" style="max-width:400px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin-top:0; text-align:center; font-size:0.9rem; letter-spacing:1px;">
                CREATE NEW VAULT
            </h3>
            {#if hasVault}
                <p style="color:#ffaa00; font-size:0.65rem; line-height:1.4; margin:0.25rem 0 0;">
                    Commander will save the current vault first, then create a new active vault.
                </p>
            {:else}
                <p style="color:#888; font-size:0.65rem; line-height:1.4; margin:0.25rem 0 0;">
                    Create a new encrypted vault for wallet backups and optional app secrets.
                </p>
            {/if}
            <p style="color:#888; font-size:0.65rem; line-height:1.4; margin:0.4rem 0 0;">
                <strong style="color:#ff8888;">Important:</strong> If you lose your passphrase, your encrypted data cannot be recovered.
            </p>
            <div class="input-group" style="margin-top:0.6rem;">
                <label for="create-vault-form-pass" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">NEW VAULT PASSPHRASE (min 8 chars)</label>
                <input
                    id="create-vault-form-pass"
                    type="password"
                    class="input-glass"
                    bind:value={createVaultFormPass}
                    placeholder="At least 8 characters"
                    style="font-size:0.75rem; padding:0.45rem;"
                />
            </div>
            <div class="input-group" style="margin-top:0.4rem;">
                <label for="create-vault-form-pass-confirm" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">CONFIRM PASSPHRASE</label>
                <input
                    id="create-vault-form-pass-confirm"
                    type="password"
                    class="input-glass"
                    bind:value={createVaultFormPassConfirm}
                    placeholder="Re-enter passphrase"
                    style="font-size:0.75rem; padding:0.45rem;"
                />
            </div>
            {#if createVaultFormError}
                <p style="color:#ff5555; font-size:0.65rem; margin:0.4rem 0 0;">{createVaultFormError}</p>
            {/if}
            <div style="display:flex; gap:0.5rem; margin-top:0.75rem;">
                <button
                    class="cyber-btn primary-glow small"
                    on:click={submitCreateVaultForm}
                    disabled={createVaultFormWorking || !createVaultFormPass || createVaultFormPass.length < 8 || createVaultFormPass !== createVaultFormPassConfirm}
                    style="flex:1;"
                >
                    {createVaultFormWorking ? "CREATING…" : (hasVault ? "BACK UP + CREATE" : "CREATE VAULT")}
                </button>
                <button
                    class="cyber-btn ghost small"
                    on:click={() => { showCreateVaultForm = false; createVaultFormPass = ""; createVaultFormPassConfirm = ""; createVaultFormError = ""; }}
                    style="flex:1;"
                >
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

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
        on:click|self={closeUnlockModal}
        on:keydown={(e) => e.key === "Escape" && closeUnlockModal()}
    >
        <div class="modal-staged">
            <div class="modal-header">
                <h3>
                    {unlockingFile ? "🔓 DECRYPT FILE" : "🔐 UNLOCK CORE WALLET"}
                </h3>
            </div>
            <div class="modal-body">
                <p>
                    {unlockingFile
                        ? "Enter password to decrypt file:"
                        : unlockModalPurpose === "vault_backup"
                            ? "Enter the Core wallet passphrase to unlock the runtime wallet for backup. This is separate from your vault passphrase."
                            : "Enter your Core wallet passphrase to export keys."}
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
                    on:click={closeUnlockModal}>CANCEL</button
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
