<script>
  import { onMount, createEventDispatcher } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { core } from "@tauri-apps/api";
  import { save, open, ask } from "@tauri-apps/plugin-dialog";
  import { systemHubSection } from "../stores/systemHub.js";
  import ToolsNetwork from "../tools/ToolsNetwork.svelte";
  import HelpHitbox from "../ui/HelpHitbox.svelte";
  import { APP_VERSION } from "../constants.js";

  let activeSection = $systemHubSection;
  $: {
    if (activeSection !== $systemHubSection) {
      $systemHubSection = activeSection;
    }
  }

  // Data tab state
  let dataFolderInfo = {
    path: "--",
    default_path: "--",
    using_custom_path: false,
    commander_settings_path: "--",
    bootstrap_path: "--",
    size_bytes: 0,
    size_display: "--",
    config_exists: false,
    wallet_exists: false,
    folder_exists: false,
    blocks_exists: false,
    chainstate_exists: false,
    debug_log_exists: false,
    lock_exists: false,
    bootstrap_error: null,
  };
  let dataLoading = false;

  async function loadDataInfo() {
    dataLoading = true;
    try {
      dataFolderInfo = await core.invoke("get_data_folder_info");
    } catch (err) {
      dispatchToast("Failed to load data info", "error");
    }
    dataLoading = false;
  }

  async function openDataDir() {
    try {
      await core.invoke("open_data_dir");
    } catch (err) {
      try {
        const config = await core.invoke("init_config");
        dispatchToast(`Path: ${config.data_dir}`, "info");
      } catch {
        dispatchToast("Failed to open folder", "error");
      }
    }
  }

  async function backupDataFolder() {
    try {
      const ts = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
      const filePath = await save({
        title: "Save Data Folder Backup",
        defaultPath: `hemp0x_data_backup_${ts}`,
      });
      if (!filePath) return;
      dispatchToast("Backing up data folder...", "info");
      await core.invoke("backup_data_folder_to", { path: filePath });
      dispatchToast(`Backup saved to: ${filePath}`, "success");
      loadDataInfo();
    } catch (err) {
      dispatchToast(`Backup failed: ${err}`, "error");
    }
  }

  let snapshotInstalling = false;
  let snapshotModalOpen = false;
  let snapshotFilePath = "";
  let snapshotInstallStartedAt = null;
  let snapshotProgressTimer = null;
  let snapshotProgressMessage = "";

  function formatElapsed(ms) {
    const totalSeconds = Math.max(0, Math.floor(ms / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    if (minutes <= 0) return `${seconds}s`;
    return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  }

  function stopSnapshotProgressTimer() {
    if (snapshotProgressTimer) {
      clearInterval(snapshotProgressTimer);
      snapshotProgressTimer = null;
    }
  }

  function startSnapshotProgressTimer() {
    stopSnapshotProgressTimer();
    snapshotInstallStartedAt = Date.now();
    snapshotProgressMessage = "Preparing snapshot install...";
    snapshotProgressTimer = setInterval(() => {
      const elapsed = formatElapsed(Date.now() - snapshotInstallStartedAt);
      snapshotProgressMessage = `Installing snapshot (${elapsed}). Keep Commander open.`;
      if (processingMessage.startsWith("Extracting snapshot")) {
        processingMessage = `Extracting snapshot (${elapsed}). Large archives can take several minutes.`;
      }
    }, 1000);
  }

  async function installSnapshot() {
    if (snapshotInstalling) return;
    const selected = await open({
      title: "Select Snapshot Archive",
      filters: [{ name: "7-Zip Archive", extensions: ["7z"] }],
      multiple: false,
    });
    if (!selected) return;
    snapshotFilePath = selected;
    snapshotModalOpen = true;
  }

  async function confirmSnapshotInstall() {
    snapshotModalOpen = false;
    snapshotInstalling = true;
    isProcessing = true;
    startSnapshotProgressTimer();
    try {
      processingMessage = "Stopping node...";
      try {
        await core.invoke("stop_node");
        for (let i = 0; i < 10; i++) {
          await new Promise((r) => setTimeout(r, 1000));
          try {
            await core.invoke("get_info");
            processingMessage = `Waiting for node to stop... (${10 - i}s)`;
          } catch {
            break;
          }
        }
        await new Promise((r) => setTimeout(r, 1000));
      } catch {}
      processingMessage = "Extracting snapshot (0s). Large archives can take several minutes.";
      snapshotProgressMessage = "Extracting snapshot. Keep Commander open.";
      const result = await core.invoke("extract_snapshot", { archivePath: snapshotFilePath });
      dispatchToast(result, "success");
      loadDataInfo();
      processingMessage = "Restarting node...";
      snapshotProgressMessage = "Snapshot installed. Restarting Core.";
      try {
        await core.invoke("start_node");
      } catch {
        dispatchToast("Snapshot installed. Please restart the node manually.", "info");
      }
    } catch (err) {
      dispatchToast(`Snapshot failed: ${err}`, "error");
    }
    stopSnapshotProgressTimer();
    snapshotInstallStartedAt = null;
    snapshotProgressMessage = "";
    snapshotInstalling = false;
    isProcessing = false;
  }

  function cancelSnapshotInstall() {
    snapshotModalOpen = false;
    snapshotFilePath = "";
  }

  // Data dir selection with optional move/copy
  let chooseDataDirInProgress = false;
  let dataDirModalOpen = false;
  let dataDirModalTarget = "";

  async function chooseDataFolder() {
    if (chooseDataDirInProgress) return;
    const selected = await open({ title: "Select Core Data Directory", directory: true, multiple: false });
    if (!selected) return;

    // If current data exists and has content, ask what to do
    if (dataFolderInfo.folder_exists && dataFolderInfo.size_bytes > 0) {
      dataDirModalTarget = selected;
      dataDirModalOpen = true;
      return;
    }

    // Fresh / empty — just set
    chooseDataDirInProgress = true;
    try {
      dataFolderInfo = await core.invoke("set_core_data_dir", { path: selected });
      dispatchToast("Core data directory set.", "success");
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
    chooseDataDirInProgress = false;
  }

  function cancelDataDirModal() {
    dataDirModalOpen = false;
    dataDirModalTarget = "";
  }

  async function dataDirAction(action) {
    const target = dataDirModalTarget;
    dataDirModalOpen = false;
    dataDirModalTarget = "";

    if (action === "point") {
      chooseDataDirInProgress = true;
      try {
        dataFolderInfo = await core.invoke("set_core_data_dir", { path: target });
        dispatchToast("Core data directory set to new empty folder.", "success");
      } catch (err) {
        dispatchToast(`Failed: ${err}`, "error");
      }
      chooseDataDirInProgress = false;
      return;
    }

    if (action === "copy") {
      isProcessing = true;
      try {
        processingMessage = "Stopping daemon...";
        await core.invoke("stop_node");
        for (let i = 0; i < 20; i++) {
          await new Promise((r) => setTimeout(r, 1000));
          try {
            const info = await core.invoke("get_data_folder_info");
            if (!info.lock_exists) break;
            processingMessage = `Waiting for daemon to stop... (${20 - i}s)`;
          } catch {
            break;
          }
        }
        await new Promise((r) => setTimeout(r, 1000));

        processingMessage = "Copying data to new location...";
        await core.invoke("copy_core_data_dir_to", { targetPath: target });

        processingMessage = "Activating new data directory...";
        dataFolderInfo = await core.invoke("set_core_data_dir", { path: target });

        processingMessage = "Restarting daemon...";
        try {
          await core.invoke("start_node");
          dispatchToast("Data copied and new directory activated. Old data was not deleted.", "success");
        } catch {
          dispatchToast("Data copied and activated. Please restart the daemon manually.", "info");
        }
      } catch (err) {
        dispatchToast(`Copy failed: ${err}`, "error");
      }
      isProcessing = false;
      chooseDataDirInProgress = false;
    }
  }

  async function resetToDefaultDir() {
    try {
      dataFolderInfo = await core.invoke("reset_core_data_dir");
      dispatchToast("Data directory reset to default.", "success");
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
  }

  // Config tab
  let configText = "";
  async function loadConfig(silent = false) {
    try {
      configText = await core.invoke("read_config");
      await hydrateGuidedControls();
      guidedPreview = null;
      previewToken = null;
      if (!silent) dispatchToast("Configuration Loaded", "success");
    } catch (err) {
      if (!silent) dispatchToast("Config missing or empty", "info");
    }
  }
  async function saveConfig() {
    try {
      await core.invoke("write_config", { contents: configText });
      dispatchToast("Configuration saved. Restart Core to activate raw-editor changes.", "success");
    } catch (err) {
      dispatchToast(`Failed to save config: ${err}`, "error");
    }
  }
  async function exportConfig() {
    try {
      const filePath = await core.invoke("dialog_write_text_file", {
        content: configText,
        defaultPath: "hemp.conf",
        title: "Export Hemp0x Configuration",
        filters: [["Hemp0x configuration", "conf"], ["All files", "*"]],
      });
      dispatchToast(`Configuration exported to ${filePath}`, "success");
    } catch (err) {
      if (!String(err).includes("No file selected")) {
        dispatchToast(`Configuration export failed: ${err}`, "error");
      }
    }
  }
  async function importConfig() {
    try {
      const imported = await core.invoke("dialog_read_text_file", {
        title: "Import Hemp0x Configuration",
        filters: [["Hemp0x configuration", "conf"], ["All files", "*"]],
      });
      if (!imported.trim()) {
        dispatchToast("The selected configuration is empty.", "error");
        return;
      }
      configText = imported;
      guidedPreset = "custom";
      invalidatePreview({ resetPreset: false });
      dispatchToast("Configuration loaded into the raw editor. Review it, then select Save Config.", "info");
    } catch (err) {
      if (!String(err).includes("No file selected")) {
        dispatchToast(`Configuration import failed: ${err}`, "error");
      }
    }
  }
  async function createDefaultConfig() {
    try {
      await core.invoke("create_default_config");
      dispatchToast("Default config created", "success");
      loadDataInfo();
      loadConfig(true);
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
  }
  let showConfHelp = false;
  function toggleConfHelp() { showConfHelp = !showConfHelp; }

  let configHelpSections = [];
  let configHelpLoading = false;
  async function loadConfigHelp() {
    if (configHelpLoading || configHelpSections.length > 0) return;
    configHelpLoading = true;
    try {
      configHelpSections = await core.invoke("get_config_help_reference");
    } catch {}
    configHelpLoading = false;
  }

  // Guided configuration state
  let guidedPreset = "custom";
  let guidedControls = {
    server: true,
    listen: true,
    daemon: false,
    dbcache: 4096,
    maxconnections: 125,
    prune: 0,
    txindex: false,
    addressindex: false,
    assetindex: false,
    timestampindex: false,
    spentindex: false,
    zmqpubrawtx: "",
    addnodeEntry: "",
    addnodeList: [],
  };

  let guidedPreview = null;
  let previewToken = null;
  let previewChanges = null;
  let guidedPreviewLoading = false;
  let guidedApplyLoading = false;
  let guidedDirtyKeys = new Set();

  const CORE_DEFAULTS = {
    server: true, listen: true, daemon: false,
    dbcache: 450, maxconnections: 125, prune: 0,
    txindex: false, addressindex: false, assetindex: false,
    timestampindex: false, spentindex: false,
  };

  async function hydrateGuidedControls() {
    try {
      const parsed = await core.invoke("parse_current_config");
      guidedControls.server = parseBoolConfig(parsed, "server", CORE_DEFAULTS.server);
      guidedControls.listen = parseBoolConfig(parsed, "listen", CORE_DEFAULTS.listen);
      guidedControls.daemon = parseBoolConfig(parsed, "daemon", CORE_DEFAULTS.daemon);
      guidedControls.dbcache = parseIntConfig(parsed, "dbcache", CORE_DEFAULTS.dbcache);
      guidedControls.maxconnections = parseIntConfig(parsed, "maxconnections", CORE_DEFAULTS.maxconnections);
      guidedControls.prune = parseIntConfig(parsed, "prune", CORE_DEFAULTS.prune);
      guidedControls.txindex = parseBoolConfig(parsed, "txindex", CORE_DEFAULTS.txindex);
      guidedControls.addressindex = parseBoolConfig(parsed, "addressindex", CORE_DEFAULTS.addressindex);
      guidedControls.assetindex = parseBoolConfig(parsed, "assetindex", CORE_DEFAULTS.assetindex);
      guidedControls.timestampindex = parseBoolConfig(parsed, "timestampindex", CORE_DEFAULTS.timestampindex);
      guidedControls.spentindex = parseBoolConfig(parsed, "spentindex", CORE_DEFAULTS.spentindex);
      guidedControls.zmqpubrawtx = parsed["zmqpubrawtx"] || "";

      // Extract addnode entries from the raw config text
      const nodes = [];
      for (const line of configText.split("\n")) {
        const trimmed = line.trim();
        if (trimmed.startsWith("addnode=")) {
          const val = trimmed.substring(8).trim();
          if (val && !nodes.includes(val)) nodes.push(val);
        }
      }
      guidedControls.addnodeList = nodes;
      guidedControls.addnodeEntry = "";
      guidedDirtyKeys = new Set();
    } catch {
      // Keep defaults
    }
  }

  function parseBoolConfig(parsed, key, def) {
    const v = parsed[key];
    if (v === undefined || v === null || v === "") return def;
    return v === "1" || v === "true" || v === "yes";
  }

  function parseIntConfig(parsed, key, def) {
    const v = parsed[key];
    if (v === undefined || v === null || v === "") return def;
    const n = parseInt(v, 10);
    return isNaN(n) ? def : n;
  }

  function invalidatePreview({ resetPreset = true } = {}) {
    guidedPreview = null;
    previewToken = null;
    previewChanges = null;
    if (resetPreset) guidedPreset = "custom";
  }

  function markGuidedDirty(key) {
    guidedDirtyKeys = new Set([...guidedDirtyKeys, key]);
    invalidatePreview();
  }

  function applyPreset(preset) {
    guidedPreset = preset;
    if (preset === "full") {
      guidedControls = {
        ...guidedControls,
        server: true,
        listen: true,
        daemon: false,
        dbcache: 4096,
        maxconnections: 125,
        prune: 0,
        txindex: true,
        addressindex: true,
        assetindex: true,
        timestampindex: true,
        spentindex: true,
        zmqpubrawtx: "",
      };
    } else if (preset === "pruned") {
      guidedControls = {
        ...guidedControls,
        server: true,
        listen: true,
        daemon: false,
        dbcache: 1024,
        maxconnections: 40,
        prune: 2048,
        txindex: false,
        addressindex: false,
        assetindex: false,
        timestampindex: false,
        spentindex: false,
        zmqpubrawtx: "",
      };
    }
    guidedDirtyKeys = new Set();
    invalidatePreview({ resetPreset: false });
  }

  function buildChangesMap() {
    const changes = {};
    const include = (key) => guidedPreset !== "custom" || guidedDirtyKeys.has(key);
    if (include("server")) changes.server = guidedControls.server ? "1" : "0";
    if (include("listen")) changes.listen = guidedControls.listen ? "1" : "0";
    if (include("daemon")) changes.daemon = guidedControls.daemon ? "1" : "0";
    if (include("dbcache")) changes.dbcache = String(guidedControls.dbcache);
    if (include("maxconnections")) changes.maxconnections = String(guidedControls.maxconnections);
    if (include("prune")) changes.prune = String(guidedControls.prune);
    if (include("txindex") || (include("prune") && guidedControls.prune > 0)) {
      changes.txindex = guidedControls.prune > 0 ? "0" : (guidedControls.txindex ? "1" : "0");
    }
    if (include("addressindex")) changes.addressindex = guidedControls.addressindex ? "1" : "0";
    if (include("assetindex")) changes.assetindex = guidedControls.assetindex ? "1" : "0";
    if (include("timestampindex")) changes.timestampindex = guidedControls.timestampindex ? "1" : "0";
    if (include("spentindex")) changes.spentindex = guidedControls.spentindex ? "1" : "0";
    if (include("zmqpubrawtx")) {
      changes.zmqpubrawtx = guidedControls.zmqpubrawtx.trim() || null;
    }
    if (include("addnode")) changes.addnode = guidedControls.addnodeList.join(",");
    return changes;
  }

  async function previewGuidedChanges() {
    guidedPreviewLoading = true;
    const changes = buildChangesMap();
    try {
      guidedPreview = await core.invoke("preview_config_changes", { changes });
      previewToken = guidedPreview.preview_token;
      previewChanges = changes;
    } catch (err) {
      dispatchToast(`Preview failed: ${err}`, "error");
      guidedPreview = null;
      previewToken = null;
      previewChanges = null;
    }
    guidedPreviewLoading = false;
  }

  async function applyGuidedConfig() {
    if (guidedApplyLoading) return;
    if (!previewToken || !previewChanges) return;
    const needsFullReindex = guidedPreview?.reindex_required?.length > 0;
    const needsChainstateReindex =
      !needsFullReindex && guidedPreview?.reindex_chainstate_required?.length > 0;
    const repairModeNeeded = needsFullReindex
      ? "reindex"
      : needsChainstateReindex
        ? "reindex-chainstate"
        : null;
    if (repairModeNeeded) {
      const confirmed = await ask(
        repairModeNeeded === "reindex"
          ? "These settings require a full reindex. Core may take hours or days to rebuild, and returning from a pruned node can require downloading the blockchain again. Back up your wallet and vault before continuing. Apply the configuration and start the reindex now?"
          : "These settings require a chainstate reindex. Core may be unavailable for an extended period while indexes rebuild. Back up your wallet and vault before continuing. Apply the configuration and start the reindex now?",
        { title: "Apply Configuration and Reindex", kind: "warning" },
      );
      if (!confirmed) return;
    }
    guidedApplyLoading = true;
    isProcessing = true;
    processingMessage = "Backing up and applying configuration...";
    try {
      const result = await core.invoke("apply_guided_config", {
        changes: previewChanges,
        previewToken,
      });
      dispatchToast("Configuration applied. Backup created.", "success");
      configText = await core.invoke("read_config");
      await hydrateGuidedControls();
      if (result.changes && result.changes.length === 0) {
        dispatchToast("No changes to apply.", "info");
      } else {
        await restartCoreAfterConfigChange(repairModeNeeded);
      }
    } catch (err) {
      dispatchToast(`Apply failed: ${err}`, "error");
    }
    isProcessing = false;
    guidedApplyLoading = false;
    previewToken = null;
    guidedPreview = null;
  }

  async function restartCoreAfterConfigChange(repairModeNeeded) {
    isProcessing = true;
    try {
      processingMessage = "Stopping Core...";
      try {
        await core.invoke("stop_node");
      } catch {
        // A stopped daemon needs no additional action before startup.
      }
      for (let i = 0; i < 30; i++) {
        await new Promise((resolve) => setTimeout(resolve, 1000));
        try {
          const info = await core.invoke("get_data_folder_info");
          if (!info.lock_exists) break;
          processingMessage = "Waiting for Core to stop...";
        } catch {
          break;
        }
      }
      if (repairModeNeeded) {
        processingMessage =
          repairModeNeeded === "reindex"
            ? "Scheduling full reindex..."
            : "Scheduling chainstate reindex...";
        await core.invoke("set_daemon_repair_mode", { mode: repairModeNeeded });
        repairMode = repairModeNeeded;
      }
      processingMessage = repairModeNeeded
        ? "Starting Core rebuild..."
        : "Restarting Core...";
      await core.invoke("start_node");
      if (repairModeNeeded) {
        repairActive = true;
        startRepairPolling();
        dispatchToast("Core rebuild started. Progress is available in System > Repair.", "success");
      } else {
        processingMessage = "Waiting for Core RPC...";
        const readiness = await core.invoke("wait_for_daemon_ready", {
          timeoutMs: 120000,
        });
        if (!readiness.ready) {
          throw new Error(readiness.rpc_error || "Core did not become ready.");
        }
        dispatchToast("Core restarted with the updated configuration.", "success");
      }
    } catch (err) {
      dispatchToast(`Configuration was saved, but Core restart failed: ${err}`, "error");
    } finally {
      isProcessing = false;
    }
  }

  function addAddnode() {
    const entry = guidedControls.addnodeEntry.trim();
    if (!entry) return;
    if (!guidedControls.addnodeList.includes(entry)) {
      guidedControls.addnodeList = [...guidedControls.addnodeList, entry];
    }
    guidedControls.addnodeEntry = "";
    markGuidedDirty("addnode");
  }

  function removeAddnode(entry) {
    guidedControls.addnodeList = guidedControls.addnodeList.filter(e => e !== entry);
    markGuidedDirty("addnode");
  }

  // Logs
  let logText = "";
  async function refreshLog(silent = false) {
    try {
      logText = await core.invoke("read_log", { lines: 500 });
      if (!silent) dispatchToast("Logs Refreshed", "success");
    } catch (err) {
      if (!silent) dispatchToast("Failed to read logs", "error");
    }
  }
  async function clearLog() {
    try {
      await core.invoke("truncate_log");
      logText = "";
      dispatchToast("Log Cleared", "success");
    } catch (e) {
      dispatchToast(`Failed: ${e}`, "error");
    }
  }
  function saveLog() {
    try {
      const blob = new Blob([logText], { type: "text/plain" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "hemp0x_debug.log";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      dispatchToast("Log Downloaded", "success");
    } catch (e) {
      dispatchToast("Save failed", "error");
    }
  }

  // Repair tab
  let repairMode = "none";
  let repairModeLoading = false;
  let repairConfirmOpen = false;
  let pendingRepairMode = "";
  let repairBackupConfirmed = false;

  // Repair progress monitor
  let repairStatus = null;
  let repairPollingInterval = null;
  let repairActive = false;

  async function loadRepairMode() {
    try {
      repairMode = await core.invoke("get_daemon_repair_mode");
    } catch {}
  }

  async function loadRepairStatus() {
    try {
      repairStatus = await core.invoke("get_daemon_repair_status");
      if (repairStatus && repairStatus.active) {
        repairActive = true;
        startRepairPolling();
      } else if (repairActive && repairStatus && (repairStatus.phase.includes("complete") || repairStatus.phase.includes("online"))) {
        repairActive = false;
        stopRepairPolling();
      }
    } catch {}
  }

  function startRepairPolling() {
    stopRepairPolling();
    repairPollingInterval = setInterval(() => {
      loadRepairStatus();
    }, 3000);
  }

  function stopRepairPolling() {
    if (repairPollingInterval) {
      clearInterval(repairPollingInterval);
      repairPollingInterval = null;
    }
  }

  function onRepairClick(mode) {
    if (mode === "none") {
      clearRepairMode();
      return;
    }
    if (repairActive) return;
    pendingRepairMode = mode;
    repairBackupConfirmed = false;
    repairConfirmOpen = true;
  }

  async function confirmRepair() {
    repairConfirmOpen = false;
    const mode = pendingRepairMode;
    pendingRepairMode = "";
    repairBackupConfirmed = false;
    isProcessing = true;

    try {
      processingMessage = "Stopping daemon...";
      await core.invoke("stop_node");
      for (let i = 0; i < 20; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        try {
          const info = await core.invoke("get_data_folder_info");
          if (!info.lock_exists) break;
          processingMessage = `Waiting for daemon to stop... (${20 - i}s)`;
        } catch {
          break;
        }
      }
      await new Promise((r) => setTimeout(r, 1000));

      processingMessage = "Setting repair mode...";
      await core.invoke("set_daemon_repair_mode", { mode });
      repairMode = mode;

      processingMessage = "Starting daemon with repair flag...";
      await core.invoke("start_node");

      repairActive = true;
      startRepairPolling();
      dispatchToast(`Repair mode ${mode} applied. Daemon is restarting.`, "success");
    } catch (err) {
      dispatchToast(`Repair failed: ${err}`, "error");
    }

    isProcessing = false;
  }

  function cancelRepair() {
    repairConfirmOpen = false;
    pendingRepairMode = "";
    repairBackupConfirmed = false;
  }

  async function clearRepairMode() {
    try {
      await core.invoke("clear_daemon_repair_mode");
      repairMode = "none";
      repairActive = false;
      stopRepairPolling();
      dispatchToast("Repair mode cleared.", "info");
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
  }

  async function cancelPendingRepair() {
    try {
      await core.invoke("clear_daemon_repair_mode");
      repairMode = "none";
      repairActive = false;
      repairStatus = null;
      stopRepairPolling();
      dispatchToast("Pending repair cancelled.", "info");
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
  }

  // External Core binary folder
  let coreBinaryDir = null;
  let coreBinaryMode = "bundled";
  let coreBinaryDirModalOpen = false;
  let coreBinaryDirSelecting = false;

  async function loadCoreBinaryDir() {
    try {
      coreBinaryDir = await core.invoke("get_core_binary_dir");
      coreBinaryMode = coreBinaryDir ? "external" : "bundled";
    } catch {}
  }

  async function selectCoreBinaryDir() {
    if (coreBinaryDirSelecting) return;
    const selected = await open({ title: "Select External Core Binary Folder", directory: true, multiple: false });
    if (!selected) return;
    coreBinaryDirSelecting = true;
    isProcessing = true;
    try {
      processingMessage = "Validating Core binary folder...";
      const status = await core.invoke("set_core_binary_dir", { path: selected });
      coreBinaryDir = selected;
      coreBinaryMode = "external";
      loadRuntimeStatus();
      if (status.daemon_exists && status.cli_exists) {
        const txNote = status.tx_exists ? "" : " hemp0x-tx was not found, but it is optional for Commander.";
        dispatchToast(`External Core folder set successfully.${txNote}`, "success");
      } else {
        dispatchToast("Required daemon or CLI binary was not found. Check folder contents.", "warning");
      }
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
    isProcessing = false;
    coreBinaryDirSelecting = false;
  }

  async function resetCoreBinaryDir() {
    isProcessing = true;
    try {
      processingMessage = "Resetting to bundled Core...";
      const status = await core.invoke("reset_core_binary_dir");
      coreBinaryDir = null;
      coreBinaryMode = "bundled";
      loadRuntimeStatus();
      dispatchToast("Reset to bundled Core binaries.", "success");
    } catch (err) {
      dispatchToast(`Failed: ${err}`, "error");
    }
    isProcessing = false;
  }

  // System / Overview
  let runtimeStatus = {
    daemon: { exists: false, path: "--", base_version: null },
    cli: { exists: false, path: "--", base_version: null },
    bundled_core_next_ready: false,
  };

  async function loadRuntimeStatus() {
    try {
      runtimeStatus = await core.invoke("get_runtime_status");
    } catch {
      runtimeStatus = {
        daemon: { exists: false, path: "--", base_version: null },
        cli: { exists: false, path: "--", base_version: null },
        bundled_core_next_ready: false,
      };
    }
  }

  async function extractBinaries() {
    try {
      const selected = await open({ title: "Select Folder for Binaries", directory: true, multiple: false });
      if (!selected) return;
      dispatchToast("Extracting binaries...", "info");
      const res = await core.invoke("extract_binaries", { targetDir: selected });
      dispatchToast(res, "success");
      loadRuntimeStatus();
    } catch (err) {
      dispatchToast(`Extraction Failed: ${err}`, "error");
    }
  }

  let daemonSettings = { auto_start_daemon_on_launch: false, keep_daemon_running_on_close: false, allow_non_bundled_core_next: false };

  async function loadDaemonSettings() {
    try {
      const settings = await core.invoke("load_app_settings");
      daemonSettings.auto_start_daemon_on_launch = settings.auto_start_daemon_on_launch;
      daemonSettings.keep_daemon_running_on_close = settings.keep_daemon_running_on_close;
      daemonSettings.allow_non_bundled_core_next = settings.allow_non_bundled_core_next;
    } catch {}
  }
  async function saveDaemonSetting(key, value) {
    daemonSettings[key] = value;
    try {
      const current = await core.invoke("load_app_settings");
      current[key] = value;
      await core.invoke("save_app_settings", { settings: current });
    } catch (e) {
      dispatchToast(`Failed to save setting: ${e}`, "error");
    }
  }

  const dispatch = createEventDispatcher();

  function dispatchToast(msg, type = "info", notify = true) {
    dispatch("toast", { msg, type, notify });
  }

  let isProcessing = false;
  let processingMessage = "Processing...";

  onMount(() => {
    loadConfig(true);
    refreshLog(true);
    loadDataInfo();
    loadRuntimeStatus();
    loadDaemonSettings();
    loadRepairMode();
    loadCoreBinaryDir();
    loadRepairStatus();
    return () => {
      stopRepairPolling();
    };
  });
</script>

<div class="system-hub" in:fade={{ duration: 200 }}>
  <header class="hub-header">
    <div class="hub-tabs">
      <nav class="hub-tab-list" aria-label="System sections">
        {#each ["overview", "data", "config", "network", "logs", "repair"] as tab}
          <button
            class="hub-tab"
            class:active={activeSection === tab}
            on:click={() => (activeSection = tab)}
          >
            {tab.toUpperCase()}
          </button>
        {/each}
      </nav>
      <HelpHitbox title="System Hub">
        <p><strong>Overview</strong> — App version, binary status, daemon settings, and Core software management.</p>
        <p><strong>Data</strong> — Manage the Core data directory. Commander settings live inside the active data folder. Use "Choose Data Folder" to change location. If data exists, you can copy it to the new location or start fresh.</p>
        <p><strong>Config</strong> — Edit hemp.conf directly. A default config can be created if none exists.</p>
        <p><strong>Network</strong> — Peer ban list, ping / port diagnostics, and network mode switching.</p>
        <p><strong>Logs</strong> — View and download the daemon debug.log.</p>
        <p><strong>Repair</strong> — Schedule a one-shot reindex or reindex-chainstate. Requires wallet backup confirmation. The daemon is stopped, the flag is set, and the daemon is restarted automatically. Progress is monitored in real-time.</p>
      </HelpHitbox>
    </div>
  </header>

  <div class="hub-body">
    {#key activeSection}
      <div class="hub-section" in:fade={{ duration: 150 }}>
        {#if activeSection === "overview"}
          <div class="sh-grid compact">
            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">APP VERSION</h3>
              </div>
              <div class="sh-card-body">
                <div class="sh-metric-row">
                  <div class="sh-metric">
                    <span class="sh-metric-label">COMMANDER</span>
                    <span class="sh-metric-value">{APP_VERSION}</span>
                  </div>
                  <div class="sh-metric">
                    <span class="sh-metric-label">HEMP0XD</span>
                    <span class="sh-metric-value" class:sh-ok={runtimeStatus.daemon.exists}>
                      {runtimeStatus.daemon.base_version || (runtimeStatus.daemon.exists ? "Found" : "Not Found")}
                    </span>
                  </div>
                  <div class="sh-metric">
                    <span class="sh-metric-label">HEMP0X-CLI</span>
                    <span class="sh-metric-value" class:sh-ok={runtimeStatus.cli.exists}>
                      {runtimeStatus.cli.base_version || (runtimeStatus.cli.exists ? "Found" : "Not Found")}
                    </span>
                  </div>
                </div>
                <div class="sh-divider"></div>
                <div class="sh-subtitle">BINARY STATUS</div>
                <div class="sh-list-row">
                  <span class="sh-mono">hemp0xd</span>
                  <span class="sh-badge" class:sh-badge-ok={runtimeStatus.daemon.exists}>{runtimeStatus.daemon.exists ? "FOUND" : "MISSING"}</span>
                </div>
                <div class="sh-list-row">
                  <span class="sh-mono">hemp0x-cli</span>
                  <span class="sh-badge" class:sh-badge-ok={runtimeStatus.cli.exists}>{runtimeStatus.cli.exists ? "FOUND" : "MISSING"}</span>
                </div>
              </div>
            </div>

            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">DAEMON SETTINGS</h3>
              </div>
              <div class="sh-card-body">
                <label class="sh-check-row">
                  <input type="checkbox" checked={daemonSettings.auto_start_daemon_on_launch} on:change={(e) => saveDaemonSetting('auto_start_daemon_on_launch', e.target.checked)} />
                  <span>Auto-start daemon on launch</span>
                </label>
                <label class="sh-check-row">
                  <input type="checkbox" checked={daemonSettings.keep_daemon_running_on_close} on:change={(e) => saveDaemonSetting('keep_daemon_running_on_close', e.target.checked)} />
                  <span>Keep daemon running when Commander closes</span>
                </label>
                <label class="sh-check-row">
                  <input type="checkbox" checked={daemonSettings.allow_non_bundled_core_next} on:change={(e) => saveDaemonSetting('allow_non_bundled_core_next', e.target.checked)} />
                  <span>Allow non-bundled Core Next builds (advanced)</span>
                </label>
              </div>
            </div>

            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">CORE SOFTWARE</h3>
              </div>
              <div class="sh-card-body">
                {#if runtimeStatus.daemon.path}
                  <div class="sh-path-list tight">
                    <div class="sh-path-item">
                      <span class="sh-path-label">DAEMON</span>
                      <span class="sh-path-value sh-mono" title={runtimeStatus.daemon.path}>{runtimeStatus.daemon.path}</span>
                    </div>
                    <div class="sh-path-item">
                      <span class="sh-path-label">CLI</span>
                      <span class="sh-path-value sh-mono" title={runtimeStatus.cli.path}>{runtimeStatus.cli.path}</span>
                    </div>
                  </div>
                {/if}
                <div class="sh-subtitle" style="margin-bottom:0.5rem;">
                  BINARY MODE:
                  {#if coreBinaryMode === "external"}
                    <span class="sh-badge sh-badge-warn" style="margin-left:0.5rem;">EXTERNAL FOLDER</span>
                  {:else}
                    <span class="sh-badge sh-badge-ok" style="margin-left:0.5rem;">BUNDLED/DEFAULT</span>
                  {/if}
                </div>
                {#if coreBinaryMode === "external" && coreBinaryDir}
                  <div class="sh-path-list tight" style="margin-bottom: 0.75rem;">
                    <div class="sh-path-item">
                      <span class="sh-path-label">EXTERNAL</span>
                      <span class="sh-path-value sh-mono" title={coreBinaryDir}>{coreBinaryDir}</span>
                    </div>
                  </div>
                {/if}
                <div class="sh-action-row wrap">
                  <button class="sh-btn" on:click={extractBinaries}>EXTRACT BINARIES</button>
                  <button class="sh-btn" on:click={selectCoreBinaryDir} disabled={coreBinaryDirSelecting || repairActive}>
                    {coreBinaryDirSelecting ? "SELECTING..." : "USE EXTERNAL CORE FOLDER"}
                  </button>
                  <button class="sh-btn sh-btn-ghost" on:click={resetCoreBinaryDir} disabled={repairActive || coreBinaryMode !== "external"}>USE BUNDLED CORE</button>
                </div>
                {#if coreBinaryMode === "external"}
                  <div class="sh-divider"></div>
                  <p class="sh-help-text" style="font-size: 0.75rem; color: #888;">
                    <strong>Advanced:</strong> Commander uses the selected folder for daemon and CLI calls.
                    External Core must be trusted. Use "Use Bundled Core" to return to the packaged binaries.
                  </p>
                {/if}
                <div class="sh-divider"></div>
                <div class="sh-subtitle">CHECKSUM VERIFICATION</div>
                <div class="sh-coming-soon">Coming Soon: Verify app integrity with Hemp token signed releases</div>
              </div>
            </div>
          </div>

        {:else if activeSection === "data"}
          <div class="sh-grid">
            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">DATA FOLDER</h3>
                {#if dataLoading}
                  <span class="sh-spinner">Refreshing...</span>
                {/if}
              </div>
              <div class="sh-card-body">
                {#if dataFolderInfo.using_custom_path}
                  <div class="sh-custom-banner">
                    <span class="sh-custom-dot"></span>
                    CUSTOM DATA DIRECTORY ACTIVE
                  </div>
                {/if}
                {#if dataFolderInfo.bootstrap_error}
                  <div class="sh-custom-banner" style="background: rgba(255,68,68,0.08); color: #ff6666;">
                    <span class="sh-custom-dot" style="background: #ff6666; box-shadow: 0 0 6px #ff6666;"></span>
                    BOOTSTRAP ERROR: {dataFolderInfo.bootstrap_error}
                  </div>
                {/if}
                <div class="sh-path-list">
                  <div class="sh-path-item">
                    <span class="sh-path-label">ACTIVE</span>
                    <span class="sh-path-value sh-mono" title={dataFolderInfo.path}>{dataFolderInfo.path}</span>
                  </div>
                  <div class="sh-path-item">
                    <span class="sh-path-label">DEFAULT</span>
                    <span class="sh-path-value sh-mono" title={dataFolderInfo.default_path}>{dataFolderInfo.default_path}</span>
                  </div>
                  <div class="sh-path-item">
                    <span class="sh-path-label">SETTINGS</span>
                    <span class="sh-path-value sh-mono" title={dataFolderInfo.commander_settings_path}>{dataFolderInfo.commander_settings_path}</span>
                  </div>
                  {#if dataFolderInfo.using_custom_path}
                    <div class="sh-path-item">
                      <span class="sh-path-label">BOOTSTRAP</span>
                      <span class="sh-path-value sh-mono" title={dataFolderInfo.bootstrap_path}>{dataFolderInfo.bootstrap_path}</span>
                    </div>
                  {/if}
                </div>

                <div class="sh-status-bar">
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.folder_exists}>
                    <span class="sh-status-key">FOLDER</span>
                    <span class="sh-status-val">{dataFolderInfo.folder_exists ? "EXISTS" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.config_exists}>
                    <span class="sh-status-key">CONFIG</span>
                    <span class="sh-status-val">{dataFolderInfo.config_exists ? "FOUND" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.wallet_exists}>
                    <span class="sh-status-key">WALLET</span>
                    <span class="sh-status-val">{dataFolderInfo.wallet_exists ? "FOUND" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.blocks_exists}>
                    <span class="sh-status-key">BLOCKS</span>
                    <span class="sh-status-val">{dataFolderInfo.blocks_exists ? "FOUND" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.chainstate_exists}>
                    <span class="sh-status-key">CHAINSTATE</span>
                    <span class="sh-status-val">{dataFolderInfo.chainstate_exists ? "FOUND" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-ok={dataFolderInfo.debug_log_exists}>
                    <span class="sh-status-key">DEBUG.LOG</span>
                    <span class="sh-status-val">{dataFolderInfo.debug_log_exists ? "FOUND" : "MISSING"}</span>
                  </div>
                  <div class="sh-status-pill" class:sh-warn={dataFolderInfo.lock_exists}>
                    <span class="sh-status-key">.LOCK</span>
                    <span class="sh-status-val">{dataFolderInfo.lock_exists ? "PRESENT" : "NONE"}</span>
                  </div>
                  <div class="sh-status-pill">
                    <span class="sh-status-key">SIZE</span>
                    <span class="sh-status-val">{dataFolderInfo.size_display}</span>
                  </div>
                </div>

                <div class="sh-action-row wrap">
                  <button class="sh-btn" on:click={openDataDir}>OPEN FOLDER</button>
                  <button class="sh-btn" on:click={backupDataFolder}>BACKUP ALL</button>
                  <button class="sh-btn sh-btn-primary" on:click={installSnapshot} disabled={snapshotInstalling}>
                    {snapshotInstalling ? "INSTALLING..." : "INSTALL SNAPSHOT"}
                  </button>
                  <button class="sh-btn sh-btn-ghost" on:click={loadDataInfo}>REFRESH</button>
                </div>
              </div>
            </div>

            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">CORE DATA DIRECTORY</h3>
              </div>
              <div class="sh-card-body">
                <p class="sh-help-text">
                  The active data directory is where the blockchain, wallet, and Commander settings are stored.
                  Use <strong>Choose Data Folder</strong> to change the active location. If data already exists, you will be asked whether to copy it or start fresh.
                </p>
                <div class="sh-action-row wrap" style="margin-top: 0.75rem;">
                  <button class="sh-btn" on:click={chooseDataFolder} disabled={chooseDataDirInProgress || repairActive}>CHOOSE DATA FOLDER</button>
                  <button class="sh-btn sh-btn-ghost" on:click={resetToDefaultDir} disabled={!dataFolderInfo.using_custom_path || repairActive}>USE DEFAULT</button>
                </div>
              </div>
            </div>

            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">ABOUT DATA FOLDER</h3>
              </div>
              <div class="sh-card-body">
                <p class="sh-help-text">The <strong>data folder</strong> contains all Hemp0x blockchain data and wallet information.</p>
                <ul class="sh-bullet-list">
                  <li><strong>wallet.dat</strong> — Your wallet keys and transaction history. BACK THIS UP!</li>
                  <li><strong>hemp.conf</strong> — Node configuration file</li>
                  <li><strong>blocks/</strong> — Downloaded blockchain data</li>
                  <li><strong>chainstate/</strong> — Current UTXO set</li>
                  <li><strong>debug.log</strong> — Node debug log file</li>
                </ul>
                <div class="sh-divider"></div>
                <div class="sh-subtitle">COMMANDER SETTINGS</div>
                <p class="sh-help-text">
                  Commander app settings are stored inside the <strong>active data folder</strong> under <code>commander/app_settings.json</code>.
                  A small bootstrap file in the default data area remembers which folder is active, so Commander can find it on startup.
                </p>
                <p class="sh-danger-text">Never share your wallet.dat file with anyone.</p>
              </div>
            </div>
          </div>

        {:else if activeSection === "config"}
          <div class="sh-flex-col fill" style="gap: 0.75rem; overflow-y: auto; padding-right: 4px;">
            <!-- Guided Configuration Section -->
            <div class="sh-guided-config">
              <div class="sh-guided-header">
                <h3 class="sh-guided-title">NODE PRESET</h3>
                <span class="sh-guided-subtitle">Select a preset or customize individual settings below</span>
              </div>

              <div class="sh-preset-cards">
                <button
                  class="sh-preset-card"
                  class:sh-preset-active={guidedPreset === "full"}
                  on:click={() => applyPreset("full")}
                >
                  <div class="sh-preset-icon">FULL</div>
                  <div class="sh-preset-label">Full Feature Node</div>
                  <div class="sh-preset-desc">All Commander features: wallet, explorer, assets, snapshots, recovery</div>
                  <div class="sh-preset-tags">
                    <span class="sh-preset-tag sh-tag-ok">all indexes</span>
                    <span class="sh-preset-tag sh-tag-ok">full history</span>
                  </div>
                </button>

                <button
                  class="sh-preset-card"
                  class:sh-preset-active={guidedPreset === "pruned"}
                  on:click={() => applyPreset("pruned")}
                >
                  <div class="sh-preset-icon">SAVE</div>
                  <div class="sh-preset-label">Storage Saver</div>
                  <div class="sh-preset-desc">Lower disk use. No historical rescans. Reduced explorer detail.</div>
                  <div class="sh-preset-tags">
                    <span class="sh-preset-tag sh-tag-warn">pruned</span>
                    <span class="sh-preset-tag sh-tag-warn">no txindex</span>
                  </div>
                </button>

                <button
                  class="sh-preset-card"
                  class:sh-preset-active={guidedPreset === "custom"}
                  on:click={() => { guidedPreset = "custom"; invalidatePreview(); }}
                >
                  <div class="sh-preset-icon">CUSTOM</div>
                  <div class="sh-preset-label">Custom</div>
                  <div class="sh-preset-desc">Raw editor is the source of truth. Controls show a snapshot from last load.</div>
                  <div class="sh-preset-tags">
                    <span class="sh-preset-tag">raw editor</span>
                  </div>
                </button>
              </div>

              <!-- Guided Controls -->
              <div class="sh-guided-controls">
                <div class="sh-guided-section">
                  <h4 class="sh-guided-section-title">ESSENTIAL</h4>
                  <div class="sh-controls-grid">
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.server} on:change={() => markGuidedDirty("server")} />
                      <span class="sh-toggle-label">Server (RPC)</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.listen} on:change={() => markGuidedDirty("listen")} />
                      <span class="sh-toggle-label">Listen</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.daemon} on:change={() => markGuidedDirty("daemon")} />
                      <span class="sh-toggle-label">Daemon Mode</span>
                    </label>
                  </div>
                </div>

                <div class="sh-guided-section">
                  <h4 class="sh-guided-section-title">INDEXES</h4>
                  <div class="sh-controls-grid">
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.txindex} disabled={guidedControls.prune > 0} on:change={() => markGuidedDirty("txindex")} />
                      <span class="sh-toggle-label">txindex</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.addressindex} on:change={() => markGuidedDirty("addressindex")} />
                      <span class="sh-toggle-label">addressindex</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.assetindex} on:change={() => markGuidedDirty("assetindex")} />
                      <span class="sh-toggle-label">assetindex</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.timestampindex} on:change={() => markGuidedDirty("timestampindex")} />
                      <span class="sh-toggle-label">timestampindex</span>
                    </label>
                    <label class="sh-toggle-row">
                      <input type="checkbox" bind:checked={guidedControls.spentindex} on:change={() => markGuidedDirty("spentindex")} />
                      <span class="sh-toggle-label">spentindex</span>
                    </label>
                  </div>
                </div>

                <div class="sh-guided-section">
                  <h4 class="sh-guided-section-title">PERFORMANCE</h4>
                  <div class="sh-controls-grid">
                    <label class="sh-input-row">
                      <span class="sh-input-label">dbcache (MB)</span>
                      <input type="number" min="4" max="32768" step="128" bind:value={guidedControls.dbcache} on:change={() => markGuidedDirty("dbcache")} class="sh-num-input" />
                    </label>
                    <label class="sh-input-row">
                      <span class="sh-input-label">Max Connections</span>
                      <input type="number" min="1" max="1000" bind:value={guidedControls.maxconnections} on:change={() => markGuidedDirty("maxconnections")} class="sh-num-input" />
                    </label>
                  </div>
                </div>

                <div class="sh-guided-section">
                  <h4 class="sh-guided-section-title">PRUNE</h4>
                  <div class="sh-controls-grid">
                    <label class="sh-input-row">
                      <span class="sh-input-label">Prune Target (MiB)</span>
                      <input type="number" min="0" max="1048576" step="100" bind:value={guidedControls.prune} on:change={() => markGuidedDirty("prune")} class="sh-num-input" />
                    </label>
                  </div>
                  {#if guidedControls.prune > 0}
                    <div class="sh-guided-note sh-note-warn">
                      <strong>Prune mode</strong> is incompatible with txindex. Rescans cannot recover history outside retained blocks. Returning to a full node requires re-downloading the blockchain.
                    </div>
                  {/if}
                </div>

                <div class="sh-guided-section">
                  <h4 class="sh-guided-section-title">
                    ADVANCED
                  </h4>
                  <div class="sh-controls-grid">
                    <label class="sh-input-row">
                      <span class="sh-input-label">ZMQ Raw TX</span>
                      <input type="text" placeholder="tcp://127.0.0.1:28332" bind:value={guidedControls.zmqpubrawtx} on:input={() => markGuidedDirty("zmqpubrawtx")} class="sh-text-input" />
                    </label>
                  </div>
                  <div class="sh-addnode-section">
                    <div class="sh-input-row" style="flex: 1;">
                      <span class="sh-input-label">addnode</span>
                      <input type="text" placeholder="IP:port" bind:value={guidedControls.addnodeEntry} class="sh-text-input" style="flex: 1;" on:keydown={(e) => e.key === "Enter" && addAddnode()} />
                    </div>
                    <button class="sh-btn sh-btn-sm" on:click={addAddnode}>ADD</button>
                  </div>
                  {#if guidedControls.addnodeList.length > 0}
                    <div class="sh-addnode-list">
                      {#each guidedControls.addnodeList as node, i}
                        <div class="sh-addnode-tag">
                          <span class="sh-mono" style="font-size: 0.7rem;">{node}</span>
                          <button class="sh-addnode-remove" on:click={() => removeAddnode(node)}>&times;</button>
                        </div>
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>

              <!-- Validation & Pre-Apply Info -->
              {#if guidedPreview}
                <div class="sh-preview-panel">
                  {#if guidedPreview.validation_warnings && guidedPreview.validation_warnings.length > 0}
                    <div class="sh-preview-section sh-preview-warnings">
                      <h4 class="sh-preview-label">WARNINGS</h4>
                      {#each guidedPreview.validation_warnings as w}
                        <div class="sh-preview-warn-item">{w}</div>
                      {/each}
                    </div>
                  {/if}
                  {#if guidedPreview.validation_errors && guidedPreview.validation_errors.length > 0}
                    <div class="sh-preview-section sh-preview-errors">
                      <h4 class="sh-preview-label">ERRORS</h4>
                      {#each guidedPreview.validation_errors as e}
                        <div class="sh-preview-error-item">{e}</div>
                      {/each}
                    </div>
                  {/if}
                  {#if guidedPreview.changes && guidedPreview.changes.length > 0}
                    <div class="sh-preview-section">
                      <h4 class="sh-preview-label">CHANGES ({guidedPreview.changes.length})</h4>
                      <div class="sh-changes-list">
                        {#each guidedPreview.changes as change}
                          <div class="sh-change-item">
                            <span class="sh-change-key">{change.key}</span>
                            {#if change.action === "changed"}
                              <span class="sh-change-old">{change.old_value}</span>
                              <span class="sh-change-arrow">&rarr;</span>
                              <span class="sh-change-new">{change.new_value}</span>
                            {:else if change.action === "added"}
                              <span class="sh-change-action sh-change-added">+{change.new_value}</span>
                            {:else if change.action === "removed"}
                              <span class="sh-change-action sh-change-removed">-{change.old_value} (removed)</span>
                            {/if}
                          </div>
                        {/each}
                      </div>
                    </div>
                  {:else}
                    <div class="sh-preview-section">
                      <span class="sh-preview-label" style="color: #888;">No changes to apply</span>
                    </div>
                  {/if}
                  {#if guidedPreview.reindex_required && guidedPreview.reindex_required.length > 0}
                    <div class="sh-preview-section sh-preview-reindex">
                      <h4 class="sh-preview-label">REINDEX REQUIRED</h4>
                      {#each guidedPreview.reindex_required as r}
                        <div class="sh-preview-reindex-item">{r}</div>
                      {/each}
                      {#if guidedPreview.reindex_chainstate_required && guidedPreview.reindex_chainstate_required.length > 0}
                        <p class="sh-help-text" style="font-size: 0.7rem; margin-top: 0.35rem;">
                          Full reindex covers the chainstate/index rebuild too. Commander will ask for confirmation, then restart Core and begin one rebuild pass after applying.
                        </p>
                      {:else}
                        <p class="sh-help-text" style="font-size: 0.7rem; margin-top: 0.35rem;">
                          Commander will ask for confirmation, then restart Core and begin the reindex after applying.
                        </p>
                      {/if}
                    </div>
                  {/if}
                  {#if (!guidedPreview.reindex_required || guidedPreview.reindex_required.length === 0) && guidedPreview.reindex_chainstate_required && guidedPreview.reindex_chainstate_required.length > 0}
                    <div class="sh-preview-section sh-preview-chainstate">
                      <h4 class="sh-preview-label">REINDEX-CHAINSTATE REQUIRED</h4>
                      {#each guidedPreview.reindex_chainstate_required as r}
                        <div class="sh-preview-chainstate-item">{r}</div>
                      {/each}
                      <p class="sh-help-text" style="font-size: 0.7rem; margin-top: 0.35rem;">
                        Commander will ask for confirmation, then restart Core and begin the chainstate rebuild after applying.
                      </p>
                    </div>
                  {/if}
                  {#if guidedPreview.restart_required}
                    <div class="sh-preview-section sh-preview-restart">
                      <span class="sh-preview-label">RESTART REQUIRED</span>
                      <span class="sh-help-text" style="font-size: 0.7rem;">Changes require a daemon restart to take effect.</span>
                    </div>
                  {/if}
                </div>
              {/if}

              <!-- Action Buttons -->
              <div class="sh-guided-actions">
                <button class="sh-btn" on:click={previewGuidedChanges} disabled={guidedPreviewLoading}>
                  {guidedPreviewLoading ? "PREVIEWING..." : "PREVIEW CHANGES"}
                </button>
                <button
                  class="sh-btn sh-btn-primary"
                  on:click={applyGuidedConfig}
                  disabled={guidedApplyLoading || !previewToken || (guidedPreview && guidedPreview.validation_errors && guidedPreview.validation_errors.length > 0)}
                >
                  {guidedApplyLoading ? "APPLYING..." : "APPLY CONFIGURATION"}
                </button>
              </div>
            </div>

            <!-- Raw Editor -->
            <div class="sh-guided-header">
              <h3 class="sh-guided-title">RAW EDITOR</h3>
              <span class="sh-guided-subtitle">Advanced users: edit hemp.conf directly. Custom preset selected automatically.</span>
            </div>
            <div class="sh-editor-wrap config-editor-wrap">
              <textarea class="sh-editor sh-mono" bind:value={configText} on:input={invalidatePreview}></textarea>
            </div>
            <div class="sh-action-row right">
              <button class="sh-btn sh-btn-ghost" on:click={() => { toggleConfHelp(); loadConfigHelp(); }}>HELP</button>
              <button class="sh-btn sh-btn-ghost" on:click={importConfig}>IMPORT CONFIG</button>
              <button class="sh-btn sh-btn-ghost" on:click={exportConfig}>BACKUP CONFIG</button>
              <button class="sh-btn sh-btn-ghost" on:click={createDefaultConfig}>CREATE DEFAULT</button>
              <button class="sh-btn sh-btn-ghost" on:click={() => loadConfig(false)}>RELOAD</button>
              <button class="sh-btn" on:click={saveConfig}>SAVE CONFIG</button>
            </div>
          </div>

        {:else if activeSection === "network"}
          <ToolsNetwork isVisible={activeSection === "network"} on:toast on:help />

        {:else if activeSection === "logs"}
          <div class="sh-flex-col fill">
            <div class="sh-editor-wrap dark">
              <div class="sh-scanline"></div>
              <textarea class="sh-editor sh-mono green" readonly bind:value={logText}></textarea>
            </div>
            <div class="sh-action-row right">
              <button class="sh-btn sh-btn-ghost" on:click={clearLog}>CLEAR LOGS</button>
              <button class="sh-btn" on:click={saveLog}>SAVE LOG</button>
              <button class="sh-btn sh-btn-ghost" on:click={() => refreshLog(false)}>REFRESH</button>
            </div>
          </div>

        {:else if activeSection === "repair"}
          <div class="sh-grid compact">
            <div class="sh-card">
              <div class="sh-card-header">
                <h3 class="sh-card-title">DAEMON REPAIR MODE</h3>
              </div>
              <div class="sh-card-body">
                <p class="sh-help-text">
                  Repair flags are <strong>one-shot</strong>: the daemon is stopped, the flag is applied, and the daemon is restarted automatically. The flag is cleared after the next start.
                </p>

                {#if repairStatus && (repairStatus.active || repairStatus.mode)}
                  <div class="sh-repair-monitor">
                    <div class="sh-repair-monitor-header">
                      {#if repairStatus.active}
                        <span class="sh-repair-spinner"></span>
                      {:else}
                        <span class="sh-repair-dot"></span>
                      {/if}
                      <span class="sh-repair-phase">{repairStatus.phase}</span>
                    </div>
                    <div class="sh-repair-monitor-details">
                      {#if repairStatus.blocks != null && repairStatus.headers != null}
                        <div class="sh-repair-stat">
                          <span class="sh-repair-stat-label">BLOCKS</span>
                          <span class="sh-repair-stat-value">{repairStatus.blocks} / {repairStatus.headers}</span>
                        </div>
                      {/if}
                      {#if repairStatus.verification_progress != null}
                        <div class="sh-repair-stat">
                          <span class="sh-repair-stat-label">PROGRESS</span>
                          <span class="sh-repair-stat-value">{(repairStatus.verification_progress * 100).toFixed(2)}%</span>
                        </div>
                      {/if}
                      {#if repairStatus.rpc_online}
                        <div class="sh-repair-stat">
                          <span class="sh-repair-stat-label">RPC</span>
                          <span class="sh-repair-stat-value sh-ok">ONLINE</span>
                        </div>
                      {:else}
                        <div class="sh-repair-stat">
                          <span class="sh-repair-stat-label">RPC</span>
                          <span class="sh-repair-stat-value">OFFLINE</span>
                        </div>
                      {/if}
                      {#if repairStatus.log_hint}
                        <div class="sh-repair-stat sh-repair-log-hint">
                          <span class="sh-repair-stat-label">HINT</span>
                          <span class="sh-repair-stat-value">{repairStatus.log_hint}</span>
                        </div>
                      {/if}
                    </div>
                    {#if !repairStatus.active && repairStatus.rpc_online && repairStatus.mode}
                      <div class="sh-repair-complete">
                        Repair process started successfully. Node is rebuilding/syncing. You can monitor progress here.
                      </div>
                    {/if}
                    {#if !repairStatus.active && !repairStatus.rpc_online && repairStatus.mode}
                      <div class="sh-action-row wrap" style="margin-top: 0.5rem;">
                        <button class="sh-btn sh-btn-ghost sh-btn-sm" on:click={cancelPendingRepair}>CANCEL PENDING REPAIR</button>
                      </div>
                    {/if}
                  </div>
                {/if}

                <div class="sh-repair-options" class:sh-repair-locked={repairActive}>
                  {#each [
                    { mode: "reindex-chainstate", label: "REINDEX CHAINSTATE", desc: "Faster repair for chainstate / txindex recovery. Recommended first try." },
                    { mode: "reindex", label: "FULL REINDEX", desc: "Rebuilds the entire chain index. Can take hours or days." },
                  ] as opt}
                    <button
                      class="sh-repair-btn"
                      class:sh-repair-active={repairMode === opt.mode}
                      on:click={() => onRepairClick(opt.mode)}
                      disabled={repairModeLoading || repairMode === opt.mode || repairActive}
                    >
                      <span class="sh-repair-label">{opt.label}</span>
                      <span class="sh-repair-desc">{opt.desc}</span>
                      {#if repairMode === opt.mode && !repairActive}
                        <span class="sh-repair-badge">SCHEDULED</span>
                      {/if}
                    </button>
                  {/each}
                </div>
                <div class="sh-divider"></div>
                <p class="sh-danger-text">Back up wallet.dat before any repair. The daemon will be stopped and restarted automatically.</p>
              </div>
            </div>
          </div>
        {/if}
      </div>
    {/key}
  </div>
</div>

<!-- SNAPSHOT MODAL -->
{#if snapshotModalOpen}
  <div class="modal-overlay" role="button" tabindex="0" on:click|self={cancelSnapshotInstall} on:keydown={(e) => e.key === "Escape" && cancelSnapshotInstall()}>
    <div class="modal-staged snapshot-modal" transition:fly={{ y: 20, duration: 200 }}>
      <div class="modal-header warning"><h3>INSTALL SNAPSHOT</h3></div>
      <div class="modal-body">
        <p class="warning-text">This will replace your <strong>blocks</strong> and <strong>chainstate</strong> folders.</p>
        <p class="highlight-safe">Your wallet.dat will NOT be affected.</p>
        <p class="desc">Make sure you have a backup before proceeding!</p>
        <div class="snapshot-file-info">
          <span class="file-label">FILE:</span>
          <span class="file-path mono">{snapshotFilePath.split(/[/\\]/).pop()}</span>
        </div>
      </div>
      <div class="modal-actions">
        <button class="cyber-btn primary" on:click={confirmSnapshotInstall}>INSTALL</button>
        <button class="cyber-btn ghost" on:click={cancelSnapshotInstall}>CANCEL</button>
      </div>
    </div>
  </div>
{/if}

<!-- DATA DIR MOVE MODAL -->
{#if dataDirModalOpen}
  <div class="modal-overlay" role="button" tabindex="0" on:click|self={cancelDataDirModal} on:keydown={(e) => e.key === "Escape" && cancelDataDirModal()}>
    <div class="modal-staged" transition:fly={{ y: 20, duration: 200 }}>
      <div class="modal-header"><h3>CHANGE DATA DIRECTORY</h3></div>
      <div class="modal-body">
        <p class="sh-help-text" style="margin-bottom: 0.75rem;">
          Data already exists at the current location. How would you like to handle the switch to:
        </p>
        <div class="sh-path-item" style="margin-bottom: 0.75rem;">
          <span class="sh-path-value sh-mono">{dataDirModalTarget}</span>
        </div>
        <div class="sh-action-row wrap" style="justify-content: center;">
          <button class="sh-btn" on:click={() => dataDirAction("copy")}>COPY DATA HERE</button>
          <button class="sh-btn sh-btn-ghost" on:click={() => dataDirAction("point")}>START FRESH</button>
          <button class="sh-btn sh-btn-ghost" on:click={cancelDataDirModal}>CANCEL</button>
        </div>
        <p class="sh-help-text" style="margin-top: 0.75rem; font-size: 0.7rem; color: #666;">
          <strong>Copy</strong> will stop the daemon, copy all files, then activate the new folder and restart.<br>
          <strong>Start fresh</strong> only points Commander to the new empty folder. Existing data is left untouched.
        </p>
      </div>
    </div>
  </div>
{/if}

<!-- REPAIR CONFIRM MODAL -->
{#if repairConfirmOpen}
  <div class="modal-overlay" role="button" tabindex="0" on:click|self={cancelRepair} on:keydown={(e) => e.key === "Escape" && cancelRepair()}>
    <div class="modal-staged" transition:fly={{ y: 20, duration: 200 }}>
      <div class="modal-header warning"><h3>CONFIRM REPAIR</h3></div>
      <div class="modal-body">
        <p class="warning-text">
          This will stop the daemon, apply <strong>{pendingRepairMode.toUpperCase()}</strong>, and restart automatically.
        </p>
        {#if pendingRepairMode === "reindex"}
          <p class="sh-danger-text">Full reindex can take hours or days depending on hardware.</p>
        {:else if pendingRepairMode === "reindex-chainstate"}
          <p class="sh-help-text">Reindex-chainstate is faster and usually sufficient for chainstate corruption or txindex issues.</p>
        {/if}
        <label class="sh-check-row sh-repair-backup-check">
          <input type="checkbox" bind:checked={repairBackupConfirmed} />
          <span>I have backed up <strong>wallet.dat</strong> or understand the risk of data loss.</span>
        </label>
      </div>
      <div class="modal-actions">
        <button class="cyber-btn primary" on:click={confirmRepair} disabled={!repairBackupConfirmed}>CONFIRM</button>
        <button class="cyber-btn ghost" on:click={cancelRepair}>CANCEL</button>
      </div>
    </div>
  </div>
{/if}

<!-- PROCESSING OVERLAY -->
{#if isProcessing}
  <div class="modal-overlay" style="z-index: 999999;">
    <div class="modal-frame" style="text-align:center; max-width: 400px;">
      <h3 class="neon-text" style="color:var(--color-primary); margin:0 0 1rem 0;">PLEASE WAIT</h3>
      <p style="color:#aaa; margin:0;">{processingMessage}</p>
      {#if snapshotInstalling}
        <p style="color:#888; font-size:0.75rem; margin-top:1rem; line-height:1.45;">
          {snapshotProgressMessage || "Installing snapshot. Keep Commander open."}
        </p>
        <p style="color:#666; font-size:0.7rem; margin-top:0.75rem; line-height:1.4;">
          Snapshot extraction can be slow on large archives. Commander will continue when the install finishes.
        </p>
      {:else}
        <p style="color:#666; font-size:0.75rem; margin-top:1.5rem; line-height:1.4;">App will respond once the command is done.</p>
      {/if}
    </div>
  </div>
{/if}

<!-- CONFIG HELP MODAL -->
{#if showConfHelp}
  <div class="modal-overlay" role="button" tabindex="0" on:click|self={toggleConfHelp} on:keydown={(e) => e.key === "Escape" && toggleConfHelp()}>
    <div class="modal-staged wide config-help-modal">
      <div class="modal-header">
        <h3>CONFIGURATION REFERENCE</h3>
        <button class="btn-close-x" on:click={toggleConfHelp}>X</button>
      </div>
      <div class="modal-body config-help-body">
        <p class="highlight-warning"><strong>CRITICAL FOR WINDOWS:</strong> Set <code>daemon=0</code>. Setting <code>daemon=1</code> is for headless Linux/VPS only and will prevent the GUI from connecting to the node.</p>
        {#if configHelpLoading}
          <p style="color:#888; text-align:center; padding:1rem;">Loading reference...</p>
        {:else if configHelpSections.length > 0}
          {#each configHelpSections as section}
            <div class="conf-help-section">
              <h4 class="conf-help-section-title">{section.title}</h4>
              {#each section.entries as entry}
                <div class="conf-help-entry">
                  <div class="conf-help-entry-header">
                    <code class="conf-help-key">{entry.key}</code>
                    <span class="conf-help-default">Default: {entry.default_value}</span>
                  </div>
                  <p class="conf-help-desc">{entry.description}</p>
                  <p class="conf-help-relevance"><strong>Commander:</strong> {entry.commander_relevance}</p>
                </div>
              {/each}
            </div>
          {/each}
          <div class="sh-divider"></div>
          <p class="sh-help-text" style="font-size:0.7rem; color:#777; text-align:center;">
            This reference covers the common node, wallet, network, index, storage, RPC, logging, and integration settings used with Commander. Advanced operators can still use the raw editor for uncommon Core options.
          </p>
        {:else}
          <p style="color:#888; text-align:center; padding:1rem;">
            Configuration reference is unavailable. Close and reopen this window to retry.
          </p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  /* === HUB SHELL === */
  .system-hub {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-rows: auto 1fr;
  }
  .hub-header {
    padding: 0;
    margin-bottom: 0.35rem;
  }
  .hub-tabs {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.22rem;
    border: 1px solid rgba(0, 255, 65, 0.1);
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.34);
  }
  .hub-tab-list {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    flex: 1 1 auto;
    min-width: 0;
    gap: 0.25rem;
  }
  .hub-tab {
    min-width: 0;
    padding: 0.38rem 0.65rem;
    overflow: hidden;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: rgba(255, 255, 255, 0.48);
    font-size: 0.68rem;
    letter-spacing: 0.75px;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
    transition: all 0.2s;
    font-weight: 600;
  }
  .hub-tab:hover {
    border-color: rgba(0, 255, 65, 0.16);
    background: rgba(0, 255, 65, 0.025);
    color: rgba(255, 255, 255, 0.78);
    box-shadow: none;
    transform: none;
  }
  .hub-tab.active {
    border-color: rgba(0, 255, 65, 0.32);
    background: rgba(0, 255, 65, 0.07);
    color: var(--color-primary);
  }
  .hub-body {
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    display: flex;
    flex-direction: column;
  }
  .hub-body > div {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .hub-section {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  @media (max-width: 620px) {
    .hub-tab-list {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  /* === GRID / CARDS === */
  .sh-grid {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 0.5rem 0;
  }
  .sh-grid.compact { gap: 0.75rem; }
  .sh-card {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 8px;
    overflow: hidden;
  }
  .sh-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    background: rgba(0, 255, 65, 0.04);
    border-bottom: 1px solid rgba(0, 255, 65, 0.1);
  }
  .sh-card-title {
    font-size: 0.85rem;
    color: var(--color-primary);
    margin: 0;
    letter-spacing: 2px;
    font-weight: 700;
  }
  .sh-card-body {
    padding: 1rem;
  }
  .sh-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.06);
    margin: 0.75rem 0;
  }
  .sh-subtitle {
    color: #666;
    font-size: 0.7rem;
    letter-spacing: 1px;
    margin-bottom: 0.6rem;
    text-transform: uppercase;
  }

  /* === METRICS === */
  .sh-metric-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
  }
  .sh-metric {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    text-align: center;
  }
  .sh-metric-label {
    color: #555;
    font-size: 0.65rem;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .sh-metric-value {
    color: #888;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .sh-ok { color: var(--color-primary); }

  /* === LIST ROWS / BADGES === */
  .sh-list-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 4px;
    margin-bottom: 0.35rem;
    font-size: 0.85rem;
  }
  .sh-badge {
    font-size: 0.7rem;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    background: rgba(255, 68, 68, 0.15);
    color: #ff6666;
    font-weight: 600;
    letter-spacing: 0.5px;
  }
  .sh-badge-ok {
    background: rgba(0, 255, 65, 0.1);
    color: var(--color-primary);
  }

  /* === PATHS === */
  .sh-path-list { display: flex; flex-direction: column; gap: 0.4rem; margin-bottom: 0.75rem; }
  .sh-path-list.tight { gap: 0.3rem; margin-bottom: 0.5rem; }
  .sh-path-item {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: rgba(0, 0, 0, 0.45);
    border: 1px solid rgba(0, 255, 65, 0.08);
    border-radius: 6px;
  }
  .sh-path-label {
    color: #666;
    font-size: 0.65rem;
    letter-spacing: 1px;
    flex-shrink: 0;
    min-width: 4.5rem;
  }
  .sh-path-value {
    color: var(--color-primary);
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* === STATUS PILLS === */
  .sh-status-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }
  .sh-status-pill {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.6rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    font-size: 0.75rem;
  }
  .sh-status-pill.sh-ok { border-color: rgba(0, 255, 65, 0.25); }
  .sh-status-pill.sh-warn { border-color: rgba(255, 165, 0, 0.25); }
  .sh-status-key {
    color: #555;
    font-size: 0.6rem;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .sh-status-val {
    color: #888;
    font-weight: 600;
  }
  .sh-status-pill.sh-ok .sh-status-val { color: var(--color-primary); }
  .sh-status-pill.sh-warn .sh-status-val { color: #ffa500; }

  /* === CUSTOM BANNER === */
  .sh-custom-banner {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(0, 255, 65, 0.08);
    color: var(--color-primary);
    padding: 0.3rem 0.75rem;
    border-radius: 4px;
    font-size: 0.7rem;
    letter-spacing: 1px;
    font-weight: 700;
    margin-bottom: 0.75rem;
  }
  .sh-custom-dot {
    width: 6px;
    height: 6px;
    background: var(--color-primary);
    border-radius: 50%;
    box-shadow: 0 0 6px var(--color-primary);
  }

  /* === BUTTONS === */
  .sh-btn {
    background: rgba(0, 255, 65, 0.05);
    border: 1px solid var(--color-primary);
    color: var(--color-primary);
    padding: 0.6rem 1.2rem;
    letter-spacing: 1px;
    font-weight: 700;
    transition: all 0.2s;
    cursor: pointer;
    text-transform: uppercase;
    font-size: 0.75rem;
    white-space: nowrap;
    font-family: var(--font-mono);
  }
  .sh-btn:hover:not(:disabled) {
    background: var(--color-primary);
    color: #000;
    box-shadow: 0 0 10px rgba(0, 255, 65, 0.22);
  }
  .sh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    border-color: #555;
    color: #555;
    background: transparent;
    box-shadow: none;
  }
  .sh-btn-ghost {
    border-color: rgba(255, 255, 255, 0.2);
    color: #aaa;
    background: transparent;
  }
  .sh-btn-ghost:hover:not(:disabled) {
    border-color: #fff;
    color: #fff;
    box-shadow: none;
    background: rgba(255, 255, 255, 0.05);
  }
  .sh-btn-primary {
    background: rgba(0, 255, 65, 0.1);
  }
  .sh-btn-sm {
    padding: 0.35rem 0.75rem;
    font-size: 0.65rem;
  }

  /* === ACTION ROW === */
  .sh-action-row {
    display: flex;
    gap: 0.6rem;
    align-items: center;
  }
  .sh-action-row.wrap { flex-wrap: wrap; }
  .sh-action-row.right {
    justify-content: flex-end;
    flex-wrap: wrap;
    padding: 0.5rem 0;
  }

  /* === HELP / DANGER TEXT === */
  .sh-help-text {
    color: #aaa;
    font-size: 0.8rem;
    line-height: 1.6;
    margin: 0;
  }
  .sh-help-text strong { color: #fff; }
  .sh-help-text code {
    font-family: var(--font-mono);
    background: rgba(0, 255, 65, 0.08);
    padding: 0.05rem 0.25rem;
    border-radius: 3px;
    color: #ccc;
  }
  .sh-danger-text {
    color: #ff6666;
    font-weight: 700;
    font-size: 0.8rem;
    margin-top: 0.5rem;
  }

  /* === BULLET LIST === */
  .sh-bullet-list {
    margin: 0.6rem 0;
    padding-left: 1.25rem;
    color: #aaa;
    font-size: 0.8rem;
    line-height: 1.6;
  }
  .sh-bullet-list li { margin: 0.3rem 0; }
  .sh-bullet-list li strong { color: #fff; }

  /* === COMING SOON === */
  .sh-coming-soon {
    color: #666;
    font-size: 0.8rem;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px dashed rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    text-align: center;
  }

  /* === CHECK ROW === */
  .sh-check-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    color: #aaa;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0.3rem 0;
  }
  .sh-check-row input {
    width: 14px;
    height: 14px;
    accent-color: var(--color-primary);
    flex-shrink: 0;
  }
  .sh-check-row:hover { color: #fff; }

  /* === EDITOR / LOGS === */
  .sh-flex-col.fill {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .sh-editor-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: #000;
    border: 1px solid #333;
    border-radius: 6px;
    overflow: hidden;
    position: relative;
  }
  .sh-editor-wrap.dark { border-color: #222; }
  .sh-scanline {
    position: absolute;
    top: 0; left: 0; width: 100%; height: 100%;
    background: linear-gradient(to bottom, rgba(255,255,255,0), rgba(255,255,255,0) 50%, rgba(0,0,0,0.1) 50%, rgba(0,0,0,0.1));
    background-size: 100% 4px;
    pointer-events: none;
    z-index: 5;
    opacity: 0.3;
  }
  .sh-editor {
    flex: 1;
    width: 100%;
    height: 100%;
    background: #000;
    color: #fff;
    border: none;
    resize: none;
    font-size: 0.85rem;
    padding: 0.75rem;
    outline: none;
    overflow-y: auto;
    display: block;
    box-sizing: border-box;
    line-height: 1.5;
  }
  .sh-editor.green { color: #0f0; }
  .sh-editor::-webkit-scrollbar { width: 8px; }
  .sh-editor::-webkit-scrollbar-track { background: #111; }
  .sh-editor::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; border: 1px solid #444; }
  .sh-editor::-webkit-scrollbar-thumb:hover { background: var(--color-primary); }

  /* === REPAIR === */
  .sh-repair-options {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.6rem;
    margin-top: 0.75rem;
  }
  .sh-repair-btn {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    align-items: flex-start;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    color: #aaa;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
  }
  .sh-repair-btn:hover:not(:disabled) {
    border-color: rgba(0, 255, 65, 0.3);
    background: rgba(0, 255, 65, 0.04);
  }
  .sh-repair-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .sh-repair-active {
    border-color: var(--color-primary) !important;
    background: rgba(0, 255, 65, 0.08) !important;
  }
  .sh-repair-label {
    font-size: 0.8rem;
    font-weight: 700;
    color: #fff;
    letter-spacing: 1px;
  }
  .sh-repair-desc {
    font-size: 0.7rem;
    color: #777;
  }
  .sh-repair-active .sh-repair-desc { color: #aaa; }
  .sh-repair-badge {
    margin-top: 0.3rem;
    font-size: 0.6rem;
    padding: 0.15rem 0.4rem;
    background: var(--color-primary);
    color: #000;
    border-radius: 3px;
    font-weight: 700;
    letter-spacing: 1px;
  }
  .sh-repair-locked {
    opacity: 0.4;
    pointer-events: none;
  }
  .sh-repair-monitor {
    background: rgba(0, 255, 65, 0.04);
    border: 1px solid rgba(0, 255, 65, 0.2);
    border-radius: 6px;
    padding: 0.75rem;
    margin-bottom: 0.75rem;
  }
  .sh-repair-monitor-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  .sh-repair-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(0, 255, 65, 0.2);
    border-top-color: var(--color-primary);
    border-radius: 50%;
    animation: sh-spin 0.8s linear infinite;
  }
  .sh-repair-dot {
    width: 10px;
    height: 10px;
    background: #ffa500;
    border-radius: 50%;
    box-shadow: 0 0 6px #ffa500;
  }
  @keyframes sh-spin {
    to { transform: rotate(360deg); }
  }
  .sh-repair-phase {
    color: var(--color-primary);
    font-size: 0.85rem;
    font-weight: 600;
  }
  .sh-repair-monitor-details {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .sh-repair-stat {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    padding: 0.3rem 0.5rem;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 4px;
    font-size: 0.75rem;
  }
  .sh-repair-stat-label {
    color: #555;
    font-size: 0.6rem;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .sh-repair-stat-value {
    color: #aaa;
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .sh-repair-stat-value.sh-ok { color: var(--color-primary); }
  .sh-repair-log-hint {
    flex-basis: 100%;
  }
  .sh-repair-log-hint .sh-repair-stat-value {
    color: #888;
    font-family: inherit;
    font-size: 0.7rem;
  }
  .sh-repair-complete {
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: rgba(0, 255, 65, 0.08);
    border-radius: 4px;
    color: var(--color-primary);
    font-size: 0.8rem;
    font-weight: 600;
  }
  .sh-repair-backup-check {
    margin-top: 0.75rem;
    padding: 0.5rem;
    background: rgba(255, 68, 68, 0.06);
    border-radius: 4px;
    border: 1px solid rgba(255, 68, 68, 0.15);
  }
  .sh-repair-backup-check input {
    accent-color: #ff6666;
  }
  .sh-badge-warn {
    background: rgba(255, 165, 0, 0.15);
    color: #ffa500;
  }

  /* === UTILITIES === */
  .sh-mono { font-family: "Consolas", monospace; }
  .sh-spinner { font-size: 0.65rem; color: #666; animation: sh-pulse 1.5s infinite; }
  @keyframes sh-pulse { 0%, 100% { opacity: 0.4; } 50% { opacity: 1; } }

  /* === MODAL OVERRIDES === */
  .snapshot-modal { max-width: 450px; }
  .highlight-warning {
    color: #ff5555;
    background: rgba(255, 68, 68, 0.1);
    border: 1px solid #ff5555;
    padding: 0.8rem;
    border-radius: 4px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }
  /* === GUIDED CONFIG STYLES === */
  .sh-guided-config {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 8px;
    padding: 0.85rem;
    flex-shrink: 0;
  }
  .sh-guided-header {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    margin-bottom: 0.6rem;
  }
  .sh-guided-title {
    font-size: 0.85rem;
    color: var(--color-primary);
    margin: 0;
    letter-spacing: 2px;
    font-weight: 700;
  }
  .sh-guided-subtitle {
    font-size: 0.65rem;
    color: #666;
    letter-spacing: 0.5px;
  }

  /* PRESET CARDS */
  .sh-preset-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 0.6rem;
    margin-bottom: 0.85rem;
  }
  .sh-preset-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    color: #888;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
    font-family: inherit;
    font-size: inherit;
  }
  .sh-preset-card:hover {
    border-color: rgba(0, 255, 65, 0.2);
    background: rgba(0, 255, 65, 0.03);
    color: #aaa;
  }
  .sh-preset-active {
    border-color: rgba(0, 255, 65, 0.35);
    background: rgba(0, 255, 65, 0.06);
    color: #fff;
  }
  .sh-preset-active .sh-preset-label {
    color: var(--color-primary);
  }
  .sh-preset-icon {
    width: fit-content;
    padding: 0.12rem 0.35rem;
    border: 1px solid rgba(0, 255, 65, 0.18);
    border-radius: 3px;
    color: var(--color-primary);
    font-size: 0.55rem;
    font-weight: 700;
    letter-spacing: 1px;
  }
  .sh-preset-label {
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.5px;
  }
  .sh-preset-desc {
    font-size: 0.67rem;
    color: #666;
    line-height: 1.4;
  }
  .sh-preset-active .sh-preset-desc {
    color: #999;
  }
  .sh-preset-tags {
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
    margin-top: 0.2rem;
  }
  .sh-preset-tag {
    font-size: 0.55rem;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    font-weight: 600;
    letter-spacing: 0.5px;
  }
  .sh-tag-ok {
    background: rgba(0, 255, 65, 0.12);
    color: var(--color-primary);
    border: 1px solid rgba(0, 255, 65, 0.2);
  }
  .sh-tag-warn {
    background: rgba(255, 165, 0, 0.1);
    color: #ffa500;
    border: 1px solid rgba(255, 165, 0, 0.2);
  }

  /* GUIDED CONTROLS */
  .sh-guided-controls {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-bottom: 0.75rem;
  }
  .sh-guided-section {
    padding: 0.5rem 0.6rem;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 5px;
  }
  .sh-guided-section-title {
    color: #555;
    font-size: 0.62rem;
    letter-spacing: 1.5px;
    margin: 0 0 0.4rem 0;
    text-transform: uppercase;
  }
  .sh-controls-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 0.4rem;
  }

  /* TOGGLE ROW */
  .sh-toggle-row {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    padding: 0.35rem 0.5rem;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 4px;
    cursor: pointer;
    gap: 0.5rem;
  }
  .sh-toggle-label {
    color: #aaa;
    font-size: 0.72rem;
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sh-toggle-row input[type="checkbox"] {
    width: 13px;
    height: 13px;
    accent-color: var(--color-primary);
    flex-shrink: 0;
    cursor: pointer;
  }
  .config-editor-wrap {
    min-height: clamp(320px, 50vh, 680px);
    flex: 0 0 auto;
  }
  .config-editor-wrap .sh-editor {
    min-height: inherit;
    resize: vertical;
  }
  .config-help-modal {
    width: min(58rem, calc(100vw - 1rem));
    max-height: calc(100dvh - 1rem);
  }
  .config-help-body {
    max-height: none;
    overflow-x: hidden;
  }
  .config-help-body p,
  .config-help-body code {
    overflow-wrap: anywhere;
  }
  .sh-toggle-row input[type="checkbox"]:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
  .sh-toggle-row:hover {
    background: rgba(0, 255, 65, 0.03);
  }

  /* INPUT ROW */
  .sh-input-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.35rem 0.5rem;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 4px;
    gap: 0.5rem;
  }
  .sh-input-label {
    color: #aaa;
    font-size: 0.72rem;
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .sh-num-input {
    width: 80px;
    padding: 0.25rem 0.4rem;
    background: #111;
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 3px;
    color: var(--color-primary);
    font-size: 0.75rem;
    font-family: var(--font-mono, monospace);
    text-align: right;
    outline: none;
  }
  .sh-num-input:focus {
    border-color: var(--color-primary);
    box-shadow: 0 0 4px rgba(0, 255, 65, 0.15);
  }
  .sh-text-input {
    width: 180px;
    padding: 0.25rem 0.4rem;
    background: #111;
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 3px;
    color: #aaa;
    font-size: 0.72rem;
    font-family: var(--font-mono, monospace);
    outline: none;
  }
  .sh-text-input:focus {
    border-color: var(--color-primary);
    color: #fff;
  }
  .sh-text-input::placeholder {
    color: #444;
  }

  /* NOTES */
  .sh-guided-note {
    margin-top: 0.4rem;
    padding: 0.4rem 0.5rem;
    border-radius: 4px;
    font-size: 0.68rem;
    line-height: 1.4;
    color: #ccc;
  }
  .sh-note-warn {
    background: rgba(255, 165, 0, 0.06);
    border: 1px solid rgba(255, 165, 0, 0.15);
    color: #ddb060;
  }
  .sh-note-warn strong {
    color: #ffa500;
  }

  /* ADDNODE */
  .sh-addnode-section {
    display: flex;
    gap: 0.4rem;
    align-items: flex-end;
    margin-top: 0.4rem;
  }
  .sh-addnode-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-top: 0.4rem;
  }
  .sh-addnode-tag {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0.4rem;
    background: rgba(0, 255, 65, 0.06);
    border: 1px solid rgba(0, 255, 65, 0.12);
    border-radius: 4px;
    color: #999;
    font-size: 0.7rem;
  }
  .sh-addnode-remove {
    background: none;
    border: none;
    color: #ff6666;
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
    padding: 0;
  }

  /* PREVIEW PANEL */
  .sh-preview-panel {
    margin-bottom: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .sh-preview-section {
    padding: 0.5rem 0.6rem;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 5px;
  }
  .sh-preview-warnings {
    border-color: rgba(255, 165, 0, 0.2);
    background: rgba(255, 165, 0, 0.04);
  }
  .sh-preview-errors {
    border-color: rgba(255, 68, 68, 0.25);
    background: rgba(255, 68, 68, 0.05);
  }
  .sh-preview-reindex {
    border-color: rgba(255, 165, 0, 0.2);
    background: rgba(255, 165, 0, 0.04);
  }
  .sh-preview-chainstate {
    border-color: rgba(0, 255, 65, 0.15);
    background: rgba(0, 255, 65, 0.03);
  }
  .sh-preview-restart {
    border-color: rgba(0, 255, 65, 0.15);
    background: rgba(0, 255, 65, 0.03);
  }
  .sh-preview-label {
    color: var(--color-primary);
    font-size: 0.65rem;
    letter-spacing: 1px;
    font-weight: 700;
    margin: 0 0 0.3rem 0;
  }
  .sh-preview-warnings .sh-preview-label { color: #ffa500; }
  .sh-preview-errors .sh-preview-label { color: #ff6666; }
  .sh-preview-reindex .sh-preview-label { color: #ffa500; }
  .sh-preview-chainstate .sh-preview-label { color: var(--color-primary); }
  .sh-preview-restart .sh-preview-label { color: var(--color-primary); }

  .sh-preview-warn-item, .sh-preview-error-item {
    font-size: 0.7rem;
    color: #ddb060;
    padding: 0.2rem 0;
    line-height: 1.4;
  }
  .sh-preview-error-item { color: #ff8888; }
  .sh-preview-reindex-item, .sh-preview-chainstate-item {
    font-size: 0.7rem;
    color: #999;
    font-family: var(--font-mono, monospace);
    padding: 0.15rem 0;
  }

  /* CHANGES LIST */
  .sh-changes-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.4rem;
  }
  .sh-change-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.4rem;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 3px;
    font-size: 0.7rem;
    font-family: var(--font-mono, monospace);
  }
  .sh-change-key {
    color: #aaa;
    font-weight: 600;
    min-width: 100px;
  }
  .sh-change-old {
    color: #ff8888;
    text-decoration: line-through;
  }
  .sh-change-arrow {
    color: #666;
  }
  .sh-change-new {
    color: var(--color-primary);
    font-weight: 600;
  }
  .sh-change-action {
    font-weight: 600;
  }
  .sh-change-added {
    color: var(--color-primary);
  }
  .sh-change-removed {
    color: #ff8888;
  }

  /* GUIDED ACTIONS */
  .sh-guided-actions {
    display: flex;
    gap: 0.6rem;
    justify-content: flex-end;
  }

  /* CONFIG HELP MODAL UPDATES */
  .conf-help-section {
    margin-bottom: 0.75rem;
  }
  .conf-help-section-title {
    color: var(--color-primary);
    font-size: 0.82rem;
    margin: 0 0 0.35rem 0;
    letter-spacing: 1px;
    border-bottom: 1px solid rgba(0, 255, 65, 0.15);
    padding-bottom: 0.25rem;
  }
  .conf-help-entry {
    margin-bottom: 0.5rem;
    padding: 0.4rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }
  .conf-help-entry-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.2rem;
  }
  .conf-help-key {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    color: #aaffaa;
    background: rgba(0, 255, 65, 0.06);
    padding: 0.05rem 0.35rem;
    border-radius: 3px;
  }
  .conf-help-default {
    font-size: 0.62rem;
    color: #555;
    font-style: italic;
  }
  .conf-help-desc {
    font-size: 0.72rem;
    color: #aaa;
    margin: 0.15rem 0;
    line-height: 1.5;
  }
  .conf-help-relevance {
    font-size: 0.65rem;
    color: #777;
    margin: 0.15rem 0;
    line-height: 1.4;
  }
  .conf-help-relevance strong {
    color: #999;
  }
</style>
