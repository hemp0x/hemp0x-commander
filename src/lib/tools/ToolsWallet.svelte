<script>
    import { createEventDispatcher, onDestroy, onMount, tick } from "svelte";
    import { fly } from "svelte/transition";
    import { core } from "@tauri-apps/api";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import CryptoJS from "crypto-js";
    import { coreBusyUntil, systemStatus, vaultStatus } from "../../stores.js"; // Import Store
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

    function notifyVaultNeedsSave(reason = "Vault contents changed") {
        addToolNotification(
            "Save Hemp0x Vault",
            `${reason}. Save your Hemp0x Vault file after important changes.`,
            "warning",
            null,
            true,
        );
    }

    function withTimeout(promise, ms, label) {
        let timer;
        const timeout = new Promise((_, reject) => {
            timer = window.setTimeout(() => reject(new Error(`${label} timed out`)), ms);
        });
        return Promise.race([promise, timeout]).finally(() => window.clearTimeout(timer));
    }

    // --- WALLET NAME VALIDATION (slice 64q) ---
    // Mirrors the shared backend helper `validate_core_wallet_filename`
    // (src-tauri/src/modules/utils.rs) so the UI can reject invalid Core
    // wallet filenames before any backend call. Any user-entered wallet
    // name that becomes a Core `-wallet=<name>` filename must pass this.
    const WALLET_NAME_MAX_LEN = 64;
    const WALLET_NAME_RE = /^[A-Za-z0-9_-]+$/;
    const WALLET_NAME_INVALID_MSG = "Wallet name can only use letters, numbers, hyphen, and underscore.";

    // Returns an error string when invalid, or "" when valid. An empty
    // input is treated as valid here so callers can decide whether the
    // field is required (they pass the backend default when left blank).
    function validateWalletName(name) {
        const trimmed = (name || "").trim();
        if (trimmed === "") return "";
        if (trimmed === "." || trimmed === "..") return "Wallet name cannot be '.' or '..'.";
        if (trimmed.includes("/") || trimmed.includes("\\") || trimmed.includes(":")) {
            return "Wallet name cannot contain path separators.";
        }
        if (!WALLET_NAME_RE.test(trimmed)) return WALLET_NAME_INVALID_MSG;
        if (trimmed.length > WALLET_NAME_MAX_LEN) {
            return `Wallet name must be at most ${WALLET_NAME_MAX_LEN} characters.`;
        }
        const reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
        if (reserved.includes(trimmed.toUpperCase())) {
            return "Wallet name cannot be a reserved device name.";
        }
        return "";
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
            "Commander will archive the current Core wallet file before restoring the selected wallet file. You can also create a manual backup first.",
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
                    label: "CONTINUE",
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
            // Always archive an existing wallet.dat before replacing it, even from
            // the legacy advanced path. This prevents accidental destructive restores.
            // We use restartNode: false because we handle it manually
            await core.invoke("restore_wallet", {
                path: restorePath,
                backupExisting: true,
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
            "Commander will archive the current Core wallet file before creating a new runtime wallet. You can also create a manual backup first.",
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
                    label: "CONTINUE",
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
    let encryptTargetWalletName = null;
    let encryptedWalletRestartName = "";

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
                backupExisting: true,
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
        encryptWorking = true;

        try {
            const activeWalletName = encryptTargetWalletName
                || unifiedPromotionResult?.core_wallet_name
                || connectResult?.core_wallet_name
                || walletStatus?.walletname
                || null;
            encryptedWalletRestartName = activeWalletName || "";
            if (activeWalletName && activeWalletName !== "default" && activeWalletName !== "wallet.dat") {
                await core.invoke("wallet_encrypt_named", { walletName: activeWalletName, password: newEncPass });
            } else {
                await core.invoke("wallet_encrypt", { password: newEncPass });
            }
            await new Promise((r) => setTimeout(r, 2000));

            encryptWorking = false;
            showEncryptModal = false;
            newEncPass = "";
            newEncPassConfirm = "";
            encryptTargetWalletName = null;
            showEncryptCompleteOverlay = true;
        } catch (e) {
            encryptWorking = false;
            showToast("Encryption Failed: " + e, "error");
        }
    }

    async function startPendingHistoryRecovery() {
        const walletName = pendingHistoryRecoveryWallet;
        if (!walletName) return;
        pendingHistoryRecoveryWallet = "";
        try {
            await core.invoke("vault_start_wallet_history_recovery", {
                walletName,
                fromBlock: null,
            });
            historyRecoveryBackgroundActive = true;
            coreBusyUntil.set(Date.now() + 120000);
            showToast("Wallet history recovery started in the background.", "info", false);
            window.setTimeout(() => {
                historyRecoveryBackgroundActive = false;
            }, 120000);
        } catch (err) {
            console.warn("Deferred history recovery did not start:", err);
        }
    }

    // --- SECURITY / PASSWORD CHANGE ---

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

    // Wait for Tauri to be ready, then refresh the header once.
    // The reactive $: statement may not always fire in Svelte's batching,
    // so we poll with a short timeout in onMount as the primary trigger.
    let headerInitialised = false;
    let _headerRetry = 0;

    onMount(() => {
        const tryInit = () => {
            if (headerInitialised) return;
            if (!tauriReady) {
                _headerRetry++;
                if (_headerRetry < 30) {
                    setTimeout(tryInit, 200);
                } else {
                    vaultOverviewError = "Timed out waiting for Tauri backend";
                }
                return;
            }
            headerInitialised = true;
            refreshWalletHeader();
        };
        tryInit();

        const refreshRuntimeStatus = () => {
            if (!tauriReady || document.hidden || historyRecoveryBackgroundActive) return;
            loadWalletStatus(true);
        };
        const onVisibilityChange = () => refreshRuntimeStatus();
        const timer = window.setInterval(refreshRuntimeStatus, 10000);
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
        migrationRestoreConfirmChecked = false;
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

        // Standard Core wallet unlock. Some callers provide a follow-up
        // action (key export, vault backup, guided connect); the plain
        // wallet-unlock button only unlocks and refreshes status.
        try {
            unlockError = "";
            const plainWalletUnlock = unlockModalPurpose === "wallet_unlock";
            const unlockDuration = unlockModalPurpose === "vault_backup" || plainWalletUnlock ? 300 : 60;
            const runtimeWalletName = loadedRuntimeWalletName();
            if (isDefaultRuntimeWalletName(runtimeWalletName)) {
                await core.invoke("wallet_unlock", {
                    password: unlockPassword,
                    duration: unlockDuration,
                });
            } else {
                await core.invoke("wallet_unlock_named", {
                    walletName: runtimeWalletName,
                    password: unlockPassword,
                    duration: unlockDuration,
                });
            }
            showUnlockModal = false;
            if (walletStatus && isRuntimeWalletEncrypted(walletStatus)) {
                walletStatus = {
                    ...walletStatus,
                    unlocked_until: Math.floor(Date.now() / 1000) + unlockDuration,
                    locked: false,
                };
            }
            const afterUnlock = unlockAfterWalletUnlock;
            unlockAfterWalletUnlock = null;
            const unlockedForVaultBackup = unlockModalPurpose === "vault_backup";
            unlockModalPurpose = "key_export";
            unlockPassword = "";
            if (afterUnlock) {
                await afterUnlock();
            } else if (!plainWalletUnlock) {
                proceedExport();
            }
            if (unlockedForVaultBackup || plainWalletUnlock) {
                await refreshWalletHeader();
            }
            if (plainWalletUnlock) {
                showToast("Wallet unlocked.", "success");
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

        for (let i = 0; i < selected.length; i++) {
            const item = selected[i];
            try {
                await core.invoke("import_priv_key", {
                    privKey: item.privKey,
                    label: item.label || "",
                    rescan: importRescan && i === selected.length - 1,
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
    let migrationRestoreConfirmChecked = false;
    let migrationRestoreNameError = "";
    $: migrationRestoreNameError = validateWalletName(migrationRestoreName);
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
    let vaultExportAdvancedRecovery = false;

    let vaultRestoreRecordId = "";
    let vaultRestoreWalletName = "";
    let vaultRestoreWalletNameError = "";
    $: vaultRestoreWalletNameError = validateWalletName(vaultRestoreWalletName);
    let vaultRestorePassphrase = "";
    let vaultRestoreVaultPassphrase = "";
    let vaultRestoreBirth = "";
    let vaultRestoreConfirmChecked = false;
    let vaultRestoreWorking = false;

    let vaultRemoveRecordId = "";
    let vaultRemovePassphrase = "";
    let vaultRemoveWorking = false;

    // --- VAULT-FIRST UX STATE ---
    let vaultUnlocked = false;
    let vaultUnlockedLoading = false;
    let walletStatusError = "";
    let vaultPageError = "";     // persistent error visible on unlocked page
    let walletStatus = null; // { encrypted, unlocked, walletname, ... } or null
    let walletStatusLoading = false;
    let walletDatExists = null; // null = unknown, true/false from get_data_folder_info
    let vaultOverview = null;   // result of vault_get_vault_overview
    let vaultOverviewLoading = false;
    let vaultOverviewError = null; // null = not yet loaded, "" = loaded ok, string = error

    // React to global vault lock/unlock from the header status button.
    // When the global store says locked but our local state says unlocked,
    // clear unlocked-only state and switch to the locked view.
    $: if ($vaultStatus && typeof $vaultStatus.unlocked === "boolean") {
        if (!$vaultStatus.unlocked && vaultUnlocked) {
            resetUnlockedVaultState();
        }
    }

    // --- STATE-DRIVEN VIEW STATE (64i rework) ---
    let showAdvancedRecovery = false;
    let showVaultRestorePanel = false;

    // --- RECOVERY PHRASE RESTORE ---
    let showRecoveryPhraseModal = false;
    let recoveryPhraseWords = "";
    let recoveryPhrasePassphrase = "";
    let recoveryPhrasePassphraseConfirm = "";
    let recoveryWalletName = "";
    let recoveryWalletNameError = "";
    $: recoveryWalletNameError = validateWalletName(recoveryWalletName);
    let recoveryBirthHeight = "";
    let recoveryPhraseWorking = false;
    let recoveryPhraseError = "";

    // --- CREATE NEW VAULT WALLET ---
    let showCreateWalletModal = false;
    let createWalletPassphrase = "";
    let createWalletName = "";
    let createWalletNameError = "";
    $: createWalletNameError = validateWalletName(createWalletName);
    let createWalletWorking = false;
    let createWalletError = "";
    let createWalletDone = false;
    let createWalletMnemonic = "";
    let createWalletWordCount = 12;
    let createWalletPhraseConfirmed = false;

    function resetUnlockedVaultState() {
        vaultUnlocked = false;
        vaultListPassphrase = "";
        vaultWalletRecords = [];
        vaultWalletRecordsLoaded = false;
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        selectedRecordId = "";
        vaultRestoreRecordId = "";
        vaultRestoreWalletName = "";
        vaultRestorePassphrase = "";
        vaultRestoreVaultPassphrase = "";
        vaultRestoreBirth = "";
        vaultRestoreConfirmChecked = false;
        vaultRemoveRecordId = "";
        alignmentStatus = null;
        alignmentConnectPlan = null;
        connectResult = null;
        connectError = "";
        connectErrorCode = "";
        justConnected = false;
        showAlignmentReviewModal = false;
        showVaultRestorePanel = false;
        showRecoveryPhraseModal = false;
        showCreateWalletModal = false;
        createWalletMnemonic = "";
        createWalletWordCount = 12;
        createWalletPhraseConfirmed = false;
        createWalletError = "";
        recoveryPhraseWords = "";
        recoveryPhrasePassphrase = "";
        recoveryPhrasePassphraseConfirm = "";
        recoveryPhraseError = "";
        vaultPageError = "";
        // 64p: vault security state
        showChangeVaultPassphraseModal = false;
        changeVaultPassCurrent = "";
        changeVaultPassNew = "";
        changeVaultPassConfirm = "";
        changeVaultPassWorking = false;
        changeVaultPassError = "";
        changeVaultPassResult = null;
        showChangeRuntimeWalletPassModal = false;
        changeRuntimeWalletPassCurrent = "";
        changeRuntimeWalletPassNew = "";
        changeRuntimeWalletPassConfirm = "";
        changeRuntimeWalletPassWorking = false;
        changeRuntimeWalletPassError = "";
        changeRuntimeWalletPassResult = null;
        showUnloadVaultModal = false;
        unloadVaultConfirmChecked = false;
        unloadVaultWorking = false;
        unloadVaultError = "";
        unloadVaultResult = null;
        // 65: unified wallet file import
        showUnifiedImportModal = false;
        unifiedImportPath = "";
        unifiedImportDetectedType = null;
        unifiedImportDetection = null;
        unifiedImportWorking = false;
        unifiedImportError = "";
        unifiedImportResult = null;
        unifiedImportConfirmChecked = false;
        unifiedImportMigrationPassphrase = "";
        unifiedImportSnapshotLabel = "";
        unifiedImportSnapshotWorking = false;
        unifiedImportSnapshotResult = null;
        unifiedImportSnapshotError = "";
    }

    async function ensureNamedCoreWalletLoaded(walletName, label = "wallet") {
        if (!walletName) return null;

        let lastError = "";
        try {
            const loadState = await core.invoke("vault_load_wallet_into_core", { walletName });
            if (loadState?.loaded) return loadState;
        } catch (err) {
            lastError = String(err);
        }

        await core.invoke("vault_restart_core_with_wallet", { walletName });

        for (let i = 0; i < 20; i++) {
            await new Promise((r) => setTimeout(r, 1500));
            try {
                const state = await core.invoke("vault_load_wallet_into_core", { walletName });
                if (state?.loaded) return state;
            } catch (err) {
                lastError = String(err);
            }
        }

        throw new Error(`${label} was restored, but Commander could not load it in Core. ${lastError || "The wallet was not queryable after restart."}`);
    }

    // --- SKIP BACKUP ---
    let skipBackup = false;

    let activeVaultWalletName = null; // null = no vault wallet (legacy mode), string = vault wallet name

    // --- UNIFIED WALLET FILE IMPORT (65) ---
    let showUnifiedImportModal = false;
    let unifiedImportPath = "";
    let unifiedImportDetectedType = null; // "hemp0x_vault" | "legacy_core_wallet" | "core_migration_envelope" | "unknown" | null
    let unifiedImportDetection = null;
    let unifiedImportWorking = false;
    let unifiedImportError = "";
    let unifiedImportResult = null;
    let unifiedImportConfirmChecked = false;
    let unifiedImportMigrationPassphrase = "";
    let unifiedImportSnapshotLabel = "";
    let unifiedImportSnapshotWorking = false;
    let unifiedImportSnapshotResult = null;
    let unifiedImportSnapshotError = "";

    // --- CORE-TO-WEBCOM PROMOTION (66/66b) ---
    let unifiedPromotionWorking = false;
    let unifiedPromotionError = "";
    let unifiedPromotionResult = null;
    let unifiedPromotionPassphrase = "";
    let unifiedPromotionWalletName = "";
    let unifiedPromotionReplaceExisting = false;
    let unifiedPromotionWalletUnlockPass = "";
    let unifiedPromotionProgressLabel = "";
    let unifiedPromotionStep = 0;
    let unifiedPromotionNeedsRuntimeLoad = false;
    const UNIFIED_PROMOTION_STEPS = [
        "Preparing wallet conversion",
        "Verifying and adding wallet to vault",
        "Selecting vault runtime wallet",
        "Restarting Core and loading wallet",
        "Confirming wallet status",
        "Finishing wallet setup",
    ];

    // --- ADVANCED EXPORT (66b) ---
    let showAdvancedExportModal = false;
    let advancedExportPath = "";
    let advancedExportPassphrase = "";
    let advancedExportOverwrite = false;
    let advancedExportWalletUnlockPass = "";
    let advancedExportWorking = false;
    let advancedExportError = "";
    let advancedExportResult = null;

    // --- WALLET MAINTENANCE ---
    let utxoRefreshWorking = false;
    let historyRecoverWorking = false;
    let historyRecoveryBackgroundActive = false;
    let pendingHistoryRecoveryWallet = "";
    let walletSwitchWorking = false;
    let walletSwitchTargetName = "";
    let restartingCore = false;
    let encryptCompleteRestarting = false;
    let showEncryptCompleteOverlay = false;
    let encryptWorking = false;

    // --- BIP39 MNEMONIC GENERATION (backend) ---

    // --- CONNECT STATE TRACKING ---
    let justConnected = false;     // prevents reconnect loop after successful connect

    // --- VAULT CREATE / UNLOCK STATE ---
    let createVaultPassphrase = "";
    let createVaultPassphraseConfirm = "";
    let createVaultName = "";
    let createVaultWorking = false;
    let createVaultError = "";
    let unlockVaultPassphrase = "";
    let unlockVaultWorking = false;
    let unlockVaultError = "";
    let unlockVaultErrorDetails = "";

    // --- VAULT FILE MANAGER (60s) ---
    let vaultLabelInput = "";
    let vaultLabelSaving = false;
    let vaultLabelMsg = "";
    let vaultArchiveWorking = false;
    let showVaultArchiveModal = false;
    let showVaultManagerPanel = false;

    // --- VAULT SECURITY (64p: passphrase rotation + unload/fallback) ---
    let showChangeVaultPassphraseModal = false;
    let changeVaultPassCurrent = "";
    let changeVaultPassNew = "";
    let changeVaultPassConfirm = "";
    let changeVaultPassWorking = false;
    let changeVaultPassError = "";
    let changeVaultPassResult = null; // { rotated, modified, kdf_profile }
    let showChangeRuntimeWalletPassModal = false;
    let changeRuntimeWalletPassCurrent = "";
    let changeRuntimeWalletPassNew = "";
    let changeRuntimeWalletPassConfirm = "";
    let changeRuntimeWalletPassWorking = false;
    let changeRuntimeWalletPassError = "";
    let changeRuntimeWalletPassResult = null;

    let showUnloadVaultModal = false;
    let unloadVaultConfirmChecked = false;
    let unloadVaultWorking = false;
    let unloadVaultError = "";
    let unloadVaultResult = null; // backend unload response

    $: hasVault = vaultOverview?.exists ?? false;
    $: hasBackups = vaultWalletRecords.length > 0;
    $: isVaultPassphraseRecovery = (() => {
        const rec = vaultWalletRecords.find((r) => r.record_id === selectedRecordId);
        return rec?.metadata?.recovery_mode === "vault_passphrase";
    })();

    // When vault unlocks, load records once. An empty backup list is a
    // valid loaded state, so track it separately to avoid a reload loop.
    $: if (vaultUnlocked && hasVault && !vaultWalletRecordsLoaded && !vaultWalletRecordsLoading) {
        loadVaultWalletRecords();
        loadAlignmentStatus();
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
            try {
                await core.invoke("vault_validate_import_bundle", { path: selected });
            } catch (err) {
                showToast("Invalid vault file: " + err, "error");
                return;
            }
            if (hasVault) {
                switchImportPendingPath = selected;
                switchImportPendingArchive = true;
                switchImportArchiveArmed = true;
            } else {
                await doImportVault(selected, false);
            }
        } catch (err) {
            showToast("Import failed: " + err, "error");
        }
    }

    // --- UNIFIED WALLET FILE IMPORT (65) ---
    async function unifiedImportWalletFile() {
        if (!tauriReady) return;
        const selected = await open({
            title: "Import Wallet File",
            multiple: false,
        });
        if (!selected) return;
        await openUnifiedImportPath(selected);
    }

    async function importCurrentRuntimeWalletToVault() {
        if (!tauriReady) return;
        try {
            const config = await core.invoke("init_config");
            const separator = String(config.data_dir).includes("\\") ? "\\" : "/";
            const reportedName = String(walletStatus?.walletname || "").trim();
            const walletFileName = !reportedName || reportedName === "default" || reportedName === "wallet.dat"
                ? "wallet.dat"
                : reportedName;
            await openUnifiedImportPath(`${config.data_dir}${separator}${walletFileName}`);
        } catch (err) {
            showToast("Current runtime wallet is unavailable: " + err, "error");
        }
    }

    async function openUnifiedImportPath(path) {
        unifiedImportPath = path;
        unifiedImportDetectedType = null;
        unifiedImportDetection = null;
        unifiedImportWorking = true;
        unifiedImportError = "";
        unifiedImportResult = null;
        unifiedImportConfirmChecked = false;
        unifiedImportMigrationPassphrase = "";
        unifiedImportSnapshotLabel = "";
        unifiedImportSnapshotWorking = false;
        unifiedImportSnapshotResult = null;
        unifiedImportSnapshotError = "";
        unifiedPromotionNeedsRuntimeLoad = false;
        showUnifiedImportModal = true;

        try {
            try {
                const vaultDetection = await core.invoke("vault_validate_import_bundle", { path });
                unifiedImportDetection = vaultDetection;
                unifiedImportDetectedType = "hemp0x_vault";
                return;
            } catch (_) {
                // Not a Hemp0x Vault JSON; continue with wallet/migration detection.
            }
            const detection = await core.invoke("detect_wallet_file_type", { path });
            unifiedImportDetection = detection;
            unifiedImportDetectedType = detection.detected_type;
        } catch (err) {
            unifiedImportError = String(err);
            unifiedImportDetectedType = "unknown";
        } finally {
            unifiedImportWorking = false;
        }
    }

    async function executeUnifiedVaultImport() {
        if (!unifiedImportPath) return;
        showUnifiedImportModal = false;
        if (hasVault) {
            switchImportPendingPath = unifiedImportPath;
            switchImportPendingArchive = true;
            switchImportArchiveArmed = true;
            return;
        }
        await doImportVault(unifiedImportPath, false);
    }

    async function executeUnifiedLegacyImport() {
        if (!unifiedImportPath || !unifiedImportConfirmChecked) return;
        unifiedImportWorking = true;
        unifiedImportError = "";
        unifiedImportResult = null;
        try {
            const result = await core.invoke("restore_legacy_wallet_dat", {
                path: unifiedImportPath,
                restartNode: true,
            });
            unifiedImportResult = result;
            await refreshWalletHeader();
            await loadAlignmentStatus();
            await loadVaultWalletRecords();
            if (result.already_active) {
                showToast("Already using this wallet.dat. Switched to legacy wallet mode.", "success");
            } else {
                showToast("Legacy wallet.dat imported successfully.", "success");
            }
            if (result.hemp_conf_wallet) {
                showToast("Warning: hemp.conf contains wallet=" + result.hemp_conf_wallet + " — Core may load that wallet instead of wallet.dat.", "warning", false);
            }
            if (result.restart_error) {
                unifiedImportError = result.restart_error;
                showToast(result.restart_error, "error", false);
            }
            return result;
        } catch (err) {
            unifiedImportError = String(err);
            showToast("Legacy wallet import failed: " + err, "error");
            return null;
        } finally {
            unifiedImportWorking = false;
        }
    }

    async function loadSelectedWalletFileAsRuntime() {
        if (!unifiedImportPath || unifiedImportDetectedType !== "legacy_core_wallet") return;
        unifiedPromotionError = "";
        unifiedPromotionNeedsRuntimeLoad = false;
        const selectedName = vaultBasenameFromPath(unifiedImportPath);
        if (selectedName && selectedName !== "wallet.dat") {
            const config = await core.invoke("init_config");
            const selectedDir = normalizePathForCompare(dirnameFromPath(unifiedImportPath));
            const dataDir = normalizePathForCompare(config?.data_dir);
            if (selectedDir !== dataDir) {
                unifiedPromotionError = "Named Core runtime wallets must be inside the Hemp0x Core data folder. Use the legacy wallet option below for external wallet.dat files.";
                return;
            }
            unifiedImportWorking = true;
            try {
                await core.invoke("vault_restart_core_with_wallet", { walletName: selectedName });
                activeVaultWalletName = selectedName;
                await refreshWalletHeader();
                await loadAlignmentStatus();
                unifiedPromotionError = "This wallet is now loaded as the Core runtime wallet. Click Add to Hemp0x Vault again to promote it into the portable vault.";
                showToast("Core is using the selected runtime wallet.", "success");
            } catch (err) {
                unifiedPromotionError = "Could not load selected runtime wallet: " + String(err);
                showToast("Runtime wallet load failed: " + err, "error", false);
            } finally {
                unifiedImportWorking = false;
            }
            return;
        }
        unifiedImportConfirmChecked = true;
        const result = await executeUnifiedLegacyImport();
        if (result && !result.restart_error) {
            unifiedPromotionError = "wallet.dat is now loaded as the Core runtime wallet. Click Add to Hemp0x Vault again to promote it into the portable vault.";
        }
    }

    async function executeUnifiedSnapshotImport() {
        if (!unifiedImportPath || !unifiedImportSnapshotLabel) {
            unifiedImportSnapshotError = "File and label are required.";
            return;
        }
        const explicit = vaultUnlocked ? null : (unifiedImportMigrationPassphrase || null);
        if (!vaultUnlocked && !unifiedImportMigrationPassphrase) {
            unifiedImportSnapshotError = "Vault passphrase is required (or unlock the vault first).";
            return;
        }
        unifiedImportSnapshotWorking = true;
        unifiedImportSnapshotError = "";
        unifiedImportSnapshotResult = null;
        try {
            const result = await core.invoke(
                "ipfs_vault_import_wallet_migration_record_from_path",
                {
                    path: unifiedImportPath,
                    label: unifiedImportSnapshotLabel,
                    migrationPassphrase: unifiedImportMigrationPassphrase || null,
                    vaultPassphrase: explicit,
                },
            );
            unifiedImportSnapshotResult = result;
            unifiedImportSnapshotLabel = "";
            unifiedImportMigrationPassphrase = "";
            vaultWalletRecordsLoaded = false;
            await loadVaultWalletRecords();
            showToast("Migration envelope stored as vault recovery snapshot.", "success");
        } catch (err) {
            unifiedImportSnapshotError = String(err);
        }
        unifiedImportSnapshotWorking = false;
    }

    // --- CORE-TO-WEBCOM PROMOTION (66b) ---
    async function executeUnifiedPromotion() {
        if (!unifiedImportPath) {
            unifiedPromotionError = "No file selected.";
            return;
        }
        if (!vaultUnlocked) {
            unifiedPromotionError = "Unlock the vault first.";
            return;
        }
        unifiedPromotionWorking = true;
        unifiedPromotionError = "";
        unifiedPromotionNeedsRuntimeLoad = false;
        unifiedPromotionResult = null;
        unifiedPromotionStep = 0;
        unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[0];
        await tick();

        let vaultUpdated = false;
        try {
            let result;
            unifiedPromotionStep = 1;
            unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[1];
            await tick();
            if (unifiedImportDetectedType === "core_migration_envelope") {
                if (!unifiedPromotionPassphrase) {
                    unifiedPromotionError = "Migration passphrase is required for encrypted envelopes.";
                    unifiedPromotionWorking = false;
                    unifiedPromotionProgressLabel = "";
                    return;
                }
                result = await core.invoke("ipfs_vault_promote_core_migration_to_portable_primary", {
                    path: unifiedImportPath,
                    migrationPassphrase: unifiedPromotionPassphrase,
                    vaultPassphrase: null,
                    replaceExistingPrimary: unifiedPromotionReplaceExisting,
                    runtimeWalletName: unifiedPromotionWalletName || null,
                    walletUnlockPassphrase: unifiedPromotionWalletUnlockPass || null,
                });
            } else if (unifiedImportDetectedType === "legacy_core_wallet") {
                result = await core.invoke("ipfs_vault_promote_core_wallet_to_portable_primary", {
                    path: unifiedImportPath,
                    vaultPassphrase: null,
                    replaceExistingPrimary: unifiedPromotionReplaceExisting,
                    runtimeWalletName: unifiedPromotionWalletName || null,
                    walletUnlockPassphrase: unifiedPromotionWalletUnlockPass || null,
                });
            } else {
                unifiedPromotionError = "This file type cannot be promoted to a portable vault wallet.";
                unifiedPromotionWorking = false;
                unifiedPromotionProgressLabel = "";
                return;
            }

            vaultUpdated = true;
            // CF10: clear secret-bearing variables immediately
            unifiedPromotionPassphrase = "";
            unifiedPromotionWalletUnlockPass = "";

            const walletName = result?.core_wallet_name;
            if (!walletName) {
                throw new Error("The vault was updated, but Core did not return the runtime wallet name.");
            }

            unifiedPromotionStep = 2;
            unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[2];
            await tick();
            await core.invoke("vault_set_active_wallet_name", { walletName });

            unifiedPromotionStep = 3;
            unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[3];
            await tick();
            const loadedState = await ensureNamedCoreWalletLoaded(walletName, "Promoted vault wallet");

            unifiedPromotionStep = 4;
            unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[4];
            await tick();
            result.named_wallet_loaded = true;
            result.wallet_load_restart_required = false;
            result.wallet_state = loadedState;
            unifiedPromotionResult = result;

            await new Promise((resolve) => setTimeout(resolve, 250));
            await Promise.allSettled([
                refreshWalletHeader(),
                loadAlignmentStatus(),
                loadVaultWalletRecords(),
            ]);

            unifiedPromotionStep = 5;
            unifiedPromotionProgressLabel = UNIFIED_PROMOTION_STEPS[5];
            await tick();
            pendingHistoryRecoveryWallet = walletName;

            showToast("Wallet promoted to portable Hemp0x Vault primary record.", "success");
            await autosaveActiveVaultExport("Vault primary wallet changed");
        } catch (err) {
            const errStr = String(err);
            if (errStr.startsWith("WALLET_UNLOCK_REQUIRED::")) {
                unifiedPromotionError = "Core wallet is locked. Enter the wallet unlock passphrase below.";
            } else if (errStr.startsWith("EXTERNAL_FILE_NOT_LOADED::")) {
                const detail = errStr.split("::").slice(1).join("::");
                unifiedPromotionError = `${detail} Load this file as the current Core runtime wallet first, then run Add to Hemp0x Vault again.`;
                unifiedPromotionNeedsRuntimeLoad = unifiedImportDetectedType === "legacy_core_wallet";
            } else if (vaultUpdated) {
                unifiedPromotionError = `The wallet was added to the Hemp0x Vault, but Core could not finish loading it. ${errStr}`;
            } else {
                unifiedPromotionError = errStr;
            }
            showToast(vaultUpdated ? "Vault updated, but Core activation failed." : "Promotion failed: " + errStr, "error", false);
        } finally {
            // CF10: clear secrets on all exit paths
            unifiedPromotionPassphrase = "";
            unifiedPromotionWalletUnlockPass = "";
            unifiedPromotionWorking = false;
            unifiedPromotionProgressLabel = "";
            unifiedPromotionStep = 0;
        }
    }

    // --- ADVANCED EXPORT (66b) ---
    async function openAdvancedExportModal() {
        advancedExportPath = "";
        advancedExportPassphrase = "";
        advancedExportOverwrite = false;
        advancedExportWalletUnlockPass = "";
        advancedExportWorking = false;
        advancedExportError = "";
        advancedExportResult = null;
        showAdvancedExportModal = true;
    }

    function closeAdvancedExportModal() {
        showAdvancedExportModal = false;
        advancedExportPath = "";
        advancedExportPassphrase = "";
        advancedExportOverwrite = false;
        advancedExportWalletUnlockPass = "";
        advancedExportWorking = false;
        advancedExportError = "";
        advancedExportResult = null;
    }

    async function selectAdvancedExportPath() {
        try {
            const selected = await save({
                title: "Export Core Migration Wallet",
                defaultPath: "hemp0x_migration",
            });
            if (selected) {
                advancedExportPath = selected;
            }
        } catch (e) { /* user cancelled */ }
    }

    async function executeAdvancedExport() {
        if (!advancedExportPath) {
            advancedExportError = "Select a destination file first.";
            return;
        }
        if (!advancedExportPassphrase || advancedExportPassphrase.length < 8) {
            advancedExportError = "Export passphrase must be at least 8 characters.";
            return;
        }
        advancedExportWorking = true;
        advancedExportError = "";
        advancedExportResult = null;
        try {
            const result = await core.invoke("vault_export_core_migration_wallet", {
                destPath: advancedExportPath,
                exportPassphrase: advancedExportPassphrase,
                allowOverwrite: advancedExportOverwrite,
                walletUnlockPassphrase: advancedExportWalletUnlockPass || null,
            });
            advancedExportResult = result;
            advancedExportPassphrase = "";
            advancedExportWalletUnlockPass = "";
            showToast("Core migration wallet exported successfully.", "success");
        } catch (err) {
            const errStr = String(err);
            if (errStr.startsWith("WALLET_UNLOCK_REQUIRED::")) {
                advancedExportError = "Core wallet is locked. Enter the wallet unlock passphrase below.";
            } else {
                advancedExportError = errStr;
            }
            showToast("Export failed: " + errStr, "error");
        } finally {
            advancedExportPassphrase = "";
            advancedExportWalletUnlockPass = "";
            advancedExportWorking = false;
        }
    }

    function closeUnifiedImportModal() {
        showUnifiedImportModal = false;
        unifiedImportPath = "";
        unifiedImportDetectedType = null;
        unifiedImportDetection = null;
        unifiedImportWorking = false;
        unifiedImportError = "";
        unifiedImportResult = null;
        unifiedImportConfirmChecked = false;
        unifiedImportMigrationPassphrase = "";
        unifiedImportSnapshotLabel = "";
        unifiedImportSnapshotWorking = false;
        unifiedImportSnapshotResult = null;
        unifiedImportSnapshotError = "";
        // 66b: promotion state
        unifiedPromotionWorking = false;
        unifiedPromotionError = "";
        unifiedPromotionResult = null;
        unifiedPromotionPassphrase = "";
        unifiedPromotionWalletName = "";
        unifiedPromotionReplaceExisting = false;
        unifiedPromotionWalletUnlockPass = "";
        unifiedPromotionProgressLabel = "";
        unifiedPromotionStep = 0;
        unifiedPromotionNeedsRuntimeLoad = false;
    }

    async function loadWalletDatExists() {
        if (!tauriReady) return;
        try {
            const info = await withTimeout(core.invoke("get_data_folder_info"), 8000, "Data folder status");
            walletDatExists = !!info?.wallet_exists;
        } catch (err) {
            walletDatExists = null;
        }
    }

    async function loadWalletStatus(background = false) {
        if (!tauriReady) return;
        if (historyRecoveryBackgroundActive) return;
        if (!background) {
            walletStatusLoading = true;
            walletStatusError = "";
        }
        try {
            const res = await withTimeout(core.invoke("rpc_get_wallet_info"), 8000, "Wallet status");
            if (res && res.success) {
                walletStatus = res.data;
            } else {
                walletStatus = null;
                walletStatusError = res?.error || "Wallet info unavailable";
            }
        } catch (err) {
            if (!background) {
                walletStatus = null;
                const msg = String(err);
                if (msg.includes("Wallet file not specified") || msg.includes("/wallet/")) {
                    walletStatusError = "";
                } else {
                    walletStatusError = msg;
                }
            }
        } finally {
            if (!background) {
                walletStatusLoading = false;
            }
        }
    }

    function isRuntimeWalletEncrypted(status) {
        if (!status) return false;
        if (typeof status.encrypted === "boolean") return status.encrypted;
        return Object.prototype.hasOwnProperty.call(status, "unlocked_until");
    }

    function isRuntimeWalletUnlocked(status) {
        if (typeof status?.locked === "boolean" && isRuntimeWalletEncrypted(status)) {
            return !status.locked;
        }
        const unlockedUntil = Number(status?.unlocked_until ?? 0);
        return isRuntimeWalletEncrypted(status) && unlockedUntil > 0;
    }

    async function loadVaultOverview() {
        if (!tauriReady) return;
        vaultOverviewLoading = true;
        if (vaultOverview === null) {
            vaultOverviewError = null;
        }
        try {
            vaultOverview = await withTimeout(core.invoke("vault_get_vault_overview"), 8000, "Vault status");
            vaultOverviewError = "";
        } catch (err) {
            vaultOverviewError = String(err);
        } finally {
            vaultOverviewLoading = false;
        }
    }

    async function refreshWalletHeader() {
        try {
            await Promise.all([
                loadWalletStatus(),
                loadWalletDatExists(),
                loadVaultOverview(),
                loadVaultUnlockStatus(),
                loadActiveVaultWalletName(),
            ]);
        } catch (e) {
            console.error("refreshWalletHeader failed:", e);
            vaultOverviewError = String(e);
        }
    }

    async function loadActiveVaultWalletName() {
        if (!tauriReady) return;
        try {
            const result = await withTimeout(core.invoke("vault_get_active_wallet_name"), 8000, "Active wallet name");
            activeVaultWalletName = result;
        } catch (e) {
            activeVaultWalletName = null;
        }
    }

    async function useVaultWallet(walletName) {
        if (!walletName) {
            showToast("No vault wallet name is available.", "error");
            return;
        }
        walletSwitchWorking = true;
        walletSwitchTargetName = walletName;
        vaultPageError = "";
        try {
            await core.invoke("vault_restart_core_with_wallet", { walletName });
            activeVaultWalletName = walletName;
            for (let i = 0; i < 8; i++) {
                await refreshWalletHeader();
                await loadAlignmentStatus();
                if (walletStatus?.walletname === walletName) {
                    break;
                }
                await new Promise((r) => setTimeout(r, 750));
            }
            showToast("Core is using the vault wallet.", "success");
        } catch (e) {
            showToast("Vault wallet switch failed: " + e, "error");
        } finally {
            walletSwitchWorking = false;
            walletSwitchTargetName = "";
        }
    }

    function isVaultWalletActive(walletName) {
        return !!walletName && walletStatus?.walletname === walletName;
    }

    function activeRuntimeWalletLabel() {
        if (walletStatus?.walletname && walletStatus.walletname !== "default") return walletStatus.walletname;
        if (activeVaultWalletName) return `${activeVaultWalletName} (selected for restart)`;
        return "wallet.dat";
    }

    async function useWalletDatRuntime() {
        walletSwitchWorking = true;
        walletSwitchTargetName = "wallet.dat";
        vaultPageError = "";
        try {
            const result = await withTimeout(
                core.invoke("switch_to_legacy_wallet_dat", { restartNode: true }),
                120000,
                "Switch to wallet.dat",
            );
            await refreshWalletHeader();
            await loadAlignmentStatus();
            if (result?.restart_error) {
                vaultPageError = result.restart_error;
                showToast(result.restart_error, "warning", false);
            } else {
                showToast("Core is using wallet.dat. Your Hemp0x Vault remains unlocked.", "success");
            }
            if (result?.hemp_conf_wallet) {
                showToast("Warning: hemp.conf contains wallet=" + result.hemp_conf_wallet + " — Core may load that wallet instead of wallet.dat.", "warning", false);
            }
        } catch (e) {
            vaultPageError = "Could not switch to wallet.dat: " + String(e);
            showToast("wallet.dat switch failed: " + e, "error");
        } finally {
            walletSwitchWorking = false;
            walletSwitchTargetName = "";
        }
    }

    function alignedVaultWalletName() {
        return alignmentStatus?.core_wallet_name || "hemp0x-vault-main";
    }

    async function loadVaultUnlockStatus() {
        if (!tauriReady) return;
        vaultUnlockedLoading = true;
        try {
            const res = await withTimeout(core.invoke("ipfs_vault_unlock_status"), 8000, "Vault unlock status");
            if (!res || !res.unlocked) {
                vaultUnlocked = false;
            } else {
                vaultUnlocked = true;
            }
        } catch (err) {
            // Preserve optimistic unlock if already set
            if (!vaultUnlocked) {
                vaultUnlocked = false;
            }
        } finally {
            vaultUnlockedLoading = false;
        }
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
            const requestedName = createVaultName;
            await core.invoke("ipfs_vault_setup_and_unlock", {
                passphrase: createVaultPassphrase,
            });
            createVaultPassphrase = "";
            createVaultPassphraseConfirm = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultWalletRecordsMsg = "";
            if (requestedName) {
                try { await core.invoke("vault_set_vault_label", { label: requestedName }); } catch (_) {}
                createVaultName = "";
            }
            await refreshWalletHeader();
            vaultStatus.set({ exists: true, unlocked: true });
            showToast("Vault created and unlocked for this session.", "success");
            try {
                await promptSaveActiveVault(requestedName);
            } catch (saveErr) {
                if (String(saveErr) === "No file selected") {
                    notifyVaultNeedsSave("Vault created");
                } else {
                    showToast("Vault created, but portable save failed: " + saveErr, "warning", false);
                    notifyVaultNeedsSave("Vault created");
                }
            }
        } catch (err) {
            createVaultError = String(err);
        }
        createVaultWorking = false;
    }

    async function handlePostVaultUnlockConnection() {
        try {
            await refreshWalletHeader();
            await loadAlignmentStatus();
            if (!justConnected && alignmentStatus?.wallet_record_state === "webcom_primary_detected") {
                const target = alignedVaultWalletName();
                if (alignmentStatus.connection_state === "verified_aligned" && target && !isVaultWalletActive(target)) {
                    await useVaultWallet(target);
                }
            }
        } catch (e) {
            console.error("post-vault-unlock connection check failed:", e);
        }
    }

    async function executeUnlockVault() {
        if (!unlockVaultPassphrase) {
            unlockVaultError = "Enter your vault passphrase.";
            unlockVaultErrorDetails = "";
            return;
        }
        unlockVaultWorking = true;
        unlockVaultError = "";
        unlockVaultErrorDetails = "";
        vaultPageError = "";
        try {
            const ok = await withTimeout(
                core.invoke("ipfs_unlock_vault", { passphrase: unlockVaultPassphrase }),
                12000,
                "Vault unlock",
            );
            unlockVaultPassphrase = "";
            unlockVaultWorking = false;
            if (ok) {
                vaultUnlocked = true;
                vaultStatus.set({ exists: true, unlocked: true });
                refreshWalletHeader().catch((e) => {
                    vaultPageError = "Wallet status refresh failed: " + String(e);
                    console.error("refreshWalletHeader failed:", e);
                });
                loadVaultWalletRecords().catch((e) => {
                    vaultPageError = (vaultPageError ? vaultPageError + " | " : "") + "Backup record list failed: " + String(e);
                    console.error("loadVaultWalletRecords failed:", e);
                });
                handlePostVaultUnlockConnection().catch(() => {});
            } else {
                unlockVaultError = "Incorrect passphrase.";
                unlockVaultErrorDetails = "The vault could not be decrypted with the passphrase you entered. Verify it and try again.";
            }
        } catch (err) {
            unlockVaultWorking = false;
            const msg = String(err);
            unlockVaultError = "Command 'ipfs_unlock_vault' failed: " + msg;
            unlockVaultErrorDetails = "Check the browser console (F12) for details. Verify the vault file exists and the passphrase is correct.";
            if (msg.includes("Vault does not exist")) {
                unlockVaultError = "No vault file found.";
                unlockVaultErrorDetails = "Create a new vault or import an existing one from the options below.";
            }
            console.error("ipfs_unlock_vault error:", msg);
        }
    }

    async function executeLockVault() {
        try {
            await core.invoke("ipfs_lock_vault");
            resetUnlockedVaultState();
            vaultStatus.set({ exists: hasVault, unlocked: false });
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

    function normalizePathForCompare(p) {
        return String(p || "").replace(/\\/g, "/").replace(/\/+$/, "");
    }

    function formatBytes(n) {
        const num = Number(n);
        if (!num || num < 0) return "—";
        if (num < 1024) return `${num} B`;
        if (num < 1024 * 1024) return `${(num / 1024).toFixed(1)} KB`;
        return `${(num / (1024 * 1024)).toFixed(2)} MB`;
    }

    // --- WEBCOM / HEMP0X VAULT INTEROP (63) ---


    // --- WALLET ALIGNMENT STATE (64c) ---
    let alignmentStatus = null;
    let alignmentStatusLoading = false;
    let alignmentStatusError = "";
    let alignmentConnectPlan = null;
    let alignmentConnectPlanLoading = false;
    let alignmentConnectPlanError = "";
    let showAlignmentDetails = false;





    async function loadAlignmentStatus() {
        if (!tauriReady || !vaultUnlocked) return;
        if (alignmentStatusLoading) return;
        alignmentStatusLoading = true;
        alignmentStatusError = "";
        try {
            alignmentStatus = await withTimeout(
                core.invoke("ipfs_vault_get_wallet_alignment_status_v2", {
                    vaultPassphrase: null,
                }),
                12000,
                "Vault wallet alignment status",
            );
        } catch (err) {
            alignmentStatus = null;
            alignmentStatusError = String(err);
        } finally {
            alignmentStatusLoading = false;
        }
    }

    async function loadAlignmentConnectPlan() {
        if (!tauriReady || !vaultUnlocked) return;
        alignmentConnectPlanLoading = true;
        alignmentConnectPlanError = "";
        try {
            alignmentConnectPlan = await withTimeout(
                core.invoke("ipfs_vault_preview_connect_webcom_primary_to_core", {
                    vaultPassphrase: null,
                }),
                12000,
                "Vault wallet connect plan",
            );
        } catch (err) {
            alignmentConnectPlan = null;
            alignmentConnectPlanError = String(err);
        } finally {
            alignmentConnectPlanLoading = false;
        }
    }

    let showAlignmentReviewModal = false;

    // Connect execution state (slice 64d/64e/64f/64g)
    let connectWorking = false;
    let connectStep = 0;
    let connectError = "";
    let connectErrorCode = "";
    let connectResult = null;
    let connectWalletName = "hemp0x-vault-main";
    let connectWalletNameError = "";
    $: connectWalletNameError = validateWalletName(connectWalletName);
    let connectConfirmChecked = false;

    const CONNECT_STEPS = [
        "Decrypting vault",
        "Backing up current Core wallet",
        "Preparing Core restore",
        "Checking restore data",
        "Restoring wallet into Core",
        "Verifying restore result",
        "Writing alignment record",
        "Restarting Core with vault wallet",
        "Loading wallet into Core",
    ];

    function isGuidedUnlockRequiredError(err) {
        const text = String(err || "");
        return text.startsWith("WALLET_UNLOCK_REQUIRED::") || isWalletUnlockError(err);
    }

    function isRestoreTimeoutError(err) {
        const text = String(err || "");
        return text.startsWith("RESTORE_TIMEOUT::");
    }

    function reviewAlignment() {
        showAlignmentReviewModal = true;
        connectError = "";
        connectErrorCode = "";
        connectResult = null;
        connectConfirmChecked = false;
    }

    async function executeConnectVaultWallet() {
        if (!connectConfirmChecked) {
            connectError = "Tick the confirmation box to continue.";
            return;
        }
        if (connectWalletNameError) {
            connectError = connectWalletNameError;
            return;
        }
        connectWorking = true;
        connectStep = 0;
        connectError = "";
        connectErrorCode = "";
        connectResult = null;
        let stepperTimer = null;
        try {
            await tick();
            stepperTimer = setInterval(() => {
                if (connectStep < CONNECT_STEPS.length - 1) {
                    connectStep += 1;
                }
            }, 1500);
            await new Promise((r) => setTimeout(r, 50));

        connectResult = await core.invoke("ipfs_vault_connect_webcom_primary_wallet_to_core_guided", {
            walletName: connectWalletName || null,
            birthHeight: null,
            vaultPassphrase: null,
            skipBackup: skipBackup,
        });
        clearInterval(stepperTimer);
        connectStep = CONNECT_STEPS.length;
        // Persist the active wallet name immediately so a later
        // refresh/history failure cannot leave the vault connected but
        // not selected for startup. The backend also does this, but we
        // set it again here before any other IPC for safety.
        if (connectResult?.core_wallet_name) {
            try {
                await core.invoke("vault_set_active_wallet_name", { walletName: connectResult.core_wallet_name });
                activeVaultWalletName = connectResult.core_wallet_name;
            } catch (e) {
                console.warn("vault_set_active_wallet_name failed after connect:", e);
            }
        }
        if (connectResult?.wallet_load_restart_required) {
            connectStep += 1;
            const newState = await ensureNamedCoreWalletLoaded(connectResult.core_wallet_name, "Connected vault wallet");
            if (newState?.loaded) {
                connectResult.wallet_load_restart_required = false;
                connectResult.named_wallet_loaded = true;
            } else if (newState && newState.restart_required) {
                // Core could not make the named wallet queryable. Keep
                // connectWorking true and surface a persistent, honest
                // error instead of claiming success. The user must
                // restart Core to load the vault wallet.
                connectWorking = false;
                connectError = `Vault wallet was restored, but Core still needs a restart to load "${connectResult.core_wallet_name}". Close this dialog, then use Restart Core With Vault Wallet, or restart Commander.`;
                connectErrorCode = "restart_required";
                await refreshWalletHeader().catch(() => {});
                await loadAlignmentStatus().catch(() => {});
                await loadVaultWalletRecords().catch(() => {});
                justConnected = true;
                return;
            }
        }
        await new Promise((r) => setTimeout(r, 250));
        // Final verification: ask the backend whether the active vault
        // wallet is actually queryable now. If not, surface a clear
        // persistent error instead of a silent "success".
        try {
            const startup = await core.invoke("vault_get_active_wallet_startup_state");
            if (startup?.active_wallet_name && !startup?.wallet_queryable) {
                // Reflect the honest state in the result object so the
                // success view does not claim the wallet is loaded.
                if (connectResult) {
                    connectResult.named_wallet_loaded = false;
                    connectResult.wallet_load_restart_required = true;
                }
                connectWorking = false;
                connectError = startup?.load_error
                    || `Vault wallet "${startup.active_wallet_name}" is not loaded in Core yet. Restart Core through Commander to load it. No changes were made to hemp.conf.`;
                connectErrorCode = "wallet_not_loaded";
                justConnected = true;
                return;
            }
        } catch (e) {
            // Best-effort verification; do not block success on a probe failure.
            console.warn("post-connect startup state probe failed:", e);
        }
        justConnected = true;
        connectWorking = false;
        if (connectResult?.named_wallet_loaded && connectResult?.core_wallet_name) {
            pendingHistoryRecoveryWallet = connectResult.core_wallet_name;
        }
        await tick();

        // Run non-critical refreshes after the success view has painted so the
        // modal does not appear frozen while wallet status/alignment records
        // refresh and history recovery starts.
        setTimeout(() => {
            refreshWalletHeader().catch((e) => {
                vaultPageError = "Wallet status refresh failed: " + String(e);
            });
            loadAlignmentStatus().catch((e) => {
                console.warn("post-connect alignment refresh failed:", e);
            });
            loadVaultWalletRecords().catch((e) => {
                vaultPageError = (vaultPageError ? vaultPageError + " | " : "") + "Backup record list failed: " + String(e);
            });
        }, 100);
        } catch (err) {
            if (stepperTimer) clearInterval(stepperTimer);
            const rawErr = String(err);
            if (isGuidedUnlockRequiredError(err)) {
                connectWorking = false;
                connectErrorCode = "";
                connectError = "";
                requestCoreWalletUnlockForGuidedConnect();
                showToast("Unlock the current Core wallet to continue connecting the vault wallet.", "info", false);
                return;
            } else if (isRestoreTimeoutError(err)) {
                connectErrorCode = "restore_timeout";
                const parts = rawErr.split("::");
                const inner = parts.length >= 3 ? parts.slice(2).join("::") : rawErr;
                connectError = `Core did not answer before Commander could confirm the vault wallet restore. The restore may still be finishing, or it may have completed already. Wait a few minutes, then retry the guided connect. If the wallet already exists, Commander will detect it. Details: ${inner}`;
            } else {
                connectError = rawErr;
            }
            showToast(`Connect failed: ${rawErr}`, "error");
        }
        connectWorking = false;
    }

    function requestCoreWalletUnlockForGuidedConnect() {
        unlockModalPurpose = "vault_guided_connect";
        unlockAfterWalletUnlock = async () => {
            await executeConnectVaultWallet();
        };
        unlockPassword = "";
        unlockError = "";
        showUnlockModal = true;
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
        showVaultArchiveModal = true;
    }

    function sanitizeVaultExportStem(input) {
        const clean = String(input || "")
            .trim()
            .replace(/[^a-zA-Z0-9_-]+/g, "-")
            .replace(/^-+|-+$/g, "");
        return clean || `vault-${new Date().toISOString().slice(0, 10)}`;
    }

    function dirnameFromPath(path) {
        const text = String(path || "");
        const idx = Math.max(text.lastIndexOf("/"), text.lastIndexOf("\\"));
        return idx > 0 ? text.slice(0, idx) : "";
    }

    function vaultExportDefaultPath(labelOverride = "") {
        const stem = sanitizeVaultExportStem(labelOverride || vaultOverview?.display_label || "");
        const dir = dirnameFromPath(vaultOverview?.vault_path);
        return dir ? `${dir}/${stem}.json` : `${stem}.json`;
    }

    async function promptSaveActiveVault(labelOverride = "") {
        const raw = await core.invoke("vault_read_raw_content");
        const savedPath = await core.invoke("dialog_write_text_file", {
            content: raw,
            defaultPath: vaultExportDefaultPath(labelOverride),
            title: "Save Hemp0x Vault",
            filters: [["Hemp0x Vault", "json"]],
        });
        try {
            await core.invoke("vault_set_active_export_path", { path: savedPath });
        } catch (err) {
            showToast("Vault saved, but automatic future saves were not enabled: " + err, "warning", false);
        }
        showToast(`Vault saved to ${vaultBasenameFromPath(savedPath)}.`, "success");
        return savedPath;
    }

    async function autosaveActiveVaultExport(reason = "Vault contents changed") {
        try {
            const result = await core.invoke("vault_autosave_active_export_path");
            if (result?.saved) {
                showToast(`${reason}. Saved to ${vaultBasenameFromPath(result.path)}.`, "success", false);
                return true;
            }
            notifyVaultNeedsSave(reason);
            return false;
        } catch (err) {
            showToast(`${reason}, but autosave failed: ${err}`, "warning", false);
            notifyVaultNeedsSave(reason);
            return false;
        }
    }

    async function executeArchiveVault() {
        showVaultArchiveModal = false;
        vaultArchiveWorking = true;
        try {
            await promptSaveActiveVault();
        } catch (err) {
            if (String(err) !== "No file selected") {
                showToast("Save failed: " + err, "error");
            }
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
            vaultWalletRecordsMsg = "";
            vaultStatus.set({ exists: true, unlocked: true });
            await refreshWalletHeader();
        } catch (err) {
            showToast("Could not create new vault: " + err, "error");
        }
    }

    function promptCreateNewVaultFromLocked() {
        // Open the sleek create-vault form directly so it visually matches the
        // import / unlock flow instead of the older generic confirmation modal.
        createVaultFormPass = "";
        createVaultFormPassConfirm = "";
        createVaultFormName = "";
        createVaultFormError = "";
        createVaultFormWorking = false;
        showCreateVaultForm = true;
    }

    // --- VAULT SECURITY (64p: passphrase rotation + unload/fallback) ---

    function openChangeVaultPassphraseModal() {
        if (!vaultUnlocked || !hasVault) {
            showToast("Unlock your Hemp0x Vault first to rotate its passphrase.", "error");
            return;
        }
        changeVaultPassCurrent = "";
        changeVaultPassNew = "";
        changeVaultPassConfirm = "";
        changeVaultPassError = "";
        changeVaultPassResult = null;
        changeVaultPassWorking = false;
        showChangeVaultPassphraseModal = true;
    }

    function closeChangeVaultPassphraseModal() {
        if (changeVaultPassWorking) return;
        // Always clear passphrase inputs; never leave secrets in the DOM.
        changeVaultPassCurrent = "";
        changeVaultPassNew = "";
        changeVaultPassConfirm = "";
        changeVaultPassError = "";
        // Keep the success result visible until the user explicitly closes.
        if (!changeVaultPassResult) {
            showChangeVaultPassphraseModal = false;
        }
    }

    function dismissChangeVaultPassphraseSuccess() {
        changeVaultPassResult = null;
        showChangeVaultPassphraseModal = false;
    }

    function loadedRuntimeWalletName() {
        return walletStatus?.walletname || "";
    }

    function isDefaultRuntimeWalletName(name) {
        const normalized = String(name || "").trim();
        return !normalized || normalized === "default" || normalized === "wallet.dat";
    }

    async function executeChangeVaultPassphrase() {
        changeVaultPassError = "";
        changeVaultPassResult = null;
        if (!changeVaultPassCurrent) {
            changeVaultPassError = "Enter your current vault passphrase.";
            return;
        }
        if (!changeVaultPassNew || changeVaultPassNew.length < 8) {
            changeVaultPassError = "New passphrase must be at least 8 characters.";
            return;
        }
        if (changeVaultPassNew !== changeVaultPassConfirm) {
            changeVaultPassError = "New passphrase and confirmation do not match.";
            return;
        }
        if (changeVaultPassNew === changeVaultPassCurrent) {
            changeVaultPassError = "New passphrase must be different from the current passphrase.";
            return;
        }
        changeVaultPassWorking = true;
        try {
            const result = await withTimeout(
                core.invoke("vault_change_passphrase", {
                    currentPassphrase: changeVaultPassCurrent,
                    newPassphrase: changeVaultPassNew,
                }),
                60000,
                "Vault passphrase rotation",
            );
            changeVaultPassResult = result;
            // Clear all passphrase inputs on success. The vault stays
            // unlocked because the backend updates the cached session
            // passphrase after the save succeeds.
            changeVaultPassCurrent = "";
            changeVaultPassNew = "";
            changeVaultPassConfirm = "";
            showToast("Vault passphrase changed.", "success");
            await autosaveActiveVaultExport("Vault passphrase changed");
        } catch (err) {
            changeVaultPassError = String(err);
            showToast("Vault passphrase change failed: " + err, "error");
        } finally {
            changeVaultPassWorking = false;
        }
    }

    function openChangeRuntimeWalletPassModal() {
        if (!walletStatus || !isRuntimeWalletEncrypted(walletStatus)) {
            showToast("Encrypt the runtime wallet before changing its password.", "error");
            return;
        }
        changeRuntimeWalletPassCurrent = "";
        changeRuntimeWalletPassNew = "";
        changeRuntimeWalletPassConfirm = "";
        changeRuntimeWalletPassError = "";
        changeRuntimeWalletPassResult = null;
        changeRuntimeWalletPassWorking = false;
        showChangeRuntimeWalletPassModal = true;
    }

    function closeChangeRuntimeWalletPassModal() {
        if (changeRuntimeWalletPassWorking) return;
        changeRuntimeWalletPassCurrent = "";
        changeRuntimeWalletPassNew = "";
        changeRuntimeWalletPassConfirm = "";
        changeRuntimeWalletPassError = "";
        if (!changeRuntimeWalletPassResult) {
            showChangeRuntimeWalletPassModal = false;
        }
    }

    function dismissChangeRuntimeWalletPassSuccess() {
        changeRuntimeWalletPassResult = null;
        showChangeRuntimeWalletPassModal = false;
    }

    async function executeChangeRuntimeWalletPassphrase() {
        changeRuntimeWalletPassError = "";
        changeRuntimeWalletPassResult = null;
        if (!walletStatus || !isRuntimeWalletEncrypted(walletStatus)) {
            changeRuntimeWalletPassError = "The loaded runtime wallet is not encrypted.";
            return;
        }
        if (!changeRuntimeWalletPassCurrent) {
            changeRuntimeWalletPassError = "Enter the current runtime wallet password.";
            return;
        }
        if (!changeRuntimeWalletPassNew || changeRuntimeWalletPassNew.length < 8) {
            changeRuntimeWalletPassError = "New wallet password must be at least 8 characters.";
            return;
        }
        if (changeRuntimeWalletPassNew !== changeRuntimeWalletPassConfirm) {
            changeRuntimeWalletPassError = "New wallet password and confirmation do not match.";
            return;
        }
        if (changeRuntimeWalletPassNew === changeRuntimeWalletPassCurrent) {
            changeRuntimeWalletPassError = "New wallet password must be different from the current password.";
            return;
        }

        changeRuntimeWalletPassWorking = true;
        const walletName = loadedRuntimeWalletName();
        try {
            if (isDefaultRuntimeWalletName(walletName)) {
                await core.invoke("change_wallet_password", {
                    oldPass: changeRuntimeWalletPassCurrent,
                    newPass: changeRuntimeWalletPassNew,
                });
            } else {
                await core.invoke("change_wallet_password_named", {
                    walletName,
                    oldPass: changeRuntimeWalletPassCurrent,
                    newPass: changeRuntimeWalletPassNew,
                });
            }
            changeRuntimeWalletPassCurrent = "";
            changeRuntimeWalletPassNew = "";
            changeRuntimeWalletPassConfirm = "";
            changeRuntimeWalletPassResult = {
                changed: true,
                wallet_name: walletName || "wallet.dat",
            };
            showToast("Runtime wallet password changed.", "success");
            if (hasVault) {
                await autosaveActiveVaultExport("Runtime wallet password changed");
            }
            await refreshWalletHeader();
        } catch (err) {
            changeRuntimeWalletPassError = String(err);
            showToast("Runtime wallet password change failed: " + err, "error");
        } finally {
            changeRuntimeWalletPassWorking = false;
        }
    }

    function openUnloadVaultModal() {
        if (!hasVault) {
            showToast("No active Hemp0x Vault to unload.", "error");
            return;
        }
        unloadVaultConfirmChecked = false;
        unloadVaultWorking = false;
        unloadVaultError = "";
        unloadVaultResult = null;
        showUnloadVaultModal = true;
    }

    function closeUnloadVaultModal() {
        if (unloadVaultWorking) return;
        unloadVaultConfirmChecked = false;
        unloadVaultError = "";
        if (!unloadVaultResult) {
            showUnloadVaultModal = false;
        }
    }

    function dismissUnloadVaultResult() {
        unloadVaultResult = null;
        showUnloadVaultModal = false;
    }

    async function executeUnloadVault() {
        if (!unloadVaultConfirmChecked) return;
        unloadVaultWorking = true;
        walletSwitchWorking = true;
        walletSwitchTargetName = "wallet.dat";
        unloadVaultError = "";
        unloadVaultResult = null;
        const previousVaultOverview = vaultOverview;
        const previousActiveVaultWalletName = activeVaultWalletName;
        const previousVaultUnlocked = vaultUnlocked;
        const previousVaultStatus = { exists: hasVault, unlocked: vaultUnlocked };
        resetUnlockedVaultState();
        activeVaultWalletName = null;
        vaultOverview = {
            exists: false,
            vault_path: previousVaultOverview?.vault_path ?? "",
            file_size: 0,
            file_modified: 0,
            display_label: "",
        };
        vaultOverviewError = "";
        vaultStatus.set({ exists: false, unlocked: false });
        try {
            const result = await withTimeout(
                core.invoke("vault_unload_vault_and_use_wallet_dat", { restartNode: true }),
                120000,
                "Unload vault / restart Core",
            );
            let archived = false;
            try {
                const archiveRes = await core.invoke("vault_archive_current_vault");
                if (archiveRes?.archive_path) {
                    result.archive_path = archiveRes.archive_path;
                    archived = true;
                }
            } catch (archiveErr) {
                result.archive_error = String(archiveErr);
            }

            // Clear in-memory unlocked state and flip the local vault
            // overview to "no active vault" BEFORE the reactive view
            // re-renders. Without this, the page briefly shows the locked
            // vault passphrase form (vaultUnlocked=false while the stale
            // vaultOverview still reports exists=true) until the refresh
            // below completes. When the vault file was archived, the
            // active vault no longer exists on disk, so set a synthetic
            // exists:false overview synchronously; refreshWalletHeader()
            // will replace it with the real on-disk state.
            vaultUnlocked = false;
            if (archived) {
                vaultOverview = {
                    exists: false,
                    vault_path: previousVaultOverview?.vault_path ?? "",
                    file_size: 0,
                    file_modified: 0,
                    display_label: "",
                };
                vaultOverviewError = "";
            }
            vaultStatus.set({ exists: !archived && hasVault, unlocked: false });
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultWalletRecordsError = "";
            vaultWalletRecordsMsg = "";

            // Keep the unload loader/working state active until the full
            // unload operation is actually complete: Core fallback/restart
            // finished, active vault archived, and wallet page state
            // refreshed. Only then switch the modal to the result view.
            await refreshWalletHeader();
            await loadActiveVaultWalletName();
            await loadWalletStatus();

            unloadVaultResult = result;

            if (result.no_legacy_wallet) {
                showToast("Vault unloaded. No wallet.dat was found — choose a next action below.", "warning", false);
            } else {
                showToast("Vault unloaded. Core restarted in legacy wallet.dat mode.", "success");
            }
            if (result.archive_error) {
                showToast("Vault was unloaded but could not be archived: " + result.archive_error, "warning", false);
            }
            if (result.hemp_conf_wallet) {
                showToast("Warning: hemp.conf has wallet=" + result.hemp_conf_wallet + " — Core may load that wallet instead of wallet.dat.", "warning", false);
            }
            if (result.restart_error) {
                unloadVaultError = result.restart_error;
            }
        } catch (err) {
            vaultOverview = previousVaultOverview;
            activeVaultWalletName = previousActiveVaultWalletName;
            vaultUnlocked = previousVaultUnlocked;
            vaultStatus.set(previousVaultStatus);
            await refreshWalletHeader().catch(() => {});
            unloadVaultError = String(err);
            showToast("Unload vault failed: " + err, "error");
        } finally {
            unloadVaultWorking = false;
            walletSwitchWorking = false;
            walletSwitchTargetName = "";
        }
    }

    let showCreateVaultForm = false;
    let createVaultFormPass = "";
    let createVaultFormPassConfirm = "";
    let createVaultFormName = "";
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
            const requestedName = createVaultFormName;
            if (hasVault) {
                await createNewVaultAfterArchive(createVaultFormPass);
            } else {
                await core.invoke("ipfs_vault_setup_and_unlock", {
                    passphrase: createVaultFormPass,
                });
                showToast("Vault created and unlocked for this session.", "success");
                vaultWalletRecords = [];
                vaultWalletRecordsLoaded = false;
                vaultWalletRecordsMsg = "";
                vaultStatus.set({ exists: true, unlocked: true });
                await refreshWalletHeader();
            }
            if (requestedName) {
                try { await core.invoke("vault_set_vault_label", { label: requestedName }); } catch (_) {}
                await refreshWalletHeader();
            }
            try {
                await promptSaveActiveVault(requestedName);
            } catch (saveErr) {
                if (String(saveErr) === "No file selected") {
                    notifyVaultNeedsSave("Vault created");
                } else {
                    showToast("Vault created, but portable save failed: " + saveErr, "warning", false);
                    notifyVaultNeedsSave("Vault created");
                }
            }
            showCreateVaultForm = false;
            createVaultFormPass = "";
            createVaultFormPassConfirm = "";
            createVaultFormName = "";
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
    let switchImportPendingArchive = true;

    // Import unlock popup — shown after Replace Loaded Vault
    let showImportUnlockPopup = false;
    let importUnlockPassphrase = "";
    let importUnlockWorking = false;
    let importUnlockError = "";
    let importUnlockErrorDetails = "";

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
                switchImportPendingPath = selected;
                switchImportPendingArchive = true;
                switchImportArchiveArmed = true;
            } else {
                await doImportVault(selected, false);
            }
        } catch (err) {
            showToast("Import failed: " + err, "error");
        }
    }

    async function doImportVault(path, archiveFirst) {
        try {
            if (archiveFirst && hasVault) {
                const archiveRes = await core.invoke("vault_archive_current_vault");
                const fileName = vaultBasenameFromPath(archiveRes?.archive_path);
                showToast(`Previous vault saved as ${fileName}.`, "info");
            }
            const result = await core.invoke("ipfs_vault_import_bundle_replace", {
                path,
                passphrase: null,
            });
            try {
                await core.invoke("vault_set_active_export_path", { path });
            } catch (_) {
                notifyVaultNeedsSave("Vault imported");
            }
            vaultUnlocked = false;
            vaultListPassphrase = "";
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultWalletRecordsError = "";
            vaultWalletRecordsMsg = "";
            selectedRecordId = "";
            vaultRestoreRecordId = "";
            vaultRestoreConfirmChecked = false;
            alignmentStatus = null;
            alignmentConnectPlan = null;
            connectResult = null;
            connectError = "";
            showVaultRestorePanel = false;
            vaultStatus.set({ exists: true, unlocked: false });
            showImportUnlockPopup = true;
            importUnlockPassphrase = "";
            importUnlockError = "";
            importUnlockErrorDetails = "";
            refreshWalletHeader().catch(() => {});
        } catch (err) {
            showToast("Import failed: " + err, "error");
        }
    }

    async function executeImportUnlock() {
        if (!importUnlockPassphrase) {
            importUnlockError = "Enter the passphrase for the imported vault.";
            importUnlockErrorDetails = "";
            return;
        }
        importUnlockWorking = true;
        importUnlockError = "";
        importUnlockErrorDetails = "";
        try {
            const ok = await withTimeout(
                core.invoke("ipfs_unlock_vault", { passphrase: importUnlockPassphrase }),
                12000,
                "Import vault unlock",
            );
            importUnlockPassphrase = "";
            if (ok) {
                showImportUnlockPopup = false;
                vaultUnlocked = true;
                vaultStatus.set({ exists: true, unlocked: true });
                handlePostVaultUnlockConnection().catch((e) => {
                    console.warn("Post-import vault connection check failed:", e);
                });
            } else {
                importUnlockError = "Incorrect passphrase.";
                importUnlockErrorDetails = "The imported vault could not be decrypted with the passphrase you entered.";
            }
        } catch (err) {
            importUnlockError = "Unlock failed.";
            importUnlockErrorDetails = String(err);
            console.error("Import unlock error:", String(err));
        } finally {
            importUnlockWorking = false;
        }
    }

    // --- RECOVERY PHRASE RESTORE ---
    async function executeRecoveryPhraseRestore() {
        const words = (recoveryPhraseWords || "").trim().split(/\s+/).filter(Boolean);
        if (words.length !== 12 && words.length !== 18 && words.length !== 24) {
            recoveryPhraseError = `Expected 12, 18, or 24 words; found ${words.length}.`;
            return;
        }
        // Wallet name is optional (defaults to hemp0x-vault-main), but if
        // the user enters one it must be a valid Core wallet filename.
        if (recoveryWalletName.trim() && recoveryWalletNameError) {
            recoveryPhraseError = recoveryWalletNameError;
            return;
        }
        // If vault is already unlocked, use the active session. Otherwise require a passphrase.
        const needsVaultPassphrase = !vaultUnlocked;
        if (needsVaultPassphrase && (!recoveryPhrasePassphrase || recoveryPhrasePassphrase.length < 8)) {
            recoveryPhraseError = "Vault passphrase must be at least 8 characters.";
            return;
        }
        if (needsVaultPassphrase && recoveryPhrasePassphrase !== recoveryPhrasePassphraseConfirm) {
            recoveryPhraseError = "Vault passphrases do not match.";
            return;
        }
        recoveryPhraseWorking = true;
        recoveryPhraseError = "";
        try {
            await tick();
            await new Promise((r) => setTimeout(r, 50));
            const result = await withTimeout(
                core.invoke("vault_restore_from_recovery_phrase", {
                    mnemonic: words.join(" "),
                    walletName: recoveryWalletName || null,
                    vaultPassphrase: needsVaultPassphrase ? recoveryPhrasePassphrase : "",
                    birthHeight: recoveryBirthHeight || null,
                }),
                120000,
                "Recovery phrase restore",
            );
            showRecoveryPhraseModal = false;
            recoveryPhraseWords = "";
            recoveryPhrasePassphrase = "";
            recoveryPhrasePassphraseConfirm = "";
            recoveryWalletName = "";
            recoveryBirthHeight = "";
            if (result?.vault_created_or_updated) {
                showToast("Wallet restored from recovery phrase. Your vault is unlocked for this session.", "success");
                await autosaveActiveVaultExport("Recovery wallet was added to your vault");
            } else {
                showToast("Wallet restored successfully.", "success");
            }

            // The restore just created/decrypted the vault and the
            // backend cached the passphrase, so the vault is unlocked
            // in-session. Flip local state synchronously so the Wallet
            // page immediately reflects the created/unlocked vault
            // instead of staying on the no-vault or locked view until
            // the user leaves and returns. A synthetic exists:true
            // overview bridges the render until refreshWalletHeader()
            // returns the real on-disk vault overview.
            vaultUnlocked = true;
            vaultStatus.set({ exists: true, unlocked: true });
            if (!hasVault) {
                vaultOverview = {
                    exists: true,
                    vault_path: vaultOverview?.vault_path ?? "",
                    file_size: 0,
                    file_modified: 0,
                    display_label: "",
                };
                vaultOverviewError = "";
            }
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultWalletRecordsMsg = "";
            await refreshWalletHeader();
            loadVaultWalletRecords().catch(() => {});
            loadAlignmentStatus().catch(() => {});

            // Load the restored wallet into Core through the existing
            // flow. This may restart Core, so show the wallet-switch
            // loader while it runs. A Core load failure does NOT undo
            // the successful restore; it is surfaced as a persistent
            // page error so the user can restart Core and retry.
            let coreLoadError = "";
            if (result?.core_wallet_name) {
                try {
                    await core.invoke("vault_set_active_wallet_name", { walletName: result.core_wallet_name });
                } catch (_) {}
                walletSwitchWorking = true;
                walletSwitchTargetName = result.core_wallet_name;
                try {
                    await ensureNamedCoreWalletLoaded(result.core_wallet_name, "Restored wallet");
                } catch (loadErr) {
                    coreLoadError = String(loadErr);
                } finally {
                    walletSwitchWorking = false;
                    walletSwitchTargetName = "";
                }
                await refreshWalletHeader().catch(() => {});
                loadAlignmentStatus().catch(() => {});
            }
            if (coreLoadError) {
                vaultPageError = coreLoadError;
                showToast(coreLoadError, "warning", false);
            }
        } catch (err) {
            recoveryPhraseError = String(err);
        }
        recoveryPhraseWorking = false;
    }

    async function executeCreateVaultWallet() {
        if (!createWalletPassphrase || createWalletPassphrase.length < 8) {
            createWalletError = "Vault passphrase must be at least 8 characters.";
            return;
        }
        if (createWalletName.trim() && createWalletNameError) {
            createWalletError = createWalletNameError;
            return;
        }
        createWalletWorking = true;
        createWalletError = "";
        try {
            await tick();
            await new Promise((r) => setTimeout(r, 50));
            const gen = await core.invoke("vault_generate_bip39_mnemonic", { wordCount: createWalletWordCount });
            const mnemonic = gen.mnemonic;
            createWalletMnemonic = mnemonic;
            const walletName = createWalletName || "hemp0x-vault-main";
            const result = await withTimeout(
                core.invoke("vault_restore_from_recovery_phrase", {
                    mnemonic,
                    walletName,
                    vaultPassphrase: createWalletPassphrase,
                    birthHeight: null,
                }),
                120000,
                "Create vault wallet",
            );
            createWalletDone = true;
            createWalletPassphrase = "";
            createWalletName = "";
            vaultUnlocked = true;
            vaultStatus.set({ exists: true, unlocked: true });
            // Bridge the render with a synthetic exists:true overview so
            // the page flips to the unlocked vault view immediately,
            // before refreshWalletHeader() returns the real on-disk state.
            if (!hasVault) {
                vaultOverview = {
                    exists: true,
                    vault_path: vaultOverview?.vault_path ?? "",
                    file_size: 0,
                    file_modified: 0,
                    display_label: "",
                };
                vaultOverviewError = "";
            }
            vaultWalletRecords = [];
            vaultWalletRecordsLoaded = false;
            vaultWalletRecordsMsg = "";
            // Refresh vault/wallet state first so the page reflects the
            // new vault, then load the wallet into Core through the
            // existing flow with the wallet-switch loader visible.
            await refreshWalletHeader();
            loadVaultWalletRecords().catch(() => {});
            loadAlignmentStatus().catch(() => {});
            let coreLoadError = "";
            if (result?.core_wallet_name) {
                try {
                    await core.invoke("vault_set_active_wallet_name", { walletName: result.core_wallet_name });
                } catch (_) {}
                walletSwitchWorking = true;
                walletSwitchTargetName = result.core_wallet_name;
                try {
                    await ensureNamedCoreWalletLoaded(result.core_wallet_name, "Created wallet");
                } catch (loadErr) {
                    coreLoadError = String(loadErr);
                } finally {
                    walletSwitchWorking = false;
                    walletSwitchTargetName = "";
                }
                await refreshWalletHeader().catch(() => {});
                loadAlignmentStatus().catch(() => {});
            }
            await autosaveActiveVaultExport("New wallet was stored in your vault");
            if (coreLoadError) {
                vaultPageError = coreLoadError;
                showToast(coreLoadError, "warning", false);
            }
        } catch (err) {
            createWalletError = String(err);
        }
        createWalletWorking = false;
    }

    function closeCreateWalletModal() {
        showCreateWalletModal = false;
        createWalletPassphrase = "";
        createWalletName = "";
        createWalletWorking = false;
        createWalletError = "";
        createWalletDone = false;
        createWalletMnemonic = "";
        createWalletWordCount = 12;
        createWalletPhraseConfirmed = false;
    }

    async function doConfirmImportSwitch() {
        const path = switchImportPendingPath;
        const archive = switchImportPendingArchive;
        switchImportArchiveArmed = false;
        switchImportPendingPath = "";
        switchImportPendingArchive = true;
        await doImportVault(path, archive);
    }

    async function executeVaultBackup() {
        vaultExportLabel = vaultExportLabel || "Vault Backup " + new Date().toISOString().slice(0, 10);
        await executeVaultExport();
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
        if (vaultWalletRecordsLoading) return;
        vaultWalletRecordsLoading = true;
        vaultWalletRecordsError = "";
        try {
            const explicit = vaultListPassphrase && !vaultUnlocked ? vaultListPassphrase : null;
            vaultWalletRecords = await withTimeout(
                core.invoke("ipfs_vault_list_wallet_migration_records", { vaultPassphrase: explicit }),
                10000,
                "Vault backup list",
            );
        } catch (err) {
            vaultWalletRecordsError = String(err);
            vaultWalletRecords = [];
        } finally {
            vaultWalletRecordsLoading = false;
            vaultWalletRecordsLoaded = true;
        }
    }

    async function vaultImportMigrationFile() {
        vaultWalletRecordsError = "";
        vaultWalletRecordsMsg = "";
        try {
            const selected = await open({
                title: "Select Migration Envelope File",
                multiple: false,
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
            await loadVaultWalletRecords();
            await autosaveActiveVaultExport("Core migration snapshot added to your vault");
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
            vaultWalletRecordsLoaded = false;
            await loadVaultWalletRecords();
            await autosaveActiveVaultExport("Core wallet snapshot added to your vault");
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
        if (!vaultRestoreConfirmChecked) {
            vaultWalletRecordsError = "Confirm you understand this will create a new Core wallet.";
            return;
        }
        if (!vaultRestoreRecordId || !vaultRestoreWalletName) {
            vaultWalletRecordsError = "Select a backup and set a wallet name.";
            return;
        }
        if (vaultRestoreWalletNameError) {
            vaultWalletRecordsError = vaultRestoreWalletNameError;
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
            vaultWalletRecordsMsg = `Wallet restored: ${result.wallet_name}. Commander will help you load it.`;
            vaultRestoreRecordId = "";
            vaultRestoreWalletName = "";
            vaultRestorePassphrase = "";
            vaultRestoreVaultPassphrase = "";
            vaultRestoreBirth = "";
            vaultRestoreConfirmChecked = false;
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
            await loadVaultWalletRecords();
            await autosaveActiveVaultExport("Core wallet snapshot removed from your vault");
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
            });
            if (selected) {
                migrationRestorePath = selected;
            }
        } catch (e) { /* user cancelled */ }
    }

    async function migrateRestore() {
        if (migrationWorking) return;
        if (!migrationRestoreConfirmChecked) {
            showToast("Confirm you understand this will create a new Core wallet.", "warning");
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
        if (migrationRestoreNameError) {
            migrationError = migrationRestoreNameError;
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
        migrationRestoreConfirmChecked = false;
    }

</script>

<!-- ================= STATE-DRIVEN WALLET PAGE (64i rework) ================= -->
{#if !tauriReady || (vaultOverview === null && vaultOverviewError === null)}
    <div class="glass-panel panel-soft" style="margin:1rem 0; padding:1.5rem; text-align:center;">
        <p style="color:#888; font-size:0.8rem;">Loading wallet status…</p>
    </div>
{:else if vaultOverviewError && vaultOverview === null}
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
                <h2 style="color:var(--color-primary); font-size:1rem; margin:0 0 0.4rem; letter-spacing:1px;">HEMP0X VAULT</h2>
                <p style="color:#888; font-size:0.75rem; max-width:480px; margin:0 auto 0.5rem; line-height:1.5;">
                    Your portable encrypted wallet container. Create one, import from WebCom, or restore from a recovery phrase.
                </p>
            </div>

            <div style="padding:0 1.25rem 1rem; display:flex; flex-direction:column; gap:0.6rem; max-width:480px; margin:0 auto;">
                <!-- Create New Hemp0x Vault -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">CREATE NEW HEMP0X VAULT</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Create a new encrypted vault and generate your first portable wallet.
                    </p>
                    <button class="cyber-btn small wide" on:click={promptCreateNewVaultFromLocked}>CREATE NEW VAULT</button>
                </div>

                <!-- Import Hemp0x Vault -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">IMPORT HEMP0X VAULT</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Import a vault file from WebCom or another Commander install.
                    </p>
                    <button class="cyber-btn ghost small wide" on:click={importVaultBundle}>IMPORT VAULT FROM FILE</button>
                </div>

                <!-- Import Wallet File -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">IMPORT WALLET FILE</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Import a legacy wallet.dat or a Core migration envelope. Commander detects the file type and guides you.
                    </p>
                    <button class="cyber-btn ghost small wide" on:click={unifiedImportWalletFile}>IMPORT WALLET FILE</button>
                </div>

                <!-- Restore From Recovery Phrase -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">RESTORE FROM RECOVERY PHRASE</h4>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.7rem;">
                        Enter your 12, 18, or 24-word recovery phrase. Commander will create a vault and store the restored wallet in it.
                    </p>
                    <button class="cyber-btn ghost small wide" on:click={() => { showRecoveryPhraseModal = true; }}>ENTER RECOVERY PHRASE</button>
                </div>

                <!-- Advanced Recovery (collapsed) -->
                <button class="toggle" style="background:none; border:none; color:#666; font-size:0.65rem; padding:0.25rem 0; cursor:pointer; text-align:center; letter-spacing:0.5px;" on:click={() => (showAdvancedRecovery = !showAdvancedRecovery)}>
                    {showAdvancedRecovery ? "▲" : "▼"} ADVANCED RECOVERY
                </button>
                {#if showAdvancedRecovery}
                    <div style="display:flex; flex-direction:column; gap:0.4rem;">
                        <div class="btn-row" style="gap:0.4rem;">
                            <button class="cyber-btn ghost tiny wide" on:click={openExportModal}>EXPORT KEYS</button>
                            <button class="cyber-btn ghost tiny wide" on:click={openImportModal}>IMPORT KEYS</button>
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    </div>
{:else if !vaultUnlocked}
    <!-- 2. VAULT LOCKED -->
    <div in:fly={{ y: 12, duration: 250 }}>
        <div class="glass-panel panel-soft" style="margin:0.5rem 0; padding:0;">
            {#if vaultPageError}
                <div style="padding:0.65rem 1rem; background:rgba(255,85,85,0.1); border-bottom:1px solid rgba(255,85,85,0.25);">
                    <div style="display:flex; align-items:flex-start; gap:0.5rem;">
                        <div style="flex:1;">
                            <p style="color:#ff7777; font-size:0.7rem; margin:0; line-height:1.45; word-break:break-word;">{vaultPageError}</p>
                        </div>
                        <button class="cyber-btn ghost tiny" style="white-space:nowrap;" on:click={() => { vaultPageError = ""; }}>DISMISS</button>
                    </div>
                </div>
            {/if}
            <div style="padding:1.25rem 1.5rem 0.75rem; text-align:center;">
                <h2 style="color:var(--color-primary); font-size:1rem; margin:0 0 0.4rem; letter-spacing:1px;">HEMP0X VAULT LOCKED</h2>
                <p style="color:#888; font-size:0.75rem; max-width:480px; margin:0 auto 0.5rem; line-height:1.5;">
                    Unlock your Hemp0x Vault to access your portable wallet.
                </p>
            </div>

            <div style="padding:0 1.25rem 1rem; display:flex; flex-direction:column; gap:0.6rem; max-width:480px; margin:0 auto;">
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.85rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">UNLOCK VAULT</h4>
                    <input type="password" class="input-glass" placeholder="Enter your passphrase" bind:value={unlockVaultPassphrase} on:keydown={(e) => e.key === 'Enter' && !unlockVaultWorking && executeUnlockVault()} style="font-size:0.8rem; padding:0.55rem;" />
                    <button class="cyber-btn small wide" on:click={executeUnlockVault} disabled={unlockVaultWorking || !unlockVaultPassphrase}>
                        {unlockVaultWorking ? "UNLOCKING…" : "UNLOCK"}
                    </button>
                    {#if unlockVaultError}
                        <div style="margin-top:0.45rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
                            <p style="color:#ff7777; font-size:0.72rem; margin:0; font-weight:700;">{unlockVaultError}</p>
                            {#if unlockVaultErrorDetails}
                                <p style="color:#bbb; font-size:0.66rem; margin:0.25rem 0 0; line-height:1.45; word-break:break-word;">{unlockVaultErrorDetails}</p>
                            {/if}
                        </div>
                    {/if}
                </div>

                <div style="margin-top:1.25rem; display:flex; justify-content:center; gap:0.4rem; flex-wrap:wrap;">
                    <button class="cyber-btn ghost tiny" on:click={importVaultBundle}>SWITCH / IMPORT</button>
                    <button class="cyber-btn ghost tiny" on:click={promptCreateNewVaultFromLocked}>CREATE NEW</button>
                    <button class="cyber-btn ghost tiny" on:click={promptArchiveVault}>SAVE HEMP0X VAULT</button>
                    {#if hasVault}
                        <button class="cyber-btn ghost tiny" on:click={openUnloadVaultModal}>UNLOAD VAULT</button>
                    {/if}
                </div>
                {#if showAdvancedRecovery}
                    <div style="max-width:340px; margin:0.5rem auto 0; display:flex; flex-direction:column; gap:0.35rem;">
                        <button class="cyber-btn ghost tiny wide" on:click={unifiedImportWalletFile}>IMPORT WALLET FILE</button>
                        <div class="btn-row" style="gap:0.35rem;">
                            <button class="cyber-btn ghost tiny wide" on:click={openExportModal}>EXPORT KEYS</button>
                            <button class="cyber-btn ghost tiny wide" on:click={openImportModal}>IMPORT KEYS</button>
                        </div>
                    </div>
                {/if}
                <div style="margin-top:0.6rem;">
                    <button class="toggle" style="background:none; border:none; color:#666; font-size:0.65rem; padding:0; cursor:pointer; letter-spacing:0.5px;" on:click={() => (showAdvancedRecovery = !showAdvancedRecovery)}>
                        {showAdvancedRecovery ? "▲" : "▼"} ADVANCED RECOVERY
                    </button>
                </div>

                <!-- Active File card -->
                <div style="max-width:480px; margin:1.25rem auto 0; text-align:left; background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.7rem 0.85rem;">
                    <div style="display:flex; align-items:center; justify-content:space-between; gap:0.5rem; margin-bottom:0.3rem;">
                        <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem; letter-spacing:0.5px;">ACTIVE FILE</h4>
                        <button class="toggle" style="background:none; border:none; color:#888; font-size:0.6rem; padding:0; cursor:pointer;" on:click={() => (showVaultManagerPanel = !showVaultManagerPanel)}>
                            {showVaultManagerPanel ? "▲" : "▼"} DETAILS
                        </button>
                    </div>
                    <div style="display:flex; align-items:center; gap:0.4rem; flex-wrap:wrap; font-size:0.7rem; color:#aaa;">
                        <span style="color:#888;">Loaded:</span>
                        <span title={vaultOverview?.vault_path ?? ""} style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:200px;">{vaultBasenameFromPath(vaultOverview?.vault_path)}</span>
                        {#if vaultOverview?.display_label}
                            <span style="color:var(--color-primary);">"{vaultOverview.display_label}"</span>
                        {/if}
                        <span style="color:#666;">•</span>
                        <span>{formatUnixShort(vaultOverview?.file_modified || vaultOverview?.modified)}</span>
                    </div>
                    {#if showVaultManagerPanel}
                        <div style="margin-top:0.5rem; border-top:1px dashed rgba(255,255,255,0.06); padding-top:0.5rem; display:flex; flex-direction:column; gap:0.4rem;">
                            <div style="display:grid; grid-template-columns:auto 1fr; gap:0.2rem 0.5rem; font-size:0.65rem; color:#aaa;">
                                {#if vaultOverview?.network}<span style="color:#888;">Network</span><span>{vaultOverview.network} • v{vaultOverview.bundle_version}</span>{/if}
                                <span style="color:#888;">Size</span><span>{formatBytes(vaultOverview?.file_size)}</span>
                            </div>
                            <div>
                                <label for="vault-label-locked" style="font-size:0.6rem; color:#888;">DISPLAY NAME</label>
                                <div style="display:flex; gap:0.35rem; margin-top:0.15rem;">
                                    <input id="vault-label-locked" type="text" class="input-glass" placeholder="e.g. Main Backup" bind:value={vaultLabelInput} style="font-size:0.65rem; padding:0.35rem; flex:1;" />
                                    <button class="cyber-btn tiny" on:click={saveVaultLabel} disabled={vaultLabelSaving || (vaultLabelInput === (vaultOverview?.display_label ?? ""))}>
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

                <!-- Help panel -->
                <div style="max-width:480px; margin:0.75rem auto 0; text-align:left; background:rgba(0,0,0,0.15); border:1px solid rgba(255,255,255,0.04); border-radius:5px; padding:0.65rem 0.85rem;">
                    <div style="display:flex; align-items:center; gap:0.35rem; margin-bottom:0.3rem;">
                        <span style="font-size:0.8rem; opacity:0.7;">*</span>
                        <h4 style="color:var(--color-primary); margin:0; font-size:0.65rem; letter-spacing:0.5px;">ABOUT YOUR HEMP0X VAULT</h4>
                    </div>
                    <div style="font-size:0.65rem; color:#aaa; line-height:1.55; display:flex; flex-direction:column; gap:0.3rem;">
                        <p style="margin:0;">Your Hemp0x Vault is your portable encrypted wallet container. It holds wallet recovery data and optional app credentials.</p>
                        <p style="margin:0;"><strong style="color:#ffaa00;">Important:</strong> Your passphrase protects everything inside. If you lose it, your encrypted data cannot be recovered.</p>
                        <p style="margin:0; color:#888;">Use <strong>Save Hemp0x Vault</strong> to create a dated backup of the vault file before switching or creating a new one.</p>
                    </div>
                </div>
            </div>
        </div>
    </div>
{:else}
    <!-- 3. UNLOCKED — Single-page state-driven layout -->
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
                            {#if alignmentStatus?.connection_state === "verified_aligned"}
                                <span style="color:var(--color-primary);">Verified ✓</span>
                                <span style="color:#666;">|</span>
                            {:else if alignmentStatus?.wallet_record_state === "webcom_primary_detected"}
                                <span style="color:#ffaa00;">Not connected</span>
                                <span style="color:#666;">|</span>
                            {:else if alignmentStatus && alignmentStatus.wallet_record_state}
                                <span style="color:#888;">Not from this vault</span>
                                <span style="color:#666;">|</span>
                            {/if}
                            {#if isRuntimeWalletEncrypted(walletStatus)}
                                {#if isRuntimeWalletUnlocked(walletStatus)}
                                    <span style="color:var(--color-primary);">Unlocked</span>
                                {:else}
                                    <span style="color:#ffaa00;">Locked</span>
                                {/if}
                            {:else}
                                <span style="color:#ffaa00;">Unencrypted</span>
                            {/if}
                        </span>
                    {:else}
                        <span style="font-size:0.7rem; color:#666;">Loading…</span>
                    {/if}
                    <span style="font-size:0.65rem; color:#666;">•</span>
                    <span style="font-size:0.7rem; color:var(--color-primary);">Vault Unlocked</span>
                </div>
                <div style="display:flex; gap:0.35rem; margin-left:auto;">
                    <button class="cyber-btn ghost small" on:click={executeLockVault}>LOCK</button>
                    <button class="cyber-btn ghost small" on:click={refreshWalletHeader}>REFRESH</button>
                </div>
            </header>

            <!-- Status banners -->
            {#if vaultPageError}
                <div style="padding:0.65rem 1rem; background:rgba(255,85,85,0.1); border-top:1px solid rgba(255,85,85,0.25); border-bottom:1px solid rgba(255,85,85,0.1);">
                    <div style="display:flex; align-items:flex-start; gap:0.5rem;">
                        <div style="flex:1;">
                            <p style="color:#ff7777; font-size:0.7rem; margin:0; line-height:1.45; word-break:break-word;">{vaultPageError}</p>
                        </div>
                        <button class="cyber-btn ghost tiny" style="white-space:nowrap;" on:click={() => { vaultPageError = ""; }}>
                            DISMISS
                        </button>
                    </div>
                </div>
            {/if}
            {#if walletStatusError}
                <div style="padding:0.5rem 1rem; background:rgba(255,85,85,0.08); border-top:1px solid rgba(255,85,85,0.2);">
                    <p style="color:#ff5555; font-size:0.7rem; margin:0;">{walletStatusError}</p>
                </div>
            {:else if vaultWalletRecordsError}
                <div style="padding:0.5rem 1rem; background:rgba(255,85,85,0.08); border-top:1px solid rgba(255,85,85,0.2);">
                    <div style="display:flex; align-items:flex-start; gap:0.5rem;">
                        <p style="color:#ff7777; font-size:0.7rem; margin:0; line-height:1.45; word-break:break-word; flex:1;">{vaultWalletRecordsError}</p>
                        <button class="cyber-btn ghost tiny" style="white-space:nowrap;" on:click={() => { vaultWalletRecordsError = ""; }}>
                            DISMISS
                        </button>
                    </div>
                </div>
            {/if}

            <!-- Vault file bar -->
            <div style="padding:0.5rem 1rem; border-top:1px solid rgba(255,255,255,0.05); display:flex; align-items:center; justify-content:space-between; gap:0.5rem; flex-wrap:wrap;">
                <div style="display:flex; align-items:center; gap:0.4rem; font-size:0.65rem; color:#888;">
                    <span title={vaultOverview?.vault_path ?? ""} style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:200px;">{vaultBasenameFromPath(vaultOverview?.vault_path)}</span>
                    {#if vaultOverview?.display_label}
                        <span style="color:var(--color-primary);">"{vaultOverview.display_label}"</span>
                    {/if}
                </div>
                <div style="display:flex; gap:0.35rem; flex-wrap:wrap;">
                    <button class="cyber-btn ghost tiny" on:click={switchOrImportVault}>SWITCH / IMPORT</button>
                    <button class="cyber-btn ghost tiny" on:click={promptCreateNewVaultFromLocked}>NEW</button>
                    <button class="cyber-btn ghost tiny" on:click={promptArchiveVault}>SAVE HEMP0X VAULT</button>
                </div>
            </div>
        </div>

        <!-- MAIN CONTENT: Single-page state-driven layout -->
        <div class="glass-panel panel-soft" in:fly={{ y: 8, duration: 200 }} style="margin:0.5rem 0; padding:0;">
            <div style="padding:1.25rem; display:flex; flex-direction:column; gap:1rem;">

                <!-- SECTION 1: Hemp0x Vault Wallet Status -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                    <div style="display:flex; align-items:center; gap:0.35rem; margin-bottom:0.4rem;">
                        <span style="font-size:0.8rem; opacity:0.7;">*</span>
                        <h4 style="color:var(--color-primary); margin:0; font-size:0.75rem; letter-spacing:0.5px;">HEMP0X VAULT WALLET</h4>
                    </div>
                    {#if alignmentStatusLoading}
                        <p style="color:#666; font-size:0.7rem; margin:0;">Checking vault wallet status...</p>
                    {:else if alignmentStatusError}
                        <p style="color:#ff5555; font-size:0.7rem; margin:0;">{alignmentStatusError}</p>
                    {:else if alignmentStatus}
	                        {#if !activeVaultWalletName && !isVaultWalletActive(alignedVaultWalletName())}
	                            <p style="color:#888; font-size:0.7rem; margin:0 0 0.25rem;">Hemp0x Vault wallet: Not connected</p>
	                            <p style="color:#666; font-size:0.6rem; margin:0 0 0.5rem;">Commander is using a legacy wallet.dat. Use the vault wallet below to switch Core back to portable Hemp0x Vault mode.</p>
                        {:else if alignmentStatus.wallet_record_state === "webcom_primary_detected"}
                            <p style="color:var(--color-primary); font-size:0.7rem; margin:0 0 0.25rem;">
                                Vault contains a portable wallet
                                {#if alignmentStatus.webcom_primary_seed_type}
                                    <span style="color:#888; font-size:0.6rem;">({alignmentStatus.webcom_primary_seed_type})</span>
                                {/if}
                            </p>
                            {#if alignmentStatus.connection_state === "verified_aligned" && isVaultWalletActive(alignedVaultWalletName())}
                                <p style="color:var(--color-primary); font-size:0.65rem; margin:0 0 0.25rem;">Commander wallet verified and loaded for this vault</p>
                            {:else if alignmentStatus.connection_state === "verified_aligned"}
                                <p style="color:#ffaa00; font-size:0.65rem; margin:0 0 0.25rem;">Commander wallet is verified for this vault but not loaded</p>
                                <p style="color:#666; font-size:0.6rem; margin:0 0 0.5rem;">Core is currently using {activeRuntimeWalletLabel()}.</p>
                            {:else if alignmentStatus.connection_state === "stale_unverified_alignment"}
                                <p style="color:#ffaa00; font-size:0.65rem; margin:0 0 0.25rem;">Commander wallet is not connected to this vault</p>
                            {:else}
                                <p style="color:#ffaa00; font-size:0.65rem; margin:0 0 0.25rem;">Commander is not connected to this vault wallet</p>
                            {/if}
                        {:else if alignmentStatus.wallet_record_state === "unsupported"}
                            <p style="color:#888; font-size:0.7rem; margin:0 0 0.25rem;">Vault wallet record is not a supported BIP39 wallet</p>
                        {:else}
                            <p style="color:#888; font-size:0.7rem; margin:0 0 0.25rem;">No portable wallet found in this vault</p>
                        {/if}

                        {#if !alignmentStatus.core_reachable}
                            <p style="color:#888; font-size:0.65rem; margin:0 0 0.5rem;">Core daemon status unavailable</p>
                        {:else if alignmentStatus.wallet_record_state !== "webcom_primary_detected"}
                            <p style="color:#aaa; font-size:0.65rem; margin:0 0 0.5rem;">Import a vault from WebCom or restore from a recovery phrase to get started.</p>
                        {:else if alignmentStatus.connection_state === "verified_aligned"}
                            <!-- Already aligned, quiet -->
                        {:else}
                            <p style="color:var(--color-primary); font-size:0.65rem; margin:0 0 0.5rem;">Ready to connect vault wallet</p>
                        {/if}

                        {#if alignmentStatus.wallet_record_state === "webcom_primary_detected"}
                            <div style="display:flex; gap:0.4rem; flex-wrap:wrap;">
                                {#if alignmentStatus.connection_state === "verified_aligned" && !isVaultWalletActive(alignedVaultWalletName())}
                                    <button class="cyber-btn primary-glow tiny" on:click={() => useVaultWallet(alignedVaultWalletName())} disabled={walletSwitchWorking}>
                                        {walletSwitchWorking ? "SWITCHING..." : "USE THIS VAULT WALLET"}
                                    </button>
                                {:else if alignmentStatus.connection_state !== "verified_aligned" && alignmentStatus.can_guided_connect}
                                    <button class="cyber-btn primary-glow small" on:click={async () => { await loadAlignmentConnectPlan(); showAlignmentReviewModal = true; }}>
                                        CONNECT VAULT WALLET
                                    </button>
                                {/if}
                                <button class="cyber-btn ghost tiny" on:click={() => { showAlignmentDetails = !showAlignmentDetails; }}>
                                    {showAlignmentDetails ? "▲" : "▼"} DETAILS
                                </button>
                            </div>
                        {:else}
                            <div style="display:flex; gap:0.45rem; flex-wrap:wrap;">
                                {#if walletStatus?.walletname || walletDatExists}
                                    <button class="cyber-btn primary-glow small" on:click={importCurrentRuntimeWalletToVault}>
                                        ADD CURRENT RUNTIME WALLET
                                    </button>
                                {/if}
                                <button class="cyber-btn primary-glow small" on:click={() => {
                                    showCreateWalletModal = true;
                                    createWalletPassphrase = "";
                                    createWalletName = "";
                                    createWalletWorking = false;
                                    createWalletError = "";
                                    createWalletDone = false;
                                    createWalletMnemonic = "";
                                    createWalletWordCount = 12;
                                    createWalletPhraseConfirmed = false;
                                }}>CREATE NEW WALLET</button>
                                <button class="cyber-btn ghost small" on:click={() => { showRecoveryPhraseModal = true; }}>RESTORE FROM RECOVERY PHRASE</button>
                                <button class="cyber-btn ghost small" on:click={unifiedImportWalletFile}>IMPORT WALLET FILE</button>
                            </div>
                        {/if}

                        {#if showAlignmentDetails && alignmentStatus.wallet_record_state === "webcom_primary_detected"}
                            <div style="margin-top:0.6rem; border-top:1px dashed rgba(255,255,255,0.06); padding-top:0.5rem; display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#888;">
                                {#if alignmentStatus.webcom_primary_derivation_hemp}<div><span style="color:#666;">Derivation:</span> {alignmentStatus.webcom_primary_derivation_hemp}</div>{/if}
                                {#if alignmentStatus.webcom_primary_network}<div><span style="color:#666;">Network:</span> {alignmentStatus.webcom_primary_network}</div>{/if}
                                {#if alignmentStatus.connection_state}<div><span style="color:#666;">Connection:</span> {alignmentStatus.connection_state}</div>{/if}
                                {#if alignmentStatus.verification_status}<div><span style="color:#666;">Verification:</span> {alignmentStatus.verification_status}</div>{/if}
                            </div>
                        {/if}
                    {/if}
                </div>

                <!-- SECTION 2: Runtime Wallet Status -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.75rem; letter-spacing:0.5px;">RUNTIME WALLET</h4>
                    {#if !activeVaultWalletName}
                        <div style="background:rgba(255,170,0,0.08); border:1px solid rgba(255,170,0,0.2); border-radius:4px; padding:0.35rem 0.6rem; margin-bottom:0.5rem; font-size:0.6rem; color:#ffaa00;">
                            <strong>Mode: Legacy Core wallet</strong> — Commander is using the default wallet.dat, not a connected vault wallet.
                        </div>
                    {/if}
                    {#if walletStatusError}
                        <p style="color:#ffaa00; font-size:0.7rem; margin:0;">Unavailable</p>
                    {:else if walletStatus}
                        <div style="display:flex; flex-direction:column; gap:0.3rem; font-size:0.7rem; color:#aaa;">
                            <div><span style="color:#888;">Name:</span> <span>{walletStatus.walletname ?? "default"}</span>{#if !activeVaultWalletName && (walletStatus.walletname === "default" || !walletStatus.walletname)}<span style="color:#888; font-size:0.6rem;"> (wallet.dat)</span>{/if}</div>
                            <div>
                                <span style="color:#888;">Status:</span>
                                {#if isRuntimeWalletEncrypted(walletStatus)}
                                    {#if isRuntimeWalletUnlocked(walletStatus)}
                                        <span style="color:var(--color-primary);">Encrypted & Unlocked</span>
                                    {:else}
                                        <span style="color:#ffaa00;">Encrypted & Locked</span>
                                    {/if}
                                {:else}
                                    <span style="color:#ffaa00;">Unencrypted</span>
                                {/if}
                            </div>
                            {#if walletStatus.balance !== undefined && walletStatus.balance !== null}
                                <div><span style="color:#888;">Balance:</span> <strong style="color:var(--color-primary);">{walletStatus.balance}</strong> HEMP</div>
                            {/if}
                            {#if walletStatus.txcount !== undefined}
                                <div><span style="color:#888;">Transactions:</span> <span>{walletStatus.txcount}</span></div>
                            {/if}
                        </div>
                    {:else}
                        <p style="color:#666; font-size:0.7rem; margin:0;">Loading…</p>
                    {/if}

                    <div class="laser-divider" style="margin:0.6rem 0;"></div>

                    <div style="display:flex; gap:0.4rem; flex-wrap:wrap;">
                        {#if walletStatus && !isRuntimeWalletEncrypted(walletStatus)}
                            <button class="cyber-btn primary-glow small" on:click={() => { showEncryptModal = true; }}>ENCRYPT WALLET</button>
                        {/if}
                        {#if walletStatus && isRuntimeWalletEncrypted(walletStatus) && isRuntimeWalletUnlocked(walletStatus)}
                            <button class="cyber-btn ghost tiny" on:click={async () => {
                                try {
                                    const runtimeWalletName = loadedRuntimeWalletName();
                                    if (isDefaultRuntimeWalletName(runtimeWalletName)) {
                                        await core.invoke("wallet_lock");
                                    } else {
                                        await core.invoke("wallet_lock_named", { walletName: runtimeWalletName });
                                    }
                                    showToast("Wallet locked.", "info");
                                    await refreshWalletHeader();
                                }
                                catch (e) { showToast("Lock failed: " + e, "error"); }
                            }}>LOCK WALLET</button>
                        {/if}
                        {#if walletStatus && isRuntimeWalletEncrypted(walletStatus) && !isRuntimeWalletUnlocked(walletStatus)}
                            <button class="cyber-btn ghost tiny" on:click={() => { unlockModalPurpose = "wallet_unlock"; showUnlockModal = true; }}>UNLOCK WALLET</button>
                        {/if}
                        <button class="cyber-btn ghost tiny" on:click={async () => {
                            historyRecoverWorking = true;
                            vaultPageError = "";
                            const walletName = alignmentStatus?.core_wallet_name || walletStatus?.walletname || "default";
                            try {
                                await core.invoke("vault_start_wallet_history_recovery", { walletName, fromBlock: null });
                                historyRecoveryBackgroundActive = true;
                                coreBusyUntil.set(Date.now() + 120000);
                                showToast("History recovery started. It will continue in the background.", "info", false);
                                window.setTimeout(() => {
                                    historyRecoveryBackgroundActive = false;
                                }, 120000);
                            } catch (e) {
                                vaultPageError = "History recovery could not start: " + String(e);
                                historyRecoveryBackgroundActive = false;
                            }
                            window.setTimeout(() => {
                                historyRecoverWorking = false;
                            }, 800);
                        }} disabled={historyRecoverWorking}>
                            {historyRecoverWorking ? "RECOVERING…" : "RECOVER HISTORY"}
                        </button>
                        {#if alignmentStatus?.connection_state === "verified_aligned" && activeVaultWalletName}
                            <button class="cyber-btn ghost tiny" on:click={() => useVaultWallet(alignedVaultWalletName())} disabled={walletSwitchWorking}>
                                {walletSwitchWorking ? "SWITCHING…" : "RELOAD VAULT WALLET"}
                            </button>
                        {/if}
                    </div>
                </div>

                <!-- SECTION 3: Save / safety reminder -->
                <div style="background:rgba(255,170,0,0.08); border:1px solid rgba(255,170,0,0.22); border-radius:6px; padding:0.6rem 0.9rem; font-size:0.65rem; color:#aaa; line-height:1.5; display:flex; align-items:center; gap:0.55rem;">
                    <span aria-hidden="true" style="color:#ffaa00; font-size:0.85rem; line-height:1;">&#9888;</span>
                    <span>
                        <button
                            type="button"
                            class="text-btn"
                            style="color:#ffaa00; font-size:0.65rem; font-weight:700; text-decoration:none;"
                            on:click={promptArchiveVault}
                        >
                            Save
                        </button>
                        your Hemp0x Vault file after important changes.
                    </span>
                </div>

                <!-- SECTION 3b: Vault security (advanced) — passphrase rotation + unload/fallback (64p) -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                    <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.75rem; letter-spacing:0.5px;">VAULT / WALLET SECURITY</h4>
                    <p class="desc" style="margin:0 0 0.55rem; font-size:0.65rem;">
                        Advanced security operations. Rotate the vault passphrase, change the loaded Core runtime wallet password, switch Core to wallet.dat while keeping this vault open, or unload this vault from Commander.
                    </p>
                    <div style="display:flex; gap:0.4rem; flex-wrap:wrap;">
                        <button class="cyber-btn ghost small" on:click={openChangeVaultPassphraseModal} disabled={!vaultUnlocked || !hasVault}>
                            CHANGE VAULT PASSPHRASE
                        </button>
                        <button class="cyber-btn ghost small" on:click={openChangeRuntimeWalletPassModal} disabled={!walletStatus || !isRuntimeWalletEncrypted(walletStatus)}>
                            CHANGE WALLET PASSWORD
                        </button>
                        <button class="cyber-btn ghost small" on:click={useWalletDatRuntime} disabled={!hasVault || walletSwitchWorking}>
                            {walletSwitchWorking && walletSwitchTargetName === "wallet.dat" ? "SWITCHING…" : "USE WALLET.DAT"}
                        </button>
                        <button class="cyber-btn ghost small" on:click={openUnloadVaultModal} disabled={!hasVault}>
                            UNLOAD VAULT
                        </button>
                    </div>
                </div>

                <!-- SECTION 5: Advanced recovery tools (collapsed) -->
                <button class="toggle" style="background:none; border:none; color:#666; font-size:0.65rem; padding:0.25rem 0; cursor:pointer; text-align:center; letter-spacing:0.5px;" on:click={() => (showAdvancedRecovery = !showAdvancedRecovery)}>
                    {showAdvancedRecovery ? "▲" : "▼"} ADVANCED RECOVERY TOOLS
                </button>
                {#if showAdvancedRecovery}
                    <div style="display:flex; flex-direction:column; gap:0.6rem;">
                        <!-- Core wallet backup records (migration envelopes) -->
                        <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                            <h4 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem; letter-spacing:0.5px;">CORE WALLET BACKUP RECORDS</h4>
                            <p class="desc" style="margin:0 0 0.6rem; font-size:0.65rem;">
                                Optional Commander/Core recovery snapshots stored inside this vault. Your portable Hemp0x Vault wallet above remains the primary cross-platform wallet; WebCom keeps these records but does not use them for normal wallet loading.
                            </p>
                            {#if vaultWalletRecordsMsg}
                                <div style="margin-bottom:0.4rem; border:1px solid rgba(0,255,65,0.2); background:rgba(0,255,65,0.05); border-radius:4px; padding:0.4rem 0.6rem;">
                                    <p style="color:var(--color-primary); font-size:0.65rem; margin:0;">{vaultWalletRecordsMsg}</p>
                                </div>
                            {/if}
                            {#if hasBackups}
                                <div style="display:flex; flex-direction:column; gap:0.3rem; font-size:0.7rem; color:#aaa; margin-bottom:0.5rem;">
                                    <div><span style="color:#888;">Count:</span> <span>{vaultWalletRecords.length} record{vaultWalletRecords.length === 1 ? "" : "s"}</span></div>
                                    <div>
                                        <span style="color:#888;">Latest:</span>
                                        <span style="color:var(--color-primary);">{vaultWalletRecords[0]?.label}</span>
                                        {#if vaultWalletRecords[0]?.modified}
                                            <span style="color:#666; font-size:0.65rem;"> • {new Date(vaultWalletRecords[0].modified * 1000).toLocaleString()}</span>
                                        {/if}
                                    </div>
                                </div>
                            {:else}
                                <p style="color:#888; font-size:0.65rem; margin:0 0 0.5rem;">No Core wallet backup records yet.</p>
                            {/if}

                            <div style="display:flex; gap:0.4rem; flex-wrap:wrap; margin-bottom:0.5rem;">
                                <button class="cyber-btn ghost small" on:click={executeVaultBackup}>SAVE CORE WALLET SNAPSHOT</button>
                                <button class="cyber-btn ghost small" on:click={vaultImportMigrationFile}>
                                    {vaultImportPath ? vaultImportPath.split('/').pop().split('\\').pop() : "IMPORT CORE MIGRATION FILE"}
                                </button>
                                {#if hasBackups}
                                    <button class="cyber-btn ghost small" on:click={async () => { await loadVaultWalletRecords(); showVaultRestorePanel = true; }}>RESTORE FROM BACKUP</button>
                                {/if}
                            </div>

                            {#if vaultImportPath}
                                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem; margin-bottom:0.6rem;">
                                    <h5 style="color:var(--color-primary); margin:0 0 0.45rem; font-size:0.7rem;">IMPORT CORE MIGRATION INTO VAULT</h5>
                                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                                        Stores a Core Next migration envelope as a recovery record inside this Hemp0x Vault. This does not replace the primary portable vault wallet.
                                    </p>
                                    <input type="text" class="input-glass" placeholder="Record label (e.g. Old wallet backup)" bind:value={vaultImportLabel} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                                    <input type="password" class="input-glass" placeholder="Migration file passphrase, if encrypted" bind:value={vaultImportMigrationPassphrase} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.5rem; width:100%; box-sizing:border-box;" />
                                    <div style="display:flex; gap:0.4rem; flex-wrap:wrap;">
                                        <button class="cyber-btn primary-glow tiny" on:click={executeVaultImport} disabled={vaultImportWorking || !vaultImportLabel}>
                                            {vaultImportWorking ? "IMPORTING…" : "IMPORT INTO VAULT"}
                                        </button>
                                        <button class="cyber-btn ghost tiny" on:click={() => { vaultImportPath = ""; vaultImportLabel = ""; vaultImportMigrationPassphrase = ""; }}>
                                            CANCEL
                                        </button>
                                    </div>
                                </div>
                            {/if}

                            {#if showVaultRestorePanel && hasBackups}
                                <div style="margin-top:0.5rem; border-top:1px dashed rgba(255,255,255,0.06); padding-top:0.6rem;">
                                    <h5 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.7rem;">SELECT BACKUP TO RESTORE</h5>
                                    <div style="max-height:200px; overflow-y:auto; margin-bottom:0.5rem;">
                                        {#each vaultWalletRecords as rec}
                                            <div role="button" tabindex="0" class="key-item" style="padding:0.4rem 0.6rem; cursor:pointer;" class:selected={selectedRecordId === rec.record_id} on:click={() => { selectedRecordId = rec.record_id; vaultRestoreRecordId = rec.record_id; }} on:keydown={(e) => e.key === 'Enter' && (selectedRecordId = rec.record_id, vaultRestoreRecordId = rec.record_id)}>
                                                <div style="flex:1; min-width:0;">
                                                    <div style="color:var(--color-primary); font-size:0.7rem; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">{rec.label}</div>
                                                    <div style="font-size:0.6rem; color:#888;">
                                                        {new Date(rec.modified * 1000).toLocaleString()}
                                                        {#if rec.metadata?.recovery_mode}
                                                            <span style="color:#666;"> • {rec.metadata.recovery_mode === "vault_passphrase" ? "Vault passphrase" : "Separate password"}</span>
                                                        {/if}
                                                    </div>
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                    {#if selectedRecordId}
                                        <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                                            <h4 style="color:var(--color-danger, #ff5555); margin:0 0 0.5rem; font-size:0.75rem;">RESTORE SELECTED BACKUP</h4>
                                            <p class="desc" style="color:#ffaa00; margin-bottom:0.5rem;">
                                                <strong>WARNING:</strong> This creates a new Core runtime wallet. Back up your current wallet first.
                                            </p>
                                            <input type="text" class="input-glass" placeholder="New wallet name (e.g. main)" bind:value={vaultRestoreWalletName} style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                                            {#if vaultRestoreWalletNameError}
                                                <p style="color:#ff7777; font-size:0.62rem; margin:-0.2rem 0 0.4rem; line-height:1.4;">{vaultRestoreWalletNameError}</p>
                                            {/if}
                                            {#if isVaultPassphraseRecovery}
                                                <div style="background:rgba(0,255,65,0.05); border:1px solid rgba(0,255,65,0.15); border-radius:4px; padding:0.5rem 0.75rem; margin-bottom:0.5rem;">
                                                    <p style="margin:0; font-size:0.65rem; color:var(--color-primary);">Uses the unlocked vault passphrase for recovery.</p>
                                                </div>
                                            {:else}
                                                <input type="password" class="input-glass" placeholder="Backup recovery password" bind:value={vaultRestorePassphrase} style="font-size:0.75rem; padding:0.5rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                                            {/if}
                                            <label class="vault-connect-confirm" style="margin-bottom:0.4rem;">
                                                <input type="checkbox" bind:checked={vaultRestoreConfirmChecked} />
                                                <span>I understand this will create a new Core runtime wallet</span>
                                            </label>
                                            <button class="cyber-btn danger small wide" on:click={executeVaultRestore} disabled={vaultRestoreWorking || !vaultRestoreRecordId || !vaultRestoreWalletName || !!vaultRestoreWalletNameError || (!isVaultPassphraseRecovery && !vaultRestorePassphrase) || !vaultRestoreConfirmChecked}>
                                                {vaultRestoreWorking ? "RESTORING…" : "RESTORE WALLET"}
                                            </button>
                                            <p class="desc" style="margin:0.3rem 0 0; color:#666; font-size:0.6rem;">Restores into a Core runtime wallet on disk. Commander will help you load it after restore.</p>
                                        </div>
                                    {:else}
                                        <p class="desc" style="margin:0.5rem 0 0; color:#888; font-size:0.65rem;">Select a backup above to begin restore.</p>
                                    {/if}
                                </div>
                            {/if}
                        </div>

                        <!-- Legacy wallet.dat -->
                        <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                            <h4 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.7rem;">LEGACY WALLET.DAT / KEY TOOLS</h4>
                            <p class="desc" style="margin:0 0 0.6rem; font-size:0.65rem;">
                                Compatibility tools for old Core wallet.dat files and manual key import/export. These are separate from the portable Hemp0x Vault wallet.
                            </p>
                            <div class="btn-row" style="gap:0.5rem; flex-wrap:wrap;">
                                <button class="cyber-btn ghost tiny" on:click={backupWallet}>BACKUP WALLET.DAT</button>
                                <button class="cyber-btn ghost tiny" on:click={unifiedImportWalletFile}>IMPORT WALLET FILE</button>
                                <button class="cyber-btn ghost tiny danger" on:click={createNewWallet}>NEW WALLET</button>
                            </div>
                            <div class="laser-divider" style="margin:0.6rem 0;"></div>
                            <div class="btn-row" style="gap:0.5rem; flex-wrap:wrap;">
                                <button class="cyber-btn ghost tiny danger" on:click={openExportModal}>EXPORT KEYS</button>
                                <button class="cyber-btn ghost tiny" on:click={openImportModal}>IMPORT KEYS</button>
                            </div>
                        </div>

                        <!-- Core migration envelope -->
                        <details style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                            <summary style="cursor:pointer; color:var(--color-primary); font-size:0.7rem; letter-spacing:0.5px; font-weight:700;">EXPERT CORE MIGRATION</summary>
                            <p class="desc" style="margin:0.6rem 0 0.75rem; font-size:0.65rem;">
                                Standalone Core migration envelope tools for diagnostics or offline recovery. Most users should use Import Hemp0x Vault, Save Hemp0x Vault, or the Core wallet snapshots above instead.
                            </p>
                            {#if migrationError}
                                <div style="margin-bottom:0.5rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
                                    <p style="color:#ff7777; font-size:0.65rem; margin:0; word-break:break-word;">{migrationError}</p>
                                </div>
                            {/if}
                            <div class="migration-tools-grid">
                                <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                    <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem;">EXPORT</h4>
                                    <button class="cyber-btn ghost tiny" on:click={openAdvancedExportModal}>EXPORT LOADED WALLET</button>
                                    <p class="desc" style="margin:0; font-size:0.58rem;">Encrypted private v2 export of the exact wallet currently loaded in Core.</p>
                                </div>
                                <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                    <h4 style="color:var(--color-primary); margin:0; font-size:0.7rem;">VALIDATE</h4>
                                    <button class="cyber-btn ghost tiny" on:click={migrateSelectValidatePath}>{migrationValidatePath ? migrationValidatePath.split('/').pop().split('\\').pop() : "SELECT FILE"}</button>
                                    <input type="password" class="input-glass" placeholder="Export passphrase (if encrypted)" bind:value={migrationValidatePass} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <button class="cyber-btn tiny" on:click={migrateValidate} disabled={migrationWorking || !migrationValidatePath}>{migrationWorking ? "…" : "VALIDATE"}</button>
                                    {#if migrationValidateResult}<div style="background:rgba(0,255,65,0.05); padding:0.4rem; border-radius:4px; font-size:0.65rem; max-height:140px; overflow-y:auto;"><p style="color:{migrationValidateResult.valid ? 'var(--color-primary)' : '#ff5555'}; margin:0;">{migrationValidateResult.valid ? 'VALID' : 'INVALID'}</p></div>{/if}
                                </div>
                                <div style="display:flex; flex-direction:column; gap:0.4rem;">
                                    <h4 style="color:var(--color-danger, #ff5555); margin:0; font-size:0.7rem;">RESTORE FROM FILE</h4>
                                    <button class="cyber-btn ghost tiny danger" on:click={migrateSelectRestorePath}>{migrationRestorePath ? migrationRestorePath.split('/').pop().split('\\').pop() : "SELECT FILE"}</button>
                                    <input type="text" class="input-glass" placeholder="New wallet name" bind:value={migrationRestoreName} style="font-size:0.7rem; padding:0.4rem;"/>
                                    {#if migrationRestoreNameError}
                                        <p style="color:#ff7777; font-size:0.62rem; margin:-0.2rem 0 0; line-height:1.4;">{migrationRestoreNameError}</p>
                                    {/if}
                                    <input type="password" class="input-glass" placeholder="Export passphrase" bind:value={migrationRestorePass} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <input type="number" min="0" step="1" class="input-glass" placeholder="Birth height (optional)" bind:value={migrationRestoreBirth} style="font-size:0.7rem; padding:0.4rem;"/>
                                    <label class="vault-connect-confirm">
                                        <input type="checkbox" bind:checked={migrationRestoreConfirmChecked} />
                                        <span>I understand this will create a new Core runtime wallet</span>
                                    </label>
                                    <button class="cyber-btn danger tiny" on:click={migrateRestore} disabled={migrationWorking || !migrationRestorePath || !migrationRestoreName || !!migrationRestoreNameError || !migrationRestorePass || !migrationRestoreConfirmChecked}>{migrationWorking ? "…" : "RESTORE"}</button>
                                    {#if migrationRestoreResult && !migrationError}<div style="background:rgba(0,255,65,0.05); padding:0.4rem; border-radius:4px; font-size:0.65rem;"><p style="color:var(--color-primary); margin:0;">Wallet: {migrationRestoreResult.wallet_name}</p></div>{/if}
                                </div>
                            </div>
                        </details>

                    </div>
                {/if}
            </div>
        </div>
    </div>
{/if}

<!-- ================= MODALS ================= -->

<!-- VAULT BACKUP CONFIRMATION MODAL (60t) -->
{#if showVaultArchiveModal}
    <div class="modal-overlay" role="button" tabindex="0" on:click|self={() => (showVaultArchiveModal = false)} on:keydown={(e) => e.key === "Escape" && (showVaultArchiveModal = false)}>
        <div class="vault-connect-modal" style="max-width:400px;">
            <h3 style="color:var(--color-primary); margin:0 0 0.75rem; font-size:0.9rem; letter-spacing:1px;">SAVE HEMP0X VAULT</h3>
            <p style="color:#aaa; font-size:0.72rem; margin:0 0 0.5rem; line-height:1.5;">
                Save a copy of <span style="color:var(--color-primary);">{vaultBasenameFromPath(vaultOverview?.vault_path)}</span> to a location of your choice. You will be asked where to save the file.
            </p>
            <p style="color:#888; font-size:0.65rem; margin:0 0 0.75rem; line-height:1.4;">
                Use this before switching vaults or creating a new one.
            </p>
            <div style="display:flex; gap:0.5rem;">
                <button class="cyber-btn primary-glow small" style="flex:1;" on:click={executeArchiveVault} disabled={vaultArchiveWorking}>
                    {vaultArchiveWorking ? "SAVING…" : "SAVE VAULT"}
                </button>
                <button class="cyber-btn ghost small" style="flex:1;" on:click={() => (showVaultArchiveModal = false)}>CANCEL</button>
            </div>
        </div>
    </div>
{/if}

<!-- CREATE VAULT FORM — styled to match import flow -->
{#if showCreateVaultForm}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === "Escape" && !createVaultFormWorking && (showCreateVaultForm = false)}
    >
        <div class="vault-connect-modal" style="max-width:420px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                CREATE NEW VAULT
            </h3>
            <div class="laser-divider" style="margin:0 0 0.65rem;"></div>

            {#if hasVault}
                <div class="vault-connect-warn" style="margin-bottom:0.6rem;">
                    Commander will save the current vault as a dated backup, then create a fresh active vault.
                </div>
            {:else}
                <p class="vault-connect-help" style="margin-bottom:0.5rem;">
                    Create a new encrypted active vault. Use Save Hemp0x Vault afterward to export a portable file.
                </p>
            {/if}
            <p class="vault-connect-help" style="color:#ff8888; margin-bottom:0.6rem;">
                <strong>Important:</strong> If you lose your passphrase, your encrypted data cannot be recovered.
            </p>

            <div class="vault-connect-form">
                <div>
                    <label for="create-vault-form-name" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">DISPLAY NAME <span style="color:#666;">(optional)</span></label>
                    <input id="create-vault-form-name" type="text" class="input-glass" bind:value={createVaultFormName} placeholder="e.g. Main Vault" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                </div>
                <div>
                    <label for="create-vault-form-pass" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">VAULT PASSPHRASE <span style="color:#ff8888;">(min 8 chars)</span></label>
                    <input id="create-vault-form-pass" type="password" class="input-glass" bind:value={createVaultFormPass} placeholder="At least 8 characters" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                </div>
                <div>
                    <label for="create-vault-form-pass-confirm" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">CONFIRM PASSPHRASE</label>
                    <input id="create-vault-form-pass-confirm" type="password" class="input-glass" bind:value={createVaultFormPassConfirm} placeholder="Re-enter passphrase" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                </div>
            </div>

            {#if createVaultFormError}
                <div class="vault-connect-error" style="margin-top:0.5rem;">
                    {createVaultFormError}
                </div>
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
                    on:click={() => { showCreateVaultForm = false; createVaultFormPass = ""; createVaultFormPassConfirm = ""; createVaultFormName = ""; createVaultFormError = ""; }}
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
    <div class="modal-overlay" role="button" tabindex="0" on:keydown={(e) => {
        if (e.key === "Escape" && !encryptWorking) {
            showEncryptModal = false;
            encryptTargetWalletName = null;
        }
    }}>
        <div class="vault-connect-modal" style="max-width:400px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px;">
                SET WALLET PASSWORD
            </h3>
            <p style="color:#bbb; font-size:0.68rem; margin:0 0 0.75rem; line-height:1.4;">
                <strong style="color:#ff5555;">IMPORTANT:</strong> If you lose this password, you will
                <strong style="color:#ff5555;">LOSE ACCESS</strong> to your wallet and will need to re-import a backed-up vault. Save it securely.
            </p>
            <input
                type="password"
                class="input-glass"
                placeholder="New password"
                bind:value={newEncPass}
                style="font-size:0.75rem; padding:0.5rem; width:100%; box-sizing:border-box; margin-bottom:0.4rem;"
            />
            <input
                type="password"
                class="input-glass"
                placeholder="Confirm password"
                bind:value={newEncPassConfirm}
                style="font-size:0.75rem; padding:0.5rem; width:100%; box-sizing:border-box; margin-bottom:0.75rem;"
            />
            {#if encryptWorking}
                <div class="vault-connect-loading" style="margin-bottom:0.75rem;">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Encrypting wallet…</p>
                </div>
            {:else}
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn primary-glow small" style="flex:1;" on:click={performWalletEncrypt}>ENCRYPT</button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={() => {
                        showEncryptModal = false;
                        encryptTargetWalletName = null;
                    }}>CANCEL</button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- EXPORT DANGER CONFIRMATION MODAL -->
{#if showExportDangerModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
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
                SET EXPORT PASSWORD
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
        style="z-index:5000;"
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === "Escape" && closeUnlockModal()}
    >
        <div class="vault-connect-modal" style="max-width:400px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                {unlockingFile ? "DECRYPT FILE" : "UNLOCK CORE WALLET"}
            </h3>
            <p style="color:#aaa; font-size:0.7rem; margin:0 0 0.6rem;">
                {unlockingFile
                    ? "Enter password to decrypt file:"
                    : unlockModalPurpose === "vault_backup"
                        ? "Enter the Core wallet passphrase to unlock the runtime wallet for backup. This is separate from your vault passphrase."
                        : unlockModalPurpose === "wallet_unlock"
                            ? "Enter your Core wallet passphrase to unlock the runtime wallet."
                            : unlockModalPurpose === "vault_guided_connect"
                                ? "Enter your Core wallet passphrase so Commander can back up the current runtime wallet before connecting the vault wallet."
                                : "Enter your Core wallet passphrase to export keys."}
            </p>
            <input
                type="password"
                class="input-glass"
                placeholder="Passphrase"
                bind:value={unlockPassword}
                on:keydown={(e) =>
                    e.key === "Enter" && tryUnlockWallet()}
                style="font-size:0.8rem; padding:0.55rem; width:100%; box-sizing:border-box; margin-bottom:0.5rem;"
            />
            {#if unlockError}
                <div style="margin-bottom:0.5rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
                    <p style="color:#ff7777; font-size:0.7rem; margin:0;">{unlockError}</p>
                </div>
            {/if}
            <div style="display:flex; gap:0.5rem;">
                <button class="cyber-btn primary-glow small" style="flex:1;" on:click={tryUnlockWallet}>UNLOCK</button>
                <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnlockModal}>CANCEL</button>
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
        on:keydown={(e) => e.key === "Escape" && !processingKeys && (showKeyModal = false)}
    >
        <div class="modal-staged wide">
            <div class="modal-header">
                <h3>
                    {keyModalMode === "export"
                        ? "EXPORT PRIVATE KEYS"
                        : "IMPORT PRIVATE KEYS"}
                </h3>
                <button
                    class="btn-close-x"
                    on:click={() => (showKeyModal = false)}>X</button
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

<!-- ALIGNMENT REVIEW MODAL (64c / 64f / 64g) -->
{#if showAlignmentReviewModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:keydown={(e) => !connectWorking && e.key === "Escape" && (showAlignmentReviewModal = false)}
    >
        <div class="vault-connect-modal" style="max-width:440px; max-height:85vh; overflow-y:auto; padding:1rem 1.15rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.8rem; letter-spacing:1px;">
                VAULT WALLET CONNECT
            </h3>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if alignmentConnectPlanLoading}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#666; font-size:0.7rem; margin:0;">Reading vault plan…</p>
                </div>
            {:else if alignmentConnectPlanError}
                <p style="color:#ff5555; font-size:0.7rem; margin:0; text-align:center;">{alignmentConnectPlanError}</p>
            {:else if alignmentConnectPlan}
                <!-- Compact plan summary -->
                <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#aaa; margin-bottom:0.75rem;">
                    <div><span style="color:#666;">Network:</span> {alignmentConnectPlan.webcom_primary_network || "—"}</div>
                    <div><span style="color:#666;">Derivation:</span> {alignmentConnectPlan.webcom_primary_derivation_hemp || "—"}</div>
                    <div><span style="color:#666;">History scan:</span> {alignmentConnectPlan.rescan_window_label || "—"}</div>
                </div>

                {#if alignmentConnectPlan.can_guided_connect}
                    {#if connectWorking}
                        <!-- WORKING VIEW: replaces the form with a stepper -->
                        <div class="vault-connect-working">
                            <div class="vault-connect-spinner vault-connect-spinner-lg"></div>
                            <p class="vault-connect-working-step">{CONNECT_STEPS[connectStep] || "Working…"}</p>
                            <div class="vault-connect-progress">
                                <div class="vault-connect-progress-bar"></div>
                            </div>
                            <ol class="vault-connect-stepper">
                                {#each CONNECT_STEPS as step, i}
                                    <li class:done={i < connectStep} class:active={i === connectStep}>
                                        <span class="step-dot"></span>
                                        <span class="step-label">{step}</span>
                                    </li>
                                {/each}
                            </ol>
                            <p style="color:#666; font-size:0.55rem; text-align:center; margin:0.5rem 0 0;">
                                The restore can take a few minutes while Core checks wallet history. Please keep the app open.
                            </p>
                        </div>
                    {:else if connectResult}
                        <!-- SUCCESS VIEW -->
                        <div class="vault-connect-result">
                            <div class="vault-connect-success">
                                <span class="vault-connect-check">&#10003;</span>
                                <span>
                                    {#if connectResult.named_wallet_loaded}
                                        Vault wallet <strong>{connectResult.core_wallet_name}</strong> is restored and loaded in Core.
                                    {:else if connectResult.wallet_state?.wallet_file_exists}
                                        Vault wallet <strong>{connectResult.core_wallet_name}</strong> is restored on disk, but Core cannot query it yet.
                                    {:else}
                                        Vault wallet restore complete.
                                    {/if}
                                </span>
                            </div>
                            <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#aaa;">
                                <div><span style="color:#666;">State:</span> {connectResult.connection_state}</div>
                                {#if connectResult.wallet_balance?.balance !== undefined && connectResult.wallet_balance?.balance !== null}
                                    <div>
                                        <span style="color:#666;">Core wallet balance:</span>
                                        <strong style="color:var(--color-primary);">
                                            {connectResult.wallet_balance.balance}
                                        </strong> HEMP
                                        <span style="color:#666;">
                                            (from <code>{connectResult.core_wallet_name}</code>)
                                        </span>
                                        {#if connectResult.wallet_balance.txcount !== undefined}
                                            <span style="color:#666;">• {connectResult.wallet_balance.txcount} tx</span>
                                        {/if}
                                        {#if connectResult.wallet_balance.scanning !== undefined && connectResult.wallet_balance.scanning !== null}
                                            <span style="color:#666;">• scanning: {JSON.stringify(connectResult.wallet_balance.scanning)}</span>
                                        {/if}
                                    </div>
                                {/if}
                                {#if connectResult.utxo_scan}
                                    <div>
                                        <span style="color:#666;">UTXOs found by chain scan:</span>
                                        {connectResult.utxo_scan.total_amount} HEMP across {connectResult.utxo_scan.utxo_count} UTXOs
                                        <span style="color:#666;">(scanned {connectResult.utxo_scan.scanned_addresses} derived addresses)</span>
                                    </div>
                                {/if}
                                {#if connectResult.wallet_address_count}
                                    <div><span style="color:#666;">Derived addresses:</span> {connectResult.wallet_address_count}</div>
                                {/if}
                                {#if connectResult.first_tx_block_height}
                                    <div><span style="color:#666;">First activity at block:</span> {connectResult.first_tx_block_height}</div>
                                {/if}
                            </div>
                            {#if connectResult.wallet_load_restart_required}
                                <div class="vault-connect-warn">
                                    <strong>Core needs to restart to load this vault wallet.</strong>
                                    The vault wallet file is on disk at
                                    <code>{connectResult.wallet_state?.wallet_file_path}</code>,
                                    but Core cannot query it yet.
                                </div>
                                {#if restartingCore}
                                    <div class="vault-connect-loading">
                                        <span class="vault-connect-spinner"></span>
                                        <p style="color:#aaa; font-size:0.7rem; margin:0;">Restarting Core... please wait, this may take a few moments.</p>
                                    </div>
                                {:else}
                                    <button
                                        class="cyber-btn primary-glow small wide"
                                        on:click={async () => {
                                            restartingCore = true;
                                            await new Promise((r) => setTimeout(r, 50));
                                            try {
                                                await core.invoke("vault_restart_core_with_wallet", { walletName: connectResult.core_wallet_name });
                                                // Poll until wallet is queryable (non-blocking intervals)
                                                let loaded = false;
                                                for (let i = 0; i < 20; i++) {
                                                    await new Promise((r) => setTimeout(r, 1500));
                                                    try {
                                                        const newState = await core.invoke("vault_load_wallet_into_core", { walletName: connectResult.core_wallet_name });
                                                        if (newState.loaded) { loaded = true; break; }
                                                    } catch (_) {}
                                                }
                                                if (loaded) {
                                                    connectResult.named_wallet_loaded = true;
                                                    connectResult.wallet_load_restart_required = false;
                                                }
                                            } catch (e) {
                                                showToast("Restart failed: " + e, "error");
                                            }
                                            restartingCore = false;
                                        }}
                                    >
                                        RESTART CORE WITH THIS WALLET
                                    </button>
                                {/if}
                            {:else if connectResult.named_wallet_loaded}
                                <div class="vault-connect-info">
                                    <strong>Vault wallet is loaded in Core.</strong>
                                    It is queryable as <code>{connectResult.core_wallet_name}</code>.
                                </div>
                            {/if}
                            {#if connectResult.deep_rescan_triggered}
                                <div class="vault-connect-info">
                                    <strong>Balance is live, history is filling in.</strong>
                                    Commander has started a background history scan for this wallet. Your balance is correct immediately. Full transaction history will appear over the next few minutes.
                                </div>
                            {:else if connectResult.history_rescan?.skipped_reason === "named_wallet_not_loaded"}
                                <div class="vault-connect-info">
                                    <strong>History scan was not started.</strong>
                                    The background history scan only runs after the vault wallet is loaded in Core.
                                </div>
                            {/if}
                            {#if connectResult.runtime_wallet_encryption === "needs_user_action"}
                                <div class="vault-connect-warn">
                                    <strong>Runtime wallet needs encryption.</strong>
                                    The Core runtime wallet is currently unencrypted on disk.
                                </div>
                                <button
                                    class="cyber-btn primary-glow small wide"
                                    on:click={() => {
                                        showAlignmentReviewModal = false;
                                        showEncryptModal = true;
                                    }}
                                >
                                    ENCRYPT RUNTIME WALLET
                                </button>
                            {/if}
                        </div>
                    {:else}
                        <!-- IDLE / READY VIEW -->
                        <div class="vault-connect-form">
                            <p class="vault-connect-help">
                                {#if alignmentConnectPlan.pre_connect_backup_required}
                                    Commander will first back up your current Core runtime wallet into the Hemp0x Vault, then restore the vault wallet into Core. If the Core wallet is locked, you will be asked to unlock it first.
                                {:else}
                                    Commander will restore this Hemp0x Vault wallet into Core and verify that it loaded correctly.
                                {/if}
                            </p>
                            <p class="vault-connect-help" style="color:#888;">
                                Back up your current wallet first (recommended), or skip backup and connect directly. The new vault wallet is created as a separate Core wallet file (for example
                                <code>{connectWalletName || "hemp0x-vault-main"}</code>) and loaded in Core.
                            </p>
                            {#if alignmentConnectPlan.pre_connect_backup_required}
                                <label class="vault-connect-confirm">
                                    <input type="checkbox" bind:checked={skipBackup} />
                                    <span>Skip backup — connect without backing up current wallet</span>
                                </label>
                            {/if}
                            {#if alignmentConnectPlan.rescan_window_label}
                                <div class="vault-connect-info" style="font-size:0.55rem; padding:0.4rem 0.6rem;">
                                    <strong>History scan:</strong> {alignmentConnectPlan.rescan_window_label}
                                    <br />
                                    Your Core wallet balance should be available quickly. Full transaction history is checked in the background after the wallet is loaded.
                                </div>
                            {/if}
                            <input
                                type="text"
                                class="input-glass"
                                placeholder="Wallet name (e.g. hemp0x-vault-main)"
                                bind:value={connectWalletName}
                            />
                            {#if connectWalletNameError}
                                <p style="color:#ff7777; font-size:0.62rem; margin:0.25rem 0 0; line-height:1.4;">{connectWalletNameError}</p>
                            {/if}
                            <label class="vault-connect-confirm">
                                <input type="checkbox" bind:checked={connectConfirmChecked} />
                                <span>I understand this will create a new named wallet. My existing Core wallet will not be overwritten.</span>
                            </label>
                            <button
                                class="cyber-btn primary-glow small wide"
                                on:click={executeConnectVaultWallet}
                                disabled={!connectConfirmChecked || !!connectWalletNameError}
                            >
                                {alignmentConnectPlan.guided_connect_label || "CONNECT VAULT WALLET"}
                            </button>
                            {#if connectError}
                                <div class="vault-connect-error">
                                    {#if connectErrorCode === "wallet_unlock_required"}
                                        <strong>Core wallet is locked.</strong><br />{connectError}
                                    {:else if connectErrorCode === "restore_timeout"}
                                        <strong>Restore timed out.</strong><br />{connectError}
                                    {:else if connectErrorCode === "restart_required"}
                                        <strong>Restart required.</strong><br />{connectError}
                                    {:else if connectErrorCode === "wallet_not_loaded"}
                                        <strong>Vault wallet not loaded yet.</strong><br />{connectError}
                                    {:else}
                                        {connectError}
                                    {/if}
                                </div>
                            {/if}
                        </div>
                    {/if}
                {:else}
                    <!-- BLOCKED VIEW -->
                    <div class="vault-connect-blocked">
                        <strong style="color:#ffaa00;">Unavailable</strong>
                        <p style="color:#aaa; font-size:0.7rem; margin:0.3rem 0 0;">{alignmentConnectPlan.blocker_detail || alignmentConnectPlan.blocker || "Core daemon is not running or wallet cannot be connected."}</p>
                    </div>
                {/if}

                {#if !connectWorking && !connectResult}
                    <details style="margin-top:0.75rem;">
                        <summary style="cursor:pointer; color:#888; font-size:0.6rem; letter-spacing:0.5px;">WHAT WILL HAPPEN</summary>
                        <ol style="margin:0.4rem 0 0; padding-left:1.2rem; color:#aaa; font-size:0.6rem; line-height:1.5;">
                            {#if alignmentConnectPlan.pre_connect_backup_required}
                                <li>Back up the current Core wallet into the vault (using your vault passphrase).</li>
                            {/if}
                            <li>Prepare the vault wallet data for Core.</li>
                            <li>Ask Core to validate the restore data.</li>
                            <li>Restore it into a new named Core wallet.</li>
                            <li>Verify the restored wallet matches the expected vault wallet profile.</li>
                            <li>Write a verified alignment record so future vault loads detect this wallet.</li>
                        </ol>
                    </details>
                {/if}
            {/if}

            <div class="laser-divider" style="margin:0.75rem 0;"></div>

            <div style="display:flex; gap:0.5rem;">
                <button
                    class="cyber-btn ghost small"
                    on:click={() => (showAlignmentReviewModal = false)}
                    disabled={connectWorking}
                    style="flex:1;"
                >
                    {connectResult ? "DONE" : "CLOSE"}
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- IMPORT / REPLACE VAULT CONFIRMATION OVERLAY -->
{#if switchImportArchiveArmed}
    <div class="modal-overlay" role="button" tabindex="0" on:click|self={() => { switchImportArchiveArmed = false; switchImportPendingPath = ""; }} on:keydown={(e) => e.key === "Escape" && (switchImportArchiveArmed = false, switchImportPendingPath = "")}>
        <div class="vault-connect-modal" style="max-width:420px;">
            <h3 style="color:var(--color-primary); margin:0 0 0.75rem; font-size:0.9rem; letter-spacing:1px;">REPLACE LOADED VAULT?</h3>
            <p style="color:#aaa; font-size:0.72rem; margin:0 0 0.75rem; line-height:1.5;">
                This will replace your currently loaded vault with the selected file. Unlock the imported vault to connect its wallet to Core.
            </p>
            <label class="vault-connect-confirm" style="margin-bottom:0.75rem;">
                <input type="checkbox" bind:checked={switchImportPendingArchive} />
                <span>Archive current vault first (saves a dated backup)</span>
            </label>
            <div style="display:flex; gap:0.5rem;">
                <button class="cyber-btn primary-glow small" style="flex:1;" on:click={doConfirmImportSwitch}>REPLACE LOADED VAULT</button>
                <button class="cyber-btn ghost small" style="flex:1;" on:click={() => { switchImportArchiveArmed = false; switchImportPendingPath = ""; switchImportPendingArchive = false; }}>CANCEL</button>
            </div>
        </div>
    </div>
{/if}

<!-- IMPORT UNLOCK POPUP — shown after vault is loaded, accepts passphrase to unlock -->
{#if showImportUnlockPopup}
    <div class="modal-overlay" role="button" tabindex="0" on:click|self={() => {}} on:keydown={(e) => e.key === "Escape" && (showImportUnlockPopup = false)}>
        <div class="vault-connect-modal" style="max-width:400px; padding:1.5rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.9rem; letter-spacing:1px;">UNLOCK IMPORTED VAULT</h3>
            <p style="color:#aaa; font-size:0.7rem; margin:0 0 0.75rem; line-height:1.4;">
                Enter the passphrase matching the vault you just imported.
            </p>
            <input
                type="password"
                class="input-glass"
                placeholder="Imported vault passphrase"
                bind:value={importUnlockPassphrase}
                on:keydown={(e) => e.key === 'Enter' && !importUnlockWorking && executeImportUnlock()}
                style="font-size:0.8rem; padding:0.55rem; width:100%; box-sizing:border-box; margin-bottom:0.5rem;"
            />
	            {#if importUnlockError}
	                <div style="margin-bottom:0.5rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
	                    <p style="color:#ff7777; font-size:0.7rem; margin:0; font-weight:700;">{importUnlockError}</p>
	                    {#if importUnlockErrorDetails}
	                        <p style="color:#bbb; font-size:0.65rem; margin:0.2rem 0 0; word-break:break-word;">{importUnlockErrorDetails}</p>
	                    {/if}
	                </div>
	            {/if}
	            {#if importUnlockWorking}
	                <div class="vault-connect-loading" style="margin-bottom:0.75rem;">
	                    <span class="vault-connect-spinner"></span>
	                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Unlocking vault... please wait.</p>
	                </div>
	            {/if}
	            <div style="display:flex; gap:0.5rem;">
                <button class="cyber-btn primary-glow small" style="flex:1;" on:click={executeImportUnlock} disabled={importUnlockWorking || !importUnlockPassphrase}>
                    {importUnlockWorking ? "UNLOCKING…" : "UNLOCK VAULT"}
                </button>
                <button class="cyber-btn ghost small" style="flex:1;" on:click={() => { showImportUnlockPopup = false; }}>
                    CANCEL
                </button>
            </div>
        </div>
    </div>
{/if}

<!-- RECOVERY PHRASE RESTORE MODAL -->
{#if showRecoveryPhraseModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:keydown={(e) => !recoveryPhraseWorking && e.key === "Escape" && (showRecoveryPhraseModal = false)}
    >
        <div class="vault-connect-modal" style="max-width:440px; max-height:90vh; overflow-y:auto; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                RESTORE FROM RECOVERY PHRASE
            </h3>
            <p style="color:#aaa; font-size:0.68rem; margin:0 0 0.6rem; line-height:1.4;">
                Enter your 12, 18, or 24-word BIP39 recovery phrase. Commander will create a vault wallet backed by this phrase and restore it into Core.
            </p>
            <div style="margin-bottom:0.5rem;">
                <label for="recovery-phrase-words" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">RECOVERY PHRASE</label>
                <textarea
                    id="recovery-phrase-words"
                    class="input-glass"
                    bind:value={recoveryPhraseWords}
                    placeholder="Enter each word separated by a space"
                    rows="3"
                    style="font-size:0.7rem; padding:0.5rem; width:100%; box-sizing:border-box; resize:vertical; font-family:monospace;"
                ></textarea>
            </div>
            <div style="margin-bottom:0.5rem;">
                <label for="recovery-wallet-name" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">WALLET NAME</label>
                <input
                    id="recovery-wallet-name"
                    type="text"
                    class="input-glass"
                    bind:value={recoveryWalletName}
                    placeholder="e.g. hemp0x-vault-main"
                    style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;"
                />
                {#if recoveryWalletNameError}
                    <p style="color:#ff7777; font-size:0.62rem; margin:0.25rem 0 0; line-height:1.4;">{recoveryWalletNameError}</p>
                {/if}
            </div>
            <div style="margin-bottom:0.5rem;">
                <label for="recovery-birth-height" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">BIRTH HEIGHT <span style="color:#666;">(optional)</span></label>
                <input
                    id="recovery-birth-height"
                    type="number"
                    min="0"
                    step="1"
                    class="input-glass"
                    bind:value={recoveryBirthHeight}
                    placeholder="Block height to start scanning from"
                    style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;"
                />
            </div>
            <div style="border-top:1px dashed rgba(255,255,255,0.06); margin:0.5rem 0; padding-top:0.5rem;">
                {#if vaultUnlocked}
                    <p style="color:var(--color-primary); font-size:0.6rem; margin:0 0 0.5rem; line-height:1.4;">
                        Your vault is already unlocked. The restored wallet will be added to your active Hemp0x Vault.
                    </p>
                {:else if hasVault}
                    <p style="color:#ffaa00; font-size:0.6rem; margin:0 0 0.5rem; line-height:1.4;">
                        A Hemp0x Vault exists but is locked. Unlock the vault first, then restore from your recovery phrase.
                    </p>
                {:else}
                    <p style="color:#888; font-size:0.6rem; margin:0 0 0.5rem; line-height:1.4;">
                        Commander will create a new Hemp0x Vault and store the restored wallet in it. Set a passphrase to encrypt the new vault.
                    </p>
                    <div style="margin-bottom:0.4rem;">
                        <label for="recovery-phrase-pass" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">VAULT PASSPHRASE <span style="color:#ff8888;">(min 8 chars)</span></label>
                        <input
                            id="recovery-phrase-pass"
                            type="password"
                            class="input-glass"
                            bind:value={recoveryPhrasePassphrase}
                            placeholder="At least 8 characters"
                            style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;"
                        />
                    </div>
                    <div style="margin-bottom:0.6rem;">
                        <label for="recovery-phrase-pass-confirm" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">CONFIRM PASSPHRASE</label>
                        <input
                            id="recovery-phrase-pass-confirm"
                            type="password"
                            class="input-glass"
                            bind:value={recoveryPhrasePassphraseConfirm}
                            placeholder="Re-enter passphrase"
                            style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;"
                        />
                    </div>
                {/if}
            </div>
            {#if recoveryPhraseError}
                <div style="margin-bottom:0.5rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
                    <p style="color:#ff7777; font-size:0.65rem; margin:0; word-break:break-word;">{recoveryPhraseError}</p>
                </div>
            {/if}
            {#if recoveryPhraseWorking}
                <div class="vault-connect-loading" style="margin-bottom:0.5rem;">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Restoring wallet from recovery phrase…</p>
                </div>
            {:else}
                <div style="display:flex; gap:0.5rem;">
                    <button
                        class="cyber-btn primary-glow small"
                        style="flex:1;"
                        on:click={executeRecoveryPhraseRestore}
                        disabled={!recoveryPhraseWords || (!!recoveryWalletNameError && recoveryWalletName.trim()) || (!vaultUnlocked && (!recoveryPhrasePassphrase || recoveryPhrasePassphrase.length < 8 || recoveryPhrasePassphrase !== recoveryPhrasePassphraseConfirm))}
                    >
                        RESTORE WALLET
                    </button>
                    <button
                        class="cyber-btn ghost small"
                        style="flex:1;"
                        on:click={() => { showRecoveryPhraseModal = false; recoveryPhraseWords = ""; recoveryPhrasePassphrase = ""; recoveryPhrasePassphraseConfirm = ""; recoveryWalletName = ""; recoveryBirthHeight = ""; recoveryPhraseError = ""; }}
                    >
                        CANCEL
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- CREATE NEW VAULT WALLET MODAL -->
{#if showCreateWalletModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:keydown={(e) => !createWalletWorking && e.key === "Escape" && closeCreateWalletModal()}
    >
        <div class="vault-connect-modal" style="max-width:440px; max-height:90vh; overflow-y:auto; padding:1.25rem;">
            {#if createWalletDone}
                <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                    WALLET CREATED
                </h3>
                <div class="vault-connect-success" style="margin-bottom:0.6rem;">
                    <span class="vault-connect-check">&#10003;</span>
                    <span>Wallet stored in vault and loaded in Core.</span>
                </div>
                <div style="border:1px solid rgba(255,170,0,0.3); background:rgba(255,170,0,0.06); border-radius:5px; padding:0.6rem 0.75rem; margin-bottom:0.6rem;">
                    <p style="color:#ffaa00; font-size:0.6rem; margin:0 0 0.3rem; font-weight:700;">SAVE YOUR RECOVERY PHRASE</p>
                    <p style="color:#aaa; font-size:0.6rem; margin:0 0 0.4rem; line-height:1.4;">Write this phrase down and store it offline. It is also encrypted inside your Hemp0x Vault.</p>
                    <div style="background:rgba(0,0,0,0.4); border:1px dashed rgba(255,255,255,0.1); border-radius:4px; padding:0.5rem; display:flex; flex-wrap:wrap; gap:0.25rem; justify-content:center; user-select:all;">
                        {#each createWalletMnemonic.split(/\s+/).filter(Boolean) as word, index}
                            <span style="font-family:monospace; font-size:0.64rem; color:var(--color-primary); border:1px solid rgba(0,255,65,0.16); background:rgba(0,255,65,0.04); border-radius:4px; padding:0.16rem 0.32rem; white-space:nowrap;">
                                {index + 1}. {word}
                            </span>
                        {/each}
                    </div>
                </div>
                <label class="vault-connect-confirm" style="margin-bottom:0.6rem;">
                    <input type="checkbox" bind:checked={createWalletPhraseConfirmed} />
                    <span>I saved this phrase or I understand it remains encrypted inside my vault.</span>
                </label>
                <button class="cyber-btn primary-glow small wide" on:click={closeCreateWalletModal} disabled={!createWalletPhraseConfirmed}>
                    CONTINUE
                </button>
            {:else if createWalletWorking}
                <div class="vault-connect-loading" style="padding:1.5rem 0;">
                    <span class="vault-connect-spinner vault-connect-spinner-lg"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0.5rem 0 0;">Generating wallet and storing in vault…</p>
                </div>
            {:else}
                <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                    CREATE NEW WALLET
                </h3>
                <p style="color:#aaa; font-size:0.68rem; margin:0 0 0.6rem; line-height:1.4;">
                    Commander will generate a new BIP39 recovery phrase and store it securely in your Hemp0x Vault. The wallet will be restored into Core automatically.
                </p>
                <div style="margin-bottom:0.5rem;">
                    <label for="create-wallet-name" style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">WALLET NAME</label>
                    <input
                        id="create-wallet-name"
                        type="text"
                        class="input-glass"
                        bind:value={createWalletName}
                        placeholder="e.g. hemp0x-vault-main"
                        style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;"
                    />
                    {#if createWalletNameError}
                        <p style="color:#ff7777; font-size:0.62rem; margin:0.25rem 0 0; line-height:1.4;">{createWalletNameError}</p>
                    {/if}
                </div>
                <div style="margin-bottom:0.5rem;">
                    <span style="font-size:0.65rem; color:#888; margin-bottom:0.2rem; display:block;">RECOVERY PHRASE LENGTH</span>
                    <div style="display:flex; gap:0.4rem;">
                        <button type="button" class="cyber-btn small wide" class:primary-glow={createWalletWordCount === 12} class:ghost={createWalletWordCount !== 12} on:click={() => (createWalletWordCount = 12)}>12 WORDS</button>
                        <button type="button" class="cyber-btn small wide" class:primary-glow={createWalletWordCount === 24} class:ghost={createWalletWordCount !== 24} on:click={() => (createWalletWordCount = 24)}>24 WORDS</button>
                    </div>
                </div>
                <div style="border-top:1px dashed rgba(255,255,255,0.06); margin:0.5rem 0; padding-top:0.5rem;">
                    <p style="color:#888; font-size:0.6rem; margin:0 0 0.5rem; line-height:1.4;">
                        Enter your vault passphrase to authorize storing the new wallet.
                    </p>
                    <input
                        type="password"
                        class="input-glass"
                        placeholder="Vault passphrase (min 8 chars)"
                        bind:value={createWalletPassphrase}
                        style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box; margin-bottom:0.6rem;"
                    />
                </div>
                {#if createWalletError}
                    <div style="margin-bottom:0.5rem; border:1px solid rgba(255,85,85,0.35); background:rgba(255,0,0,0.08); border-radius:5px; padding:0.5rem 0.6rem;">
                        <p style="color:#ff7777; font-size:0.65rem; margin:0; word-break:break-word;">{createWalletError}</p>
                    </div>
                {/if}
                <div style="display:flex; gap:0.5rem;">
                    <button
                        class="cyber-btn primary-glow small"
                        style="flex:1;"
                        on:click={executeCreateVaultWallet}
                        disabled={!createWalletPassphrase || createWalletPassphrase.length < 8 || !!createWalletNameError}
                    >
                        CREATE WALLET
                    </button>
                    <button
                        class="cyber-btn ghost small"
                        style="flex:1;"
                        on:click={closeCreateWalletModal}
                    >
                        CANCEL
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- ENCRYPTION COMPLETE OVERLAY -->
{#if showEncryptCompleteOverlay}
    <div class="modal-overlay" role="button" tabindex="0" on:keydown={(e) => e.key === "Escape" && !encryptCompleteRestarting && (showEncryptCompleteOverlay = false)}>
        <div class="vault-connect-modal" style="max-width:400px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px;">ENCRYPTION COMPLETE</h3>
            <p style="color:#aaa; font-size:0.7rem; margin:0 0 0.75rem; line-height:1.4;">
                Your wallet is now encrypted. The node has been stopped for security. Restart Core to load your encrypted wallet.
            </p>
            {#if encryptCompleteRestarting}
                <div class="vault-connect-loading" style="margin-bottom:0.75rem;">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Restarting Core... please wait, this may take a few moments.</p>
                </div>
            {:else}
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn primary-glow small" style="flex:1;" on:click={async () => {
                        encryptCompleteRestarting = true;
                        await new Promise((r) => setTimeout(r, 50));
                        try {
                            const activeWalletName = encryptedWalletRestartName
                                || connectResult?.core_wallet_name
                                || walletStatus?.walletname;
                            if (activeWalletName && activeWalletName !== "default" && activeWalletName !== "wallet.dat") {
                                await core.invoke("vault_set_active_wallet_name", { walletName: activeWalletName });
                                activeVaultWalletName = activeWalletName;
                                await core.invoke("vault_restart_core_with_wallet", { walletName: activeWalletName });
                                for (let i = 0; i < 20; i++) {
                                    await new Promise((r) => setTimeout(r, 1500));
                                    try {
                                        const newState = await core.invoke("vault_load_wallet_into_core", { walletName: activeWalletName });
                                        if (newState.loaded) {
                                            break;
                                        }
                                    } catch (_) {}
                                }
                            } else {
                                await core.invoke("start_node");
                            }
                            await loadActiveVaultWalletName();
                            await refreshWalletHeader();
                            await loadAlignmentStatus();
                            encryptedWalletRestartName = "";
                            showEncryptCompleteOverlay = false;
                            encryptCompleteRestarting = false;
                            window.setTimeout(() => {
                                startPendingHistoryRecovery();
                            }, 250);
                        } catch (e) {
                            encryptCompleteRestarting = false;
                            showToast("Restart failed: " + e, "error");
                        }
                    }}>
                        RESTART CORE
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={() => (showEncryptCompleteOverlay = false)} disabled={encryptCompleteRestarting}>
                        CLOSE
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- VAULT WALLET SWITCH OVERLAY -->
{#if walletSwitchWorking}
    <div class="modal-overlay" role="presentation">
        <div class="vault-connect-modal" style="max-width:400px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.5rem; font-size:0.85rem; letter-spacing:1px;">
                SWITCHING WALLET
            </h3>
            <p style="color:#aaa; font-size:0.7rem; margin:0 0 0.75rem; line-height:1.4;">
                {#if walletSwitchTargetName === "wallet.dat"}
                    Commander is restarting Core in wallet.dat mode while keeping your Hemp0x Vault available.
                {:else}
                    Commander is restarting Core and loading the Hemp0x Vault wallet{walletSwitchTargetName ? `: ${walletSwitchTargetName}` : ""}.
                {/if}
            </p>
            <div class="vault-connect-loading" style="margin-bottom:0.25rem;">
                <span class="vault-connect-spinner"></span>
                <p style="color:#aaa; font-size:0.7rem; margin:0;">Please wait while wallet status is confirmed.</p>
            </div>
        </div>
    </div>
{/if}

<!-- UNIFIED WALLET FILE IMPORT MODAL (65) -->
{#if showUnifiedImportModal}
    <div
        class="modal-overlay"
        role="presentation"
        style="align-items:flex-start; box-sizing:border-box; padding:0.35rem 0.75rem 1rem; overflow-y:auto;"
    >
        <div class="vault-connect-modal unified-wallet-import-modal" style="max-width:440px; padding:0.85rem;">
            <div style="display:flex; align-items:center; justify-content:space-between; gap:0.75rem; margin-bottom:0.35rem;">
                <h3 style="color:var(--color-primary); margin:0; font-size:0.82rem; letter-spacing:1px;">
                    IMPORT WALLET FILE
                </h3>
                <button
                    class="cyber-btn ghost tiny"
                    style="min-width:2rem; padding:0.3rem 0.45rem;"
                    on:click={closeUnifiedImportModal}
                    disabled={unifiedImportWorking || unifiedPromotionWorking || unifiedImportSnapshotWorking}
                    aria-label="Close import wallet file"
                >
                    X
                </button>
            </div>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if unifiedImportWorking}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Detecting file type…</p>
                </div>
            {:else if unifiedImportResult}
                <!-- SUCCESS VIEW (legacy import result) -->
                <div class="vault-connect-result">
                    <div class="vault-connect-success">
                        <span class="vault-connect-check">&#10003;</span>
                        <span>
                            {#if unifiedImportResult.already_active}
                                Already using this wallet.dat. Switched to legacy wallet mode.
                            {:else}
                                Legacy wallet.dat imported successfully.
                            {/if}
                        </span>
                    </div>
                    <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#aaa; margin-top:0.4rem;">
                        <div><span style="color:#888;">Archived existing wallet:</span> {unifiedImportResult.archived_existing ? 'Yes' : 'No previous wallet'}</div>
                        {#if unifiedImportResult.hemp_conf_wallet}
                            <div class="vault-connect-warn" style="margin-top:0.25rem;">
                                <strong>Warning:</strong> hemp.conf contains <code>wallet={unifiedImportResult.hemp_conf_wallet}</code>. Core may load that wallet instead of the default wallet.dat.
                            </div>
                        {/if}
                        {#if unifiedImportResult.restart_error}
                            <div class="vault-connect-error" style="margin-top:0.25rem;">
                                {unifiedImportResult.restart_error}
                            </div>
                        {/if}
                        {#if unifiedImportResult.restarted}
                            <div style="color:var(--color-primary); margin-top:0.25rem;">Core restarted with default wallet mode.</div>
                        {/if}
                    </div>
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal}>
                        CLOSE
                    </button>
                </div>
            {:else if unifiedImportSnapshotResult}
                <!-- SUCCESS VIEW (snapshot import) -->
                <div class="vault-connect-result">
                    <div class="vault-connect-success">
                        <span class="vault-connect-check">&#10003;</span>
                        <span>Migration envelope stored as vault recovery snapshot.</span>
                    </div>
                    <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#aaa; margin-top:0.4rem;">
                        <div><span style="color:#888;">Record:</span> {unifiedImportSnapshotResult.label} ({unifiedImportSnapshotResult.record_id})</div>
                    </div>
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal}>
                        CLOSE
                    </button>
                </div>
            {:else if unifiedImportDetectedType === "hemp0x_vault"}
                <div class="vault-connect-info" style="margin-bottom:0.6rem;">
                    <strong>Detected: Hemp0x Vault</strong>
                    {#if unifiedImportDetection?.network}
                        <span style="color:#888;"> — {unifiedImportDetection.network}</span>
                    {/if}
                </div>
                <p class="vault-connect-help" style="margin-bottom:0.65rem;">
                    This file is a portable Hemp0x Vault. Commander can switch to it and then unlock it with that vault passphrase.
                </p>
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem; margin-bottom:0.6rem;">
                    <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">IMPORT / SWITCH HEMP0X VAULT</h5>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                        Loads this vault file into Commander. If another vault is already loaded, Commander will ask before replacing it.
                    </p>
                    <button
                        class="cyber-btn primary-glow tiny"
                        on:click={executeUnifiedVaultImport}
                    >
                        {hasVault ? "SWITCH TO THIS VAULT" : "IMPORT HEMP0X VAULT"}
                    </button>
                </div>
                <div class="laser-divider" style="margin:0.5rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal}>
                        CLOSE
                    </button>
                </div>
            {:else if unifiedImportDetectedType === "legacy_core_wallet"}
                <!-- DETECTED: Legacy Core wallet file -->
                <div class="vault-connect-info" style="margin-bottom:0.6rem;">
                    <strong>Detected: Legacy Core wallet file</strong>
                    {#if unifiedImportDetection?.wallet_format}
                        <span style="color:#888;"> ({unifiedImportDetection.wallet_format})</span>
                    {/if}
                </div>
                <p class="vault-connect-help" style="margin-bottom:0.5rem;">
                    Commander can use this wallet.dat as the Core runtime wallet (legacy mode), or promote it into the portable Hemp0x Vault if it is a canonical BIP39/coin420 wallet.
                </p>

                {#if unifiedImportError}
                    <div class="vault-connect-error" style="margin-bottom:0.5rem;">
                        {unifiedImportError}
                    </div>
                {/if}

                <div class="wallet-import-options">
                <!-- Option 1: Use as Legacy Core Wallet -->
                <div class="wallet-import-option legacy" style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                    <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">USE AS LEGACY CORE WALLET</h5>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                        Use this wallet.dat as the Core runtime wallet. This is the legacy Core wallet mode. Your selected file will be copied, not moved. The current runtime wallet will be archived first.
                    </p>
                    <div class="vault-connect-warn" style="margin-bottom:0.5rem;">
                        <strong>After import, Commander will stop using the currently connected vault wallet for Core startup.</strong>
                    </div>
                    <label class="vault-connect-confirm" style="margin-bottom:0.5rem;">
                        <input type="checkbox" bind:checked={unifiedImportConfirmChecked} />
                        <span>I understand Commander will use this wallet.dat instead of the connected vault wallet.</span>
                    </label>
                    <button
                        class="cyber-btn primary-glow tiny"
                        on:click={executeUnifiedLegacyImport}
                        disabled={!unifiedImportConfirmChecked || unifiedImportWorking}
                    >
                        {unifiedImportWorking ? "IMPORTING…" : "IMPORT WALLET.DAT"}
                    </button>
                </div>

                <!-- Option 2: Add to Hemp0x Vault -->
                <div class="wallet-import-option vault" style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem;">
                    <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">ADD TO HEMP0X VAULT</h5>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                        Export this Core wallet as an encrypted migration envelope, then promote it into the WebCom-compatible portable Hemp0x Vault primary record. Only canonical BIP39/BIP44 coin420 wallets can be promoted. Legacy or imported-key wallets will be reported accurately.
                    </p>
                    {#if vaultUnlocked}
                        {#if unifiedPromotionWorking}
                            <div class="vault-connect-loading" style="padding:0.5rem 0;">
                                <span class="vault-connect-spinner"></span>
                                {#if unifiedPromotionProgressLabel}
                                    <p style="color:#aaa; font-size:0.65rem; margin:0.3rem 0 0;">
                                        {unifiedPromotionProgressLabel}
                                    </p>
                                {/if}
                                <ol class="vault-connect-stepper" style="margin-top:0.65rem;">
                                    {#each UNIFIED_PROMOTION_STEPS as step, i}
                                        <li class:active={i === unifiedPromotionStep} class:done={i < unifiedPromotionStep}>
                                            <span class="step-dot"></span>
                                            <span>{step}</span>
                                        </li>
                                    {/each}
                                </ol>
                            </div>
                        {:else if unifiedPromotionResult}
                            <div class="vault-connect-success" style="margin-bottom:0.5rem;">
                                <span class="vault-connect-check">&#10003;</span>
                                <span>Wallet promoted to portable Hemp0x Vault primary record.</span>
                            </div>
                            <div style="font-size:0.6rem; color:#aaa; margin-bottom:0.5rem;">
                                <div>Wallet: {unifiedPromotionResult.core_wallet_name}</div>
                                <div>Profile: {unifiedPromotionResult.derivation_profile}</div>
                                {#if unifiedPromotionResult.wallet_load_restart_required}
                                    <div class="vault-connect-warn" style="margin-top:0.3rem;">Restart Core to load the vault wallet.</div>
                                {/if}
                            </div>
                            {#if unifiedPromotionResult.runtime_wallet_encryption === "needs_user_action"}
                                <div class="vault-connect-warn" style="margin-bottom:0.5rem;">
                                    <strong>Runtime wallet needs encryption.</strong>
                                    Set a wallet password before using this Core wallet.
                                </div>
                                <button
                                    class="cyber-btn primary-glow tiny"
                                    on:click={() => {
                                        encryptTargetWalletName = unifiedPromotionResult.core_wallet_name;
                                        closeUnifiedImportModal();
                                        showEncryptModal = true;
                                    }}
                                >
                                    ENCRYPT RUNTIME WALLET
                                </button>
                            {/if}
                        {:else}
                            <input type="text" class="input-glass" placeholder="Wallet name (default: hemp0x-vault-main)" bind:value={unifiedPromotionWalletName} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            <input type="password" class="input-glass" placeholder="Core wallet unlock passphrase (if encrypted)" bind:value={unifiedPromotionWalletUnlockPass} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            <label class="vault-connect-confirm" style="margin-bottom:0.5rem; font-size:0.62rem;">
                                <input type="checkbox" bind:checked={unifiedPromotionReplaceExisting} />
                                <span>Replace existing primary vault wallet (if different)</span>
                            </label>
                            {#if unifiedPromotionError}
                                <div class="vault-connect-error" style="margin-bottom:0.4rem;">
                                    {unifiedPromotionError}
                                </div>
                                {#if unifiedPromotionNeedsRuntimeLoad}
                                    <button
                                        class="cyber-btn ghost tiny"
                                        style="margin-bottom:0.45rem;"
                                        on:click={loadSelectedWalletFileAsRuntime}
                                        disabled={unifiedImportWorking || unifiedPromotionWorking}
                                    >
                                        {unifiedImportWorking ? "LOADING WALLET…" : "LOAD AS RUNTIME WALLET"}
                                    </button>
                                {/if}
                            {/if}
                            <button
                                class="cyber-btn primary-glow tiny"
                                on:click={executeUnifiedPromotion}
                            >
                                ADD TO HEMP0X VAULT
                            </button>
                        {/if}
                    {:else}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0;">
                            Unlock your Hemp0x Vault first to add this wallet to the vault.
                        </p>
                    {/if}
                </div>
                </div>

                <div class="laser-divider" style="margin:0.5rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal} disabled={unifiedImportWorking || unifiedPromotionWorking}>
                        CLOSE
                    </button>
                </div>
            {:else if unifiedImportDetectedType === "core_migration_envelope"}
                <!-- DETECTED: Core migration envelope -->
                <div class="vault-connect-info" style="margin-bottom:0.6rem;">
                    <strong>Detected: Core migration envelope</strong>
                    {#if unifiedImportDetection?.migration_wallet_name}
                        <span style="color:#888;"> — wallet: {unifiedImportDetection.migration_wallet_name}</span>
                    {/if}
                    {#if unifiedImportDetection?.migration_encrypted}
                        <span style="color:#ffaa00;"> (encrypted)</span>
                    {/if}
                </div>
                <p class="vault-connect-help" style="margin-bottom:0.6rem;">
                    This is a Core Next wallet migration envelope. Commander can store it as a recovery snapshot inside your Hemp0x Vault, or you can use it as a standalone Core restore from the Expert Core Migration Files section.
                </p>

                {#if unifiedImportError}
                    <div class="vault-connect-error" style="margin-bottom:0.5rem;">
                        {unifiedImportError}
                    </div>
                {/if}

                <!-- Option 1: Use as portable vault wallet -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem; margin-bottom:0.6rem;">
                    <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">USE AS PORTABLE VAULT WALLET</h5>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                        Decrypt this Core migration envelope, verify the canonical BIP39/coin420 profile, and promote it into the WebCom-compatible portable Hemp0x Vault primary record. The matching Core runtime wallet will be restored and aligned.
                    </p>
                    {#if !unifiedImportDetection?.migration_encrypted}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0 0 0.5rem;">
                            This is a public-only migration envelope. It cannot be promoted to a portable wallet. Only encrypted private v2 envelopes can be promoted.
                        </p>
                    {:else if !unifiedImportDetection?.migration_private_keys_included}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0 0 0.5rem;">
                            This envelope does not include private keys. Only private encrypted envelopes can be promoted.
                        </p>
                    {:else if vaultUnlocked}
                        {#if unifiedPromotionWorking}
                            <div class="vault-connect-loading" style="padding:0.5rem 0;">
                                <span class="vault-connect-spinner"></span>
                                {#if unifiedPromotionProgressLabel}
                                    <p style="color:#aaa; font-size:0.65rem; margin:0.3rem 0 0;">
                                        {unifiedPromotionProgressLabel}
                                    </p>
                                {/if}
                                <ol class="vault-connect-stepper" style="margin-top:0.65rem;">
                                    {#each UNIFIED_PROMOTION_STEPS as step, i}
                                        <li class:active={i === unifiedPromotionStep} class:done={i < unifiedPromotionStep}>
                                            <span class="step-dot"></span>
                                            <span>{step}</span>
                                        </li>
                                    {/each}
                                </ol>
                            </div>
                        {:else if unifiedPromotionResult}
                            <div class="vault-connect-success" style="margin-bottom:0.5rem;">
                                <span class="vault-connect-check">&#10003;</span>
                                <span>Wallet promoted to portable Hemp0x Vault primary record.</span>
                            </div>
                            <div style="font-size:0.6rem; color:#aaa; margin-bottom:0.5rem;">
                                <div>Wallet: {unifiedPromotionResult.core_wallet_name}</div>
                                <div>Profile: {unifiedPromotionResult.derivation_profile}</div>
                                {#if unifiedPromotionResult.wallet_load_restart_required}
                                    <div class="vault-connect-warn" style="margin-top:0.3rem;">Restart Core to load the vault wallet.</div>
                                {/if}
                            </div>
                            {#if unifiedPromotionResult.runtime_wallet_encryption === "needs_user_action"}
                                <div class="vault-connect-warn" style="margin-bottom:0.5rem;">
                                    <strong>Runtime wallet needs encryption.</strong>
                                    Set a wallet password before using this Core wallet.
                                </div>
                                <button
                                    class="cyber-btn primary-glow tiny"
                                    on:click={() => {
                                        encryptTargetWalletName = unifiedPromotionResult.core_wallet_name;
                                        closeUnifiedImportModal();
                                        showEncryptModal = true;
                                    }}
                                >
                                    ENCRYPT RUNTIME WALLET
                                </button>
                            {/if}
                        {:else}
                            <input type="password" class="input-glass" placeholder="Migration file passphrase" bind:value={unifiedPromotionPassphrase} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            <input type="text" class="input-glass" placeholder="Wallet name (default: hemp0x-vault-main)" bind:value={unifiedPromotionWalletName} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            <input type="password" class="input-glass" placeholder="Core wallet unlock passphrase (if encrypted)" bind:value={unifiedPromotionWalletUnlockPass} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            <label class="vault-connect-confirm" style="margin-bottom:0.5rem; font-size:0.62rem;">
                                <input type="checkbox" bind:checked={unifiedPromotionReplaceExisting} />
                                <span>Replace existing primary vault wallet (if different)</span>
                            </label>
                            {#if unifiedPromotionError}
                                <div class="vault-connect-error" style="margin-bottom:0.4rem;">
                                    {unifiedPromotionError}
                                </div>
                            {/if}
                            <button
                                class="cyber-btn primary-glow tiny"
                                on:click={executeUnifiedPromotion}
                                disabled={!unifiedPromotionPassphrase}
                            >
                                PROMOTE TO PORTABLE VAULT WALLET
                            </button>
                        {/if}
                    {:else}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0;">
                            Unlock your Hemp0x Vault first to promote this migration envelope.
                        </p>
                    {/if}
                </div>

                <!-- Option 2: Store as recovery snapshot -->
                <div style="background:rgba(0,0,0,0.2); border:1px solid rgba(255,255,255,0.06); border-radius:6px; padding:0.75rem 1rem; margin-bottom:0.6rem;">
                    <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">STORE AS VAULT RECOVERY SNAPSHOT</h5>
                    <p class="desc" style="margin:0 0 0.5rem; font-size:0.62rem;">
                        Stores this migration envelope as a recovery record inside your Hemp0x Vault. This does not replace the primary portable vault wallet. Requires an unlocked vault.
                    </p>
                    {#if !unifiedImportDetection?.migration_private_keys_included}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0;">
                            This migration envelope is public metadata only. It can be inspected, but it cannot restore a wallet or be stored as a recovery snapshot.
                        </p>
                    {:else if vaultUnlocked}
                        {#if unifiedImportSnapshotWorking}
                            <div class="vault-connect-loading" style="padding:0.5rem 0;">
                                <span class="vault-connect-spinner"></span>
                                <p style="color:#aaa; font-size:0.65rem; margin:0;">Storing in vault...</p>
                            </div>
                        {:else}
                            <input type="text" class="input-glass" placeholder="Record label (e.g. Old wallet backup)" bind:value={unifiedImportSnapshotLabel} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.4rem; width:100%; box-sizing:border-box;" />
                            {#if unifiedImportDetection?.migration_encrypted}
                                <input type="password" class="input-glass" placeholder="Migration file passphrase" bind:value={unifiedImportMigrationPassphrase} style="font-size:0.72rem; padding:0.45rem; margin-bottom:0.5rem; width:100%; box-sizing:border-box;" />
                            {/if}
                            {#if unifiedImportSnapshotError}
                                <div class="vault-connect-error" style="margin-bottom:0.4rem;">
                                    {unifiedImportSnapshotError}
                                </div>
                            {/if}
                            <button
                                class="cyber-btn primary-glow tiny"
                                on:click={executeUnifiedSnapshotImport}
                                disabled={!unifiedImportSnapshotLabel}
                            >
                                STORE IN VAULT
                            </button>
                        {/if}
                    {:else}
                        <p style="color:#ffaa00; font-size:0.65rem; margin:0;">
                            Unlock your Hemp0x Vault first to store this migration envelope as a recovery snapshot.
                        </p>
                    {/if}
                </div>

                <div class="laser-divider" style="margin:0.5rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal} disabled={unifiedImportWorking || unifiedImportSnapshotWorking || unifiedPromotionWorking}>
                        CLOSE
                    </button>
                </div>
            {:else if unifiedImportDetectedType === "unknown"}
                <!-- DETECTED: Unknown -->
                <div class="vault-connect-error" style="margin-bottom:0.6rem;">
                    <strong>Unknown or unsupported file</strong>
                </div>
                <p class="vault-connect-help" style="margin-bottom:0.5rem;">
                    Commander could not identify this file as a legacy Core wallet or a Core migration envelope.
                </p>
                {#if unifiedImportError}
                    <div class="vault-connect-error" style="margin-bottom:0.5rem;">
                        {unifiedImportError}
                    </div>
                {/if}
                {#if unifiedImportDetection}
                    <details style="margin-bottom:0.6rem;">
                        <summary style="cursor:pointer; color:#888; font-size:0.6rem; letter-spacing:0.5px;">DETECTION DETAILS</summary>
                        <div style="margin-top:0.4rem; font-size:0.6rem; color:#aaa; line-height:1.5;">
                            {#if unifiedImportDetection.wallet_error}
                                <div><span style="color:#888;">Wallet check:</span> {unifiedImportDetection.wallet_error}</div>
                            {/if}
                            {#if unifiedImportDetection.migration_error}
                                <div><span style="color:#888;">Migration check:</span> {unifiedImportDetection.migration_error}</div>
                            {/if}
                            <div><span style="color:#888;">File:</span> {unifiedImportDetection.file_name} ({unifiedImportDetection.file_size} bytes)</div>
                        </div>
                    </details>
                {/if}
                <div class="laser-divider" style="margin:0.5rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnifiedImportModal}>
                        CLOSE
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- CHANGE VAULT PASSPHRASE MODAL (64p) -->
{#if showChangeVaultPassphraseModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => closeChangeVaultPassphraseModal()}
        on:keydown={(e) => e.key === "Escape" && closeChangeVaultPassphraseModal()}
    >
        <div class="vault-connect-modal" style="max-width:440px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                CHANGE VAULT PASSPHRASE
            </h3>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if changeVaultPassResult}
                <!-- SUCCESS VIEW -->
                <div class="vault-connect-result">
                    <div class="vault-connect-success">
                        <span class="vault-connect-check">&#10003;</span>
                        <span>Vault passphrase changed successfully. The vault stays unlocked.</span>
                    </div>
                    <div class="vault-connect-warn" style="margin-top:0.5rem;">
                        <strong>Save your Hemp0x Vault file now.</strong> Your new passphrase protects the same records; the old passphrase will no longer work.
                    </div>
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn primary-glow small" style="flex:1;" on:click={() => { promptArchiveVault(); dismissChangeVaultPassphraseSuccess(); }}>
                        SAVE VAULT FILE
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={dismissChangeVaultPassphraseSuccess}>
                        DONE
                    </button>
                </div>
            {:else if changeVaultPassWorking}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Rotating vault passphrase…</p>
                </div>
            {:else}
                <p class="vault-connect-help" style="margin-bottom:0.6rem;">
                    Re-encrypt your Hemp0x Vault with a new passphrase. Every record inside the vault is preserved unchanged. This does not change your Core wallet password or your wallet.dat.
                </p>
                <div class="vault-connect-form">
                    <div>
                        <label for="chvp-current" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">CURRENT VAULT PASSPHRASE</label>
                        <input id="chvp-current" type="password" class="input-glass" bind:value={changeVaultPassCurrent} placeholder="Current passphrase" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                    <div>
                        <label for="chvp-new" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">NEW VAULT PASSPHRASE <span style="color:#ff8888;">(min 8 chars)</span></label>
                        <input id="chvp-new" type="password" class="input-glass" bind:value={changeVaultPassNew} placeholder="At least 8 characters" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                    <div>
                        <label for="chvp-confirm" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">CONFIRM NEW PASSPHRASE</label>
                        <input id="chvp-confirm" type="password" class="input-glass" bind:value={changeVaultPassConfirm} placeholder="Re-enter new passphrase" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                </div>

                {#if changeVaultPassError}
                    <div class="vault-connect-error" style="margin-top:0.5rem;">
                        {changeVaultPassError}
                    </div>
                {/if}

                <div style="display:flex; gap:0.5rem; margin-top:0.75rem;">
                    <button
                        class="cyber-btn primary-glow small"
                        style="flex:1;"
                        on:click={executeChangeVaultPassphrase}
                        disabled={!changeVaultPassCurrent || !changeVaultPassNew || changeVaultPassNew.length < 8 || changeVaultPassNew !== changeVaultPassConfirm}
                    >
                        CHANGE PASSPHRASE
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeChangeVaultPassphraseModal}>
                        CANCEL
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- CHANGE RUNTIME WALLET PASSWORD MODAL -->
{#if showChangeRuntimeWalletPassModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => closeChangeRuntimeWalletPassModal()}
        on:keydown={(e) => e.key === "Escape" && closeChangeRuntimeWalletPassModal()}
    >
        <div class="vault-connect-modal" style="max-width:440px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                CHANGE WALLET PASSWORD
            </h3>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if changeRuntimeWalletPassResult}
                <div class="vault-connect-result">
                    <div class="vault-connect-success">
                        <span class="vault-connect-check">&#10003;</span>
                        <span>Runtime wallet password changed successfully.</span>
                    </div>
                    <p class="vault-connect-help" style="margin-top:0.55rem;">
                        Wallet: {changeRuntimeWalletPassResult.wallet_name || "wallet.dat"}
                    </p>
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <button class="cyber-btn primary-glow small wide" on:click={dismissChangeRuntimeWalletPassSuccess}>
                    DONE
                </button>
            {:else if changeRuntimeWalletPassWorking}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Changing runtime wallet password…</p>
                </div>
            {:else}
                <p class="vault-connect-help" style="margin-bottom:0.6rem;">
                    Change the Core password for the currently loaded runtime wallet. This is separate from your Hemp0x Vault passphrase.
                </p>
                <div class="vault-connect-form">
                    <div style="font-size:0.62rem; color:#aaa;">
                        <span style="color:#888;">Wallet:</span> {loadedRuntimeWalletName() || "wallet.dat"}
                    </div>
                    <div>
                        <label for="chrwp-current" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">CURRENT WALLET PASSWORD</label>
                        <input id="chrwp-current" type="password" class="input-glass" bind:value={changeRuntimeWalletPassCurrent} placeholder="Current wallet password" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                    <div>
                        <label for="chrwp-new" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">NEW WALLET PASSWORD <span style="color:#ff8888;">(min 8 chars)</span></label>
                        <input id="chrwp-new" type="password" class="input-glass" bind:value={changeRuntimeWalletPassNew} placeholder="At least 8 characters" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                    <div>
                        <label for="chrwp-confirm" style="font-size:0.6rem; color:#888; margin-bottom:0.15rem; display:block;">CONFIRM NEW PASSWORD</label>
                        <input id="chrwp-confirm" type="password" class="input-glass" bind:value={changeRuntimeWalletPassConfirm} placeholder="Re-enter new wallet password" style="font-size:0.75rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    </div>
                </div>

                {#if changeRuntimeWalletPassError}
                    <div class="vault-connect-error" style="margin-top:0.5rem;">
                        {changeRuntimeWalletPassError}
                    </div>
                {/if}

                <div style="display:flex; gap:0.5rem; margin-top:0.75rem;">
                    <button
                        class="cyber-btn primary-glow small"
                        style="flex:1;"
                        on:click={executeChangeRuntimeWalletPassphrase}
                        disabled={!changeRuntimeWalletPassCurrent || !changeRuntimeWalletPassNew || changeRuntimeWalletPassNew.length < 8 || changeRuntimeWalletPassNew !== changeRuntimeWalletPassConfirm}
                    >
                        CHANGE PASSWORD
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeChangeRuntimeWalletPassModal}>
                        CANCEL
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- UNLOAD VAULT MODAL (64p) -->
{#if showUnloadVaultModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => closeUnloadVaultModal()}
        on:keydown={(e) => e.key === "Escape" && closeUnloadVaultModal()}
    >
        <div class="vault-connect-modal" style="max-width:460px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                UNLOAD HEMP0X VAULT
            </h3>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if unloadVaultResult}
                <!-- RESULT VIEW -->
                <div class="vault-connect-result">
                    {#if unloadVaultResult.no_legacy_wallet}
                        <div class="vault-connect-warn" style="margin-bottom:0.5rem;">
                            <strong>No wallet.dat was found.</strong> Commander unloaded the active Hemp0x Vault and did not silently create a wallet. Choose a next action below.
                        </div>
                        <div style="display:flex; flex-direction:column; gap:0.4rem; font-size:0.66rem; color:#aaa;">
                            <div><span style="color:#888;">Vault file preserved:</span> {unloadVaultResult.vault_file_preserved ? 'Yes' : 'No'}</div>
                            {#if unloadVaultResult.archive_path}
                                <div><span style="color:#888;">Archived to:</span> {vaultBasenameFromPath(unloadVaultResult.archive_path)}</div>
                            {/if}
                            <div><span style="color:#888;">Active vault wallet cleared:</span> {unloadVaultResult.active_vault_wallet_name_cleared ? 'Yes' : 'No'}</div>
                            {#if unloadVaultResult.hemp_conf_wallet}
                                <div class="vault-connect-warn" style="margin-top:0.25rem;">
                                    <strong>Warning:</strong> hemp.conf contains <code>wallet={unloadVaultResult.hemp_conf_wallet}</code>. Core may load that wallet instead of the default wallet.dat. Edit the wallet= line in hemp.conf if you want to use the default wallet.
                                </div>
                            {/if}
                        </div>
                        <div class="laser-divider" style="margin:0.75rem 0;"></div>
                        <h5 style="color:var(--color-primary); margin:0 0 0.4rem; font-size:0.7rem;">NEXT ACTIONS</h5>
                        <div style="display:flex; flex-direction:column; gap:0.4rem;">
                            <button class="cyber-btn ghost small wide" on:click={() => { dismissUnloadVaultResult(); createNewWallet(); }}>CREATE NEW WALLET.DAT</button>
                            <button class="cyber-btn ghost small wide" on:click={() => { dismissUnloadVaultResult(); unifiedImportWalletFile(); }}>IMPORT WALLET FILE</button>
                            <button class="cyber-btn ghost small wide" on:click={() => { dismissUnloadVaultResult(); importVaultBundle(); }}>IMPORT / SWITCH HEMP0X VAULT</button>
                        </div>
                    {:else}
                        <div class="vault-connect-success">
                            <span class="vault-connect-check">&#10003;</span>
                            <span>Vault unloaded. Core restarted in legacy wallet.dat mode.</span>
                        </div>
                        <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.66rem; color:#aaa; margin-top:0.4rem;">
                            <div><span style="color:#888;">Vault file preserved:</span> {unloadVaultResult.vault_file_preserved ? 'Yes' : 'No'}</div>
                            {#if unloadVaultResult.archive_path}
                                <div><span style="color:#888;">Archived to:</span> {vaultBasenameFromPath(unloadVaultResult.archive_path)}</div>
                            {/if}
                            <div><span style="color:#888;">Runtime wallet file preserved:</span> {unloadVaultResult.vault_runtime_wallet_preserved ? 'Yes' : 'No'}</div>
                            <div><span style="color:#888;">Active vault wallet cleared:</span> {unloadVaultResult.active_vault_wallet_name_cleared ? 'Yes' : 'No'}</div>
                            {#if unloadVaultResult.hemp_conf_wallet}
                                <div class="vault-connect-warn" style="margin-top:0.25rem;">
                                    <strong>Warning:</strong> hemp.conf contains <code>wallet={unloadVaultResult.hemp_conf_wallet}</code>. Core may load that wallet instead of the default wallet.dat.
                                </div>
                            {/if}
                        </div>
                    {/if}

                    {#if unloadVaultError}
                        <div class="vault-connect-error" style="margin-top:0.5rem;">
                            {unloadVaultError}
                        </div>
                    {/if}
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={dismissUnloadVaultResult}>
                        CLOSE
                    </button>
                </div>
            {:else if unloadVaultWorking}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Unloading vault and restarting Core in wallet.dat mode…</p>
                </div>
            {:else if unloadVaultError}
                <div class="vault-connect-error">
                    <strong>Unload failed.</strong><br />{unloadVaultError}
                </div>
                <div style="display:flex; gap:0.5rem; margin-top:0.75rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={() => { unloadVaultError = ""; }}>
                        BACK
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnloadVaultModal}>
                        CANCEL
                    </button>
                </div>
            {:else}
                <!-- CONFIRMATION VIEW -->
                <p class="vault-connect-help" style="margin-bottom:0.5rem;">
                    Commander will unload the active Hemp0x Vault from this install and preserve it as an archived vault file. If wallet.dat exists, Core will restart in wallet.dat mode.
                </p>
                <div style="border:1px solid rgba(0,255,65,0.18); background:rgba(0,255,65,0.04); border-radius:5px; padding:0.65rem 0.75rem; color:#aaa; font-size:0.66rem; line-height:1.55; margin-bottom:0.6rem;">
                    <ul style="margin:0; padding-left:1rem; line-height:1.55;">
                        <li>The vault file is archived, not deleted.</li>
                        <li>The vault runtime wallet file is <strong>not</strong> deleted.</li>
                        <li>You can import / switch back to this vault later.</li>
                        <li>Core will stop using this vault wallet for startup.</li>
                        <li><code>hemp.conf</code> is never modified by this action.</li>
                    </ul>
                </div>
                <p class="vault-connect-help" style="margin-bottom:0.6rem;">
                    If no <code>wallet.dat</code> exists, Commander will not silently create one. You will be offered options to create or import a wallet.
                </p>
                <label class="vault-connect-confirm" style="margin-bottom:0.75rem;">
                    <input type="checkbox" bind:checked={unloadVaultConfirmChecked} />
                    <span>I understand Commander will unload this active Hemp0x Vault from this install.</span>
                </label>
                <div style="display:flex; gap:0.5rem;">
                    <button
                        class="cyber-btn primary-glow small"
                        style="flex:1;"
                        on:click={executeUnloadVault}
                        disabled={!unloadVaultConfirmChecked || unloadVaultWorking}
                    >
                        {unloadVaultWorking ? "UNLOADING…" : "UNLOAD VAULT"}
                    </button>
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeUnloadVaultModal} disabled={unloadVaultWorking}>
                        CANCEL
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- ADVANCED EXPORT MODAL (66b) -->
{#if showAdvancedExportModal}
    <div
        class="modal-overlay"
        role="button"
        tabindex="0"
        on:click|self={() => { if (!advancedExportWorking) closeAdvancedExportModal(); }}
        on:keydown={(e) => e.key === "Escape" && !advancedExportWorking && closeAdvancedExportModal()}
    >
        <div class="vault-connect-modal" style="max-width:460px; padding:1.25rem;">
            <h3 style="color:var(--color-primary); margin:0 0 0.35rem; font-size:0.85rem; letter-spacing:1px; text-align:center;">
                EXPORT CORE MIGRATION WALLET
            </h3>
            <div class="laser-divider" style="margin:0 0 0.75rem;"></div>

            {#if advancedExportWorking}
                <div class="vault-connect-loading">
                    <span class="vault-connect-spinner"></span>
                    <p style="color:#aaa; font-size:0.7rem; margin:0;">Exporting wallet…</p>
                </div>
            {:else if advancedExportResult}
                <div class="vault-connect-result">
                    <div class="vault-connect-success">
                        <span class="vault-connect-check">&#10003;</span>
                        <span>Core migration wallet exported successfully.</span>
                    </div>
                    <div style="display:flex; flex-direction:column; gap:0.25rem; font-size:0.6rem; color:#aaa; margin-top:0.4rem;">
                        <div><span style="color:#888;">Destination:</span> {advancedExportResult.destination}</div>
                        {#if advancedExportResult.wallet_name}
                            <div><span style="color:#888;">Wallet:</span> {advancedExportResult.wallet_name}</div>
                        {/if}
                        {#if advancedExportResult.exporting_wallet}
                            <div><span style="color:#888;">Exported from:</span> {advancedExportResult.exporting_wallet}</div>
                        {/if}
                        {#if advancedExportResult.validated}
                            <div style="color:var(--color-primary);">Core validation: passed</div>
                        {/if}
                    </div>
                </div>
                <div class="laser-divider" style="margin:0.75rem 0;"></div>
                <div style="display:flex; gap:0.5rem;">
                    <button class="cyber-btn ghost small" style="flex:1;" on:click={closeAdvancedExportModal}>
                        CLOSE
                    </button>
                </div>
            {:else}
                <p class="vault-connect-help" style="margin-bottom:0.6rem;">
                    Export the currently loaded Core wallet as an encrypted private v2 migration envelope to a user-chosen destination. The envelope can be restored later or promoted into a portable Hemp0x Vault wallet.
                </p>
                <div class="vault-connect-form">
                    <div style="display:flex; gap:0.4rem; align-items:center;">
                        <input type="text" class="input-glass" placeholder="Destination path" bind:value={advancedExportPath} style="flex:1; font-size:0.72rem; padding:0.45rem;" readonly />
                        <button class="cyber-btn ghost tiny" on:click={selectAdvancedExportPath}>BROWSE</button>
                    </div>
                    <input type="password" class="input-glass" placeholder="Export passphrase (min 8 characters)" bind:value={advancedExportPassphrase} style="font-size:0.72rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    <input type="password" class="input-glass" placeholder="Core wallet unlock passphrase (if encrypted and locked)" bind:value={advancedExportWalletUnlockPass} style="font-size:0.72rem; padding:0.45rem; width:100%; box-sizing:border-box;" />
                    <label class="vault-connect-confirm" style="margin-bottom:0.2rem;">
                        <input type="checkbox" bind:checked={advancedExportOverwrite} />
                        <span>Allow overwrite if destination exists</span>
                    </label>
                    {#if advancedExportError}
                        <div class="vault-connect-error" style="margin-bottom:0.4rem;">
                            {advancedExportError}
                        </div>
                    {/if}
                    <button
                        class="cyber-btn primary-glow small"
                        on:click={executeAdvancedExport}
                        disabled={!advancedExportPath || advancedExportPassphrase.length < 8}
                    >
                        EXPORT
                    </button>
                </div>
            {/if}
        </div>
    </div>
{/if}

<style>
    .card-header {
        background: rgba(0, 255, 65, 0.05);
        padding: 0.8rem 1.2rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
    }
    .panel-soft {
        background: rgba(0, 0, 0, 0.25);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
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
    /* Key list styles */
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

    /* ─── Vault connect modal styles (slice 64g) ─────────────────────── */
    .vault-connect-loading {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
        padding: 1rem 0;
    }
    .vault-connect-spinner {
        width: 14px;
        height: 14px;
        display: inline-block;
        border: 2px solid rgba(0, 255, 65, 0.15);
        border-top-color: var(--color-primary);
        border-radius: 50%;
        will-change: transform;
        animation: vault-spin 0.7s linear infinite;
    }
    .vault-connect-spinner-lg {
        width: 32px;
        height: 32px;
        border-width: 3px;
        display: block;
        margin: 0 auto 0.5rem;
    }
    @keyframes vault-spin {
        100% { transform: rotate(360deg); }
    }

    .vault-connect-working {
        text-align: center;
        padding: 0.5rem 0 0.25rem;
    }
    .vault-connect-working-step {
        color: var(--color-primary);
        font-size: 0.7rem;
        margin: 0.6rem 0 0.5rem;
        letter-spacing: 0.5px;
    }
    .vault-connect-progress {
        height: 3px;
        background: rgba(0, 255, 65, 0.1);
        border-radius: 2px;
        overflow: hidden;
        position: relative;
        margin: 0 0.5rem 0.75rem;
    }
    .vault-connect-progress-bar {
        position: absolute;
        top: 0;
        left: -40%;
        width: 40%;
        height: 100%;
        background: linear-gradient(90deg,
            transparent,
            var(--color-primary),
            transparent);
        animation: vault-sweep 1.6s linear infinite;
    }
    @keyframes vault-sweep {
        to { left: 100%; }
    }
    .vault-connect-stepper {
        list-style: none;
        padding: 0;
        margin: 0.5rem 0 0;
        text-align: left;
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .vault-connect-stepper li {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: 0.6rem;
        color: #555;
        transition: color 0.2s;
    }
    .vault-connect-stepper li.active {
        color: var(--color-primary);
    }
    .vault-connect-stepper li.done {
        color: #6e6;
    }
    .vault-connect-stepper .step-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: rgba(0, 255, 65, 0.15);
        flex-shrink: 0;
        transition: background 0.2s, box-shadow 0.2s;
    }
    .vault-connect-stepper li.active .step-dot {
        background: var(--color-primary);
        box-shadow: 0 0 8px var(--color-primary);
        animation: vault-pulse 1.2s ease-in-out infinite;
    }
    .vault-connect-stepper li.done .step-dot {
        background: #6e6;
    }
    @keyframes vault-pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.4; }
    }

    .vault-connect-form {
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
    }
    .vault-connect-help {
        color: #888;
        font-size: 0.6rem;
        line-height: 1.4;
        margin: 0;
    }
    .vault-connect-confirm {
        display: flex;
        align-items: flex-start;
        gap: 0.4rem;
        font-size: 0.6rem;
        color: #aaa;
        cursor: pointer;
        line-height: 1.4;
    }
    .vault-connect-confirm input[type="checkbox"] {
        margin-top: 0.15rem;
        accent-color: var(--color-primary);
        flex-shrink: 0;
    }
    .vault-connect-error {
        background: rgba(255, 85, 85, 0.08);
        border: 1px solid rgba(255, 85, 85, 0.3);
        border-radius: 4px;
        padding: 0.5rem 0.7rem;
        color: #ff8888;
        font-size: 0.6rem;
        line-height: 1.4;
    }
    .vault-connect-blocked {
        background: rgba(255, 170, 0, 0.08);
        border: 1px solid rgba(255, 170, 0, 0.3);
        border-radius: 6px;
        padding: 0.7rem 0.9rem;
        color: #ffaa00;
        font-size: 0.65rem;
        line-height: 1.4;
    }
    .vault-connect-modal {
        background: rgba(0, 0, 0, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.08);
        border-radius: 8px;
        padding: 1rem;
        box-shadow: 0 0 40px rgba(0, 0, 0, 0.8);
        width: 90%;
        margin: 0 auto;
    }
    .unified-wallet-import-modal {
        max-height: min(calc(100dvh - 6rem), 620px);
        overflow-y: auto;
        overscroll-behavior: contain;
        box-sizing: border-box;
        margin-top: 0;
    }
    .wallet-import-options {
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
    }
    .wallet-import-option.vault {
        order: 1;
    }
    .wallet-import-option.legacy {
        order: 2;
    }
    .migration-tools-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 1rem;
    }
    @media (max-width: 850px), (max-height: 700px) {
        .unified-wallet-import-modal {
            width: calc(100% - 1rem);
            max-height: min(calc(100dvh - 6.5rem), 560px);
            padding: 0.65rem !important;
        }
        .unified-wallet-import-modal .wallet-import-option {
            padding: 0.55rem 0.7rem !important;
        }
        .unified-wallet-import-modal .desc,
        .unified-wallet-import-modal .vault-connect-help {
            font-size: 0.58rem !important;
            line-height: 1.35;
        }
        .unified-wallet-import-modal .input-glass {
            padding: 0.36rem 0.45rem !important;
            font-size: 0.66rem !important;
        }
        .migration-tools-grid {
            grid-template-columns: 1fr;
            gap: 0.75rem;
        }
    }
    .vault-connect-result {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .vault-connect-success {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: var(--color-primary);
        font-size: 0.7rem;
        font-weight: 700;
        letter-spacing: 0.5px;
    }
    .vault-connect-check {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 24px;
        height: 24px;
        border-radius: 50%;
        background: rgba(0, 255, 65, 0.15);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        font-size: 0.9rem;
    }
    .vault-connect-warn {
        background: rgba(255, 170, 0, 0.08);
        border: 1px solid rgba(255, 170, 0, 0.3);
        border-radius: 6px;
        padding: 0.6rem 0.8rem;
        color: #ffaa00;
        font-size: 0.6rem;
        line-height: 1.4;
    }
    .vault-connect-info {
        background: rgba(0, 200, 255, 0.06);
        border: 1px solid rgba(0, 200, 255, 0.25);
        border-radius: 6px;
        padding: 0.5rem 0.7rem;
        color: #66ccff;
        font-size: 0.6rem;
        line-height: 1.4;
    }

    /* Button variants used in vault flows */
    :global(.cyber-btn.ghost) {
        background: transparent;
        color: #aaa;
        border-color: rgba(255, 255, 255, 0.1);
    }
    :global(.cyber-btn.ghost):hover {
        background: rgba(255, 255, 255, 0.05);
        color: var(--color-primary);
    }
    :global(.cyber-btn.danger) {
        background: rgba(255, 85, 85, 0.08);
        color: #ff5555;
        border-color: rgba(255, 85, 85, 0.25);
    }
    :global(.cyber-btn.danger):hover {
        background: rgba(255, 85, 85, 0.15);
        color: #ff8888;
    }
    :global(.cyber-btn.ghost.danger) {
        background: transparent;
        color: #ff8888;
        border-color: rgba(255, 85, 85, 0.2);
    }
    :global(.cyber-btn.ghost.danger):hover {
        background: rgba(255, 85, 85, 0.08);
        color: #ff5555;
    }

    .toggle {
        display: inline-flex;
        align-items: center;
        gap: 0.35rem;
        cursor: pointer;
    }
    .toggle input[type="checkbox"] {
        accent-color: var(--color-primary);
    }

    .key-item.selected {
        background: rgba(0, 255, 65, 0.08);
        border-color: rgba(0, 255, 65, 0.2);
    }

    .key-list-scroll {
        max-height: 300px;
        overflow-y: auto;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
    }
    .key-list-controls {
        display: flex;
        justify-content: space-between;
        margin-bottom: 0.5rem;
    }
    .key-balance {
        text-align: right;
        flex-shrink: 0;
        margin-left: 1rem;
    }
</style>
