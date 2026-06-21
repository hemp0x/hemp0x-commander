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
      processingMessage = "Extracting snapshot... this may take a few minutes";
      const result = await core.invoke("extract_snapshot", { archivePath: snapshotFilePath });
      dispatchToast(result, "success");
      loadDataInfo();
      processingMessage = "Restarting node...";
      try {
        await core.invoke("start_node");
      } catch {
        dispatchToast("Snapshot installed. Please restart the node manually.", "info");
      }
    } catch (err) {
      dispatchToast(`Snapshot failed: ${err}`, "error");
    }
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
      if (!silent) dispatchToast("Configuration Loaded", "success");
    } catch (err) {
      if (!silent) dispatchToast("Config missing or empty", "info");
    }
  }
  async function saveConfig() {
    try {
      await core.invoke("write_config", { contents: configText });
      dispatchToast("Configuration Saved", "success");
    } catch (err) {
      dispatchToast("Failed to save config", "error");
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
          <div class="sh-flex-col fill">
            <div class="sh-editor-wrap">
              <textarea class="sh-editor sh-mono" bind:value={configText}></textarea>
            </div>
            <div class="sh-action-row right">
              <button class="sh-btn sh-btn-ghost" on:click={toggleConfHelp}>HELP</button>
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
      <p style="color:#666; font-size:0.75rem; margin-top:1.5rem; line-height:1.4;">App will respond once the command is done.</p>
    </div>
  </div>
{/if}

<!-- CONFIG HELP MODAL -->
{#if showConfHelp}
  <div class="modal-overlay" role="button" tabindex="0" on:click|self={toggleConfHelp} on:keydown={(e) => e.key === "Escape" && toggleConfHelp()}>
    <div class="modal-staged wide">
      <div class="modal-header">
        <h3>CONFIGURATION GUIDE</h3>
        <button class="btn-close-x" on:click={toggleConfHelp}>X</button>
      </div>
      <div class="modal-body">
        <div class="conf-help-text">
          <p class="highlight-warning"><strong>CRITICAL FOR WINDOWS:</strong> Set <code>daemon=0</code>. Setting <code>daemon=1</code> is for headless Linux/VPS only and will prevent the GUI from connecting to the node.</p>
          <h4 style="color:var(--color-primary); margin-top:1rem;">hemp.conf Reference</h4>
          <p style="font-size:0.8rem; margin-bottom:0.5rem; color:#888;">Complete reference for <code>hemp.conf</code>. Copy options as needed.</p>
          <pre class="selectable">
# ==============================================================================
#                      HEMP0x CORE CONFIGURATION TEMPLATE
# ==============================================================================

# --- ESSENTIAL SETTINGS ---
# server=1: Tells the node to accept JSON-RPC commands.
# REQUIRED for Hemp0x Commander to control the node.
server=1

# listen=1: Listens for connections from outside peers.
# 1 = Run as a full node (Help the network).
# 0 = Don't accept incoming connections (Stealth/Leech mode).
listen=1

# daemon=?: Run in background?
# 0 = Run interactively/controlled by GUI (REQUIRED FOR WINDOWS APP).
# 1 = Run headless in background (Linux/VPS only).
daemon=0

# --- PERFORMANCE & STORAGE ---
# dbcache=N: Database cache size in Megabytes.
# Higher = Faster Sync, uses more RAM.
# 450 = Default (Low RAM).
# 4096 = 4GB (Recommended for fast sync if you have RAM).
dbcache=4096

# prune=N: Prune block storage to N Megabytes?
# 0 = Disable pruning (Keep full history - Required for some features).
# 550 = Minimum size (Saves disk space, but disables Wallet scans on old keys).
prune=0

# maxconnections=N: Maximum number of peer connections.
# Default is 125. Lower if you have limited bandwidth.
# maxconnections=40

# --- INDEXES (Advanced Features) ---
# Enable these if you use the "Assets" or "Tools" tabs heavily.
# note: Changing these requires a -reindex (takes time).

# Required for 'getrawtransaction' (Detailed TX lookup)
txindex=1
# Required for 'getaddress*' calls (Balance lookups API)
addressindex=1
# Required for Asset features
assetindex=1
# Records block timestamps
timestampindex=1
# Tracks spent outputs
spentindex=1

# --- RPC Authentication ---
# Core Next uses cookie auth by default (auto-generated .cookie file).
# No manual credentials needed. Commander detects and uses cookie auth
# automatically when available.

# --- Legacy RPC (username/password fallback) ---
# Only needed if you are not using Core Next cookie auth.
# Uncomment and change these if you cannot use cookie auth:
# rpcuser=hemp0xuser
# rpcpassword=CHANGE_THIS_TO_SECURE_PASSWORD

# rpcallowip=IP: Who can issue commands?
# 127.0.0.1 = Localhost only (Most Secure).
# 192.168.1.* = Local Network (Less Secure).
rpcallowip=127.0.0.1

# rpcport=N: Custom port for RPC interactions.
# Default Mainnet: 8818
# rpcport=8818
</pre>
        </div>
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
  .sh-action-row.right { justify-content: flex-end; padding: 0.5rem 0; }

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
  .conf-help-text { text-align: left; font-size: 0.95rem; line-height: 1.5; }
  .highlight-warning {
    color: #ff5555;
    background: rgba(255, 68, 68, 0.1);
    border: 1px solid #ff5555;
    padding: 0.8rem;
    border-radius: 4px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }
  .selectable {
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
    background: #000;
    padding: 1rem;
    border-radius: 6px;
    border: 1px solid #333;
    color: #aaffaa;
    overflow-x: auto;
    font-family: "Consolas", monospace;
    white-space: pre-wrap;
    font-size: 0.92rem;
    line-height: 1.4;
  }
</style>
