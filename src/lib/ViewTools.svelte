<script>
  import { onMount, onDestroy, tick } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { core } from "@tauri-apps/api";
  import { emit } from "@tauri-apps/api/event";
  import { save, open, ask } from "@tauri-apps/plugin-dialog";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import CryptoJS from "crypto-js";
  import { APP_VERSION } from "./constants.js";
  import ToolsConsole from "./tools/ToolsConsole.svelte";
  import ToolsWallet from "./tools/ToolsWallet.svelte";
  import ToolsNetwork from "./tools/ToolsNetwork.svelte";
  import ToolsJournal from "./tools/ToolsJournal.svelte";
  import ToolsHistory from "./tools/ToolsHistory.svelte";
  import ToolsConsolidation from "./tools/ToolsConsolidation.svelte";
  import ToolsRawTx from "./tools/ToolsRawTx.svelte";
  import ToolsSoloMining from "./tools/ToolsSoloMining.svelte";
  import { nodeStatus, daemonRuntime } from "../stores.js";
  import { addToastNotification } from "./stores/notifications.js";

  let activeSubTab = "CONSOLE";
  let networkMode = "mainnet";
  let tauriReady = false;

  let toastMsg = "";
  let toastType = "info"; // info, error, success
  let toastTimer;

  function showToast(msg, type = "info", notify = true) {
    clearTimeout(toastTimer);
    toastMsg = msg;
    toastType = type;
    if (notify) addToastNotification(msg, type);
    toastTimer = setTimeout(() => {
      toastMsg = "";
    }, 3000);
  }

  export let consoleOutput = "";
  export let consoleHistory = [];

  $: isNodeOnline = $nodeStatus.online;

  async function openDataDir() {
    if (!tauriReady) return;
    try {
      await core.invoke("open_data_dir");
    } catch (err) {
      // Fallback: show path for manual navigation
      try {
        const config = await core.invoke("init_config");
        showToast(`📂 Path: ${config.data_dir}`, "info");
      } catch {
        showToast("Failed to open folder", "error");
      }
    }
  }

  let dataFolderInfo = {
    path: "--",
    size_display: "--",
    config_exists: false,
    wallet_exists: false,
    folder_exists: false,
  };
  let dataLoading = false;

  async function loadDataInfo() {
    if (!tauriReady) return;
    dataLoading = true;
    try {
      dataFolderInfo = await core.invoke("get_data_folder_info");
    } catch (err) {
      showToast("Failed to load data info", "error");
    }
    dataLoading = false;
  }

  async function backupDataFolder() {
    if (!tauriReady) return;
    try {
      const ts = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
      const filePath = await save({
        title: "Save Data Folder Backup",
        defaultPath: `hemp0x_data_backup_${ts}`,
      });
      if (!filePath) return; // User cancelled
      showToast("Backing up data folder...", "info");
      await core.invoke("backup_data_folder_to", { path: filePath });
      showToast(`Backup saved to: ${filePath}`, "success");
      loadDataInfo();
    } catch (err) {
      showToast(`Backup failed: ${err}`, "error");
    }
  }

  async function createDefaultConfig() {
    if (!tauriReady) return;
    try {
      await core.invoke("create_default_config");
      showToast("Default config created", "success");
      loadDataInfo();
      loadConfig(true);
    } catch (err) {
      showToast(`Failed: ${err}`, "error");
    }
  }

  let snapshotInstalling = false;
  let snapshotModalOpen = false;
  let snapshotFilePath = "";

  async function installSnapshot() {
    if (!tauriReady || snapshotInstalling) return;

    // File picker for .7z files
    const selected = await open({
      title: "Select Snapshot Archive",
      filters: [{ name: "7-Zip Archive", extensions: ["7z"] }],
      multiple: false,
    });

    if (!selected) return;

    // Store selected path and show custom modal
    snapshotFilePath = selected;
    snapshotModalOpen = true;
  }

  async function confirmSnapshotInstall() {
    snapshotModalOpen = false;
    snapshotInstalling = true;
    isProcessing = true;

    try {
      // Step 1: Stop the node if running
      processingMessage = "Stopping node...";
      try {
        await core.invoke("stop_node");

        // Wait and verify node is stopped (check up to 10 times)
        for (let i = 0; i < 10; i++) {
          await new Promise((r) => setTimeout(r, 1000));
          try {
            await core.invoke("get_info");
            // Still running, wait more
            processingMessage = `Waiting for node to stop... (${10 - i}s)`;
          } catch {
            // Node is stopped, break out
            break;
          }
        }

        // Extra safety buffer
        await new Promise((r) => setTimeout(r, 1000));
      } catch {
        // Node might already be stopped, continue
      }

      // Step 2: Extract snapshot
      processingMessage = "Extracting snapshot... this may take a few minutes";
      const result = await core.invoke("extract_snapshot", {
        archivePath: snapshotFilePath,
      });

      showToast(result, "success");
      loadDataInfo(); // Refresh data folder info

      // Step 3: Restart node
      processingMessage = "Restarting node...";
      try {
        await core.invoke("start_node");
      } catch {
        showToast(
          "Snapshot installed. Please restart the node manually.",
          "info",
        );
      }
    } catch (err) {
      showToast(`Snapshot failed: ${err}`, "error");
    }

    snapshotInstalling = false;
    isProcessing = false;
  }

  function cancelSnapshotInstall() {
    snapshotModalOpen = false;
    snapshotFilePath = "";
  }

  // APP_VERSION imported from constants.js
  const UPDATE_SERVER = "https://updates.hemp0x.com/";

  let updateInfo = {
    commanderVersion: APP_VERSION,
    daemonVersion: "--",
    cliVersion: "--",
    daemonFound: false,
    cliFound: false,
    txFound: false,
  };
  let updateCheckStatus = "Ready to check for updates";
  let isCheckingUpdate = false;

  let daemonSettings = {
    auto_start_daemon_on_launch: false,
    keep_daemon_running_on_close: false,
    allow_non_bundled_core_next: false,
  };

  async function loadDaemonSettings() {
    try {
      const settings = await core.invoke("load_app_settings");
      daemonSettings.auto_start_daemon_on_launch = settings.auto_start_daemon_on_launch;
      daemonSettings.keep_daemon_running_on_close = settings.keep_daemon_running_on_close;
      daemonSettings.allow_non_bundled_core_next = settings.allow_non_bundled_core_next;
      daemonRuntime.update((d) => ({
        ...d,
        settings: {
          auto_start_daemon_on_launch: settings.auto_start_daemon_on_launch,
          keep_daemon_running_on_close: settings.keep_daemon_running_on_close,
          allow_non_bundled_core_next: settings.allow_non_bundled_core_next,
        },
      }));
    } catch {
      // use defaults
    }
  }

  async function saveDaemonSetting(key, value) {
    daemonSettings[key] = value;
    try {
      const current = await core.invoke("load_app_settings");
      current[key] = value;
      await core.invoke("save_app_settings", { settings: current });
      daemonRuntime.update((d) => ({
        ...d,
        settings: { ...d.settings, [key]: value },
      }));
    } catch (e) {
      showToast(`Failed to save setting: ${e}`, "error");
    }
  }

  // React to node coming online to refresh binary status
  $: if (isNodeOnline && tauriReady) {
    loadUpdateInfo();
  }

  async function loadUpdateInfo() {
    if (!tauriReady) return;

    try {
      const binaries = await core.invoke("get_binary_status");
      updateInfo.daemonFound = !!binaries.daemon_exists;
      updateInfo.cliFound = !!binaries.cli_exists;
      updateInfo.txFound = !!binaries.tx_exists;
    } catch {
      updateInfo.daemonFound = false;
      updateInfo.cliFound = false;
      updateInfo.txFound = false;
    }

    updateInfo.daemonVersion = updateInfo.daemonFound
      ? "Node Offline"
      : "Not Found";
    updateInfo.cliVersion = updateInfo.cliFound ? "Node Offline" : "Not Found";

    if (updateInfo.daemonFound && isNodeOnline) {
      try {
        const info = await core.invoke("run_cli_command", {
          command: "getinfo",
          args: "",
        });
        const parsed = JSON.parse(info);
        updateInfo.daemonVersion = `v${parsed.version || "unknown"}`;
        updateInfo.cliVersion = updateInfo.daemonVersion;

        // Detect Network Mode
        if (parsed.chain) {
          networkMode = parsed.chain;
        } else if (parsed.testnet === true) {
          networkMode = "testnet";
        } else {
          networkMode = "mainnet";
        }
      } catch {
        updateInfo.daemonVersion = "Error";
        updateInfo.cliVersion = "Error";
      }
    }
    updateInfo = updateInfo; // trigger reactivity
  }

  async function checkForUpdates() {
    isCheckingUpdate = true;
    updateCheckStatus = "Checking server...";

    const confirmed = await ask(
      "-Privacy Warning-\nPlease note, this will connect to github.com to check for the latest release.",
      {
        title: "Check for Updates",
        kind: "warning",
        okLabel: "Continue",
        cancelLabel: "Cancel",
      },
    );

    if (!confirmed) {
      isCheckingUpdate = false;
      updateCheckStatus = "Check cancelled.";
      return;
    }

    try {
      const response = await fetch(`${UPDATE_SERVER}version.json`, {
        method: "GET",
        mode: "no-cors",
        signal: AbortSignal.timeout(5000),
      });

      // no-cors doesn't give us response data, so we show placeholder
      updateCheckStatus =
        "âš ï¸ Server check not yet implemented. Visit Hemp0x.com for latest updates.";
    } catch (err) {
      updateCheckStatus =
        "âš ï¸ Could not reach update server. Visit Hemp0x.com for latest updates.";
    }

    isCheckingUpdate = false;
  }

  async function extractBinaries() {
    if (!tauriReady) return;
    try {
      const selected = await open({
        title: "Select Folder for Binaries",
        directory: true,
        multiple: false,
      });
      if (!selected) return;

      showToast("Extracting binaries...", "info");
      const res = await core.invoke("extract_binaries", {
        targetDir: selected,
      });
      showToast(res, "success");
    } catch (err) {
      showToast(`Extraction Failed: ${err}`, "error");
    }
  }

  let configText = "";
  async function loadConfig(silent = false) {
    if (!tauriReady) return;
    try {
      configText = await core.invoke("read_config");
      if (!silent) showToast("Configuration Loaded", "success");
    } catch (err) {
      if (!silent) showToast("Config missing or empty", "info");
    }
  }

  async function saveConfig() {
    if (!tauriReady) return;
    try {
      await core.invoke("write_config", { contents: configText });
      showToast("Configuration Saved", "success");
    } catch (err) {
      showToast("Failed to save config", "error");
    }
  }

  let logText = "";
  async function refreshLog(silent = false) {
    if (!tauriReady) return;
    try {
      logText = await core.invoke("read_log", { lines: 500 });
      if (!silent) showToast("Logs Refreshed", "success");
    } catch (err) {
      if (!silent) showToast("Failed to read logs", "error");
    }
  }
  async function clearLog() {
    try {
      await core.invoke("truncate_log");
      logText = "";
      showToast("Log File Deleted", "success");
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
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
      showToast("Log Downloaded", "success");
    } catch (e) {
      showToast("Save failed", "error");
    }
  }

  let showConfirmModal = false;
  let modalTitle = "";
  let modalMessage = "";
  let modalButtons = []; // { label, style, onClick }

  let isProcessing = false;
  let processingMessage = "Processing...";

  function openModal(title, message, buttons) {
    modalTitle = title;
    modalMessage = message;
    modalButtons = buttons;
    showConfirmModal = true;
  }

  function closeModal() {
    showConfirmModal = false;
  }

  let showConfHelp = false;
  function toggleConfHelp() {
    showConfHelp = !showConfHelp;
  }

  onMount(() => {
    tauriReady = typeof core?.isTauri === "function" ? core.isTauri() : false;
    if (tauriReady) {
      loadConfig(true); // Silent start
      refreshLog(true); // Silent start
      loadDataInfo(); // Load data folder info
      loadUpdateInfo(); // Load update tab info
      loadDaemonSettings(); // Load daemon lifecycle settings
    }
  });

  onDestroy(() => {
    clearTimeout(toastTimer);
  });
</script>

<div class="view-tools">
  <div class="glass-panel panel-strong cyber-panel main-frame">
    <!-- HEADER / TABS -->
    <header class="panel-header no-border">
      <div class="sub-tabs">
        {#each ["CONSOLE", "WALLET", "CONFIG", "DATA", "SYSTEM", "NETWORK", "HISTORY", "JOURNAL", "CONSOLIDATE", "RAW TX", "SOLO MINING", "LOGS"] as tab}
          <button
            class="sub-tab-btn"
            class:active={activeSubTab === tab}
            on:click={() => (activeSubTab = tab)}
          >
            {tab}
          </button>
        {/each}
      </div>
      <div class="header-status mono">
        <span class="dot" class:online={tauriReady}></span>
        {tauriReady ? "SYSTEM ONLINE" : "OFFLINE MODE"}
      </div>
    </header>

    <!-- BODY -->
    <div
      class="tools-body"
      class:no-scroll={activeSubTab === "CONSOLE" ||
        activeSubTab === "CONFIG" ||
        activeSubTab === "LOGS"}
    >
      {#key activeSubTab}
        <div class="transition-wrapper" in:fly={{ y: 20, duration: 300 }}>
          {#if activeSubTab === "CONSOLE"}
            <ToolsConsole
              bind:consoleOutput
              bind:consoleHistory
              bind:isProcessing
              bind:processingMessage
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "WALLET"}
            <ToolsWallet
              bind:isProcessing
              bind:processingMessage
              {openModal}
              {closeModal}
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "CONFIG"}
            <div
              class="tool-grid full-height"
              style="flex: 1; display: flex; flex-direction: column; min-height: 0;"
            >
              <div
                class="terminal-screen"
                style="flex: 1; display: flex; flex-direction: column;"
              >
                <textarea
                  class="config-editor mono"
                  bind:value={configText}
                  style="flex: 1; height: 100%;"
                ></textarea>
              </div>
              <div class="action-bar-right">
                <button class="cyber-btn ghost" on:click={toggleConfHelp}
                  >HELP</button
                >
                <button
                  class="cyber-btn ghost"
                  on:click={() => loadConfig(false)}>RELOAD</button
                >
                <button class="cyber-btn" on:click={saveConfig}
                  >[ SAVE CONFIG ]</button
                >
              </div>
            </div>
          {:else if activeSubTab === "LOGS"}
            <!-- LOGS -->
            <div class="tool-grid full-height">
              <div class="terminal-screen">
                <div class="scanline"></div>
                <textarea
                  class="console-output mono"
                  readonly
                  bind:value={logText}
                ></textarea>
              </div>
              <div class="action-bar-right">
                <button class="cyber-btn ghost" on:click={clearLog}
                  >DELETE LOGS</button
                >
                <button class="cyber-btn" on:click={saveLog}
                  >SAVE LOG (DL)</button
                >
                <button
                  class="cyber-btn ghost"
                  on:click={() => refreshLog(false)}>REFRESH</button
                >
              </div>
            </div>
          {:else if activeSubTab === "DATA"}
            <div class="tool-grid data-view">
              <div class="data-panel">
                <h3 class="data-title">&#x1F4C2; DATA FOLDER</h3>

                <!-- PATH ROW -->
                <div class="path-row">
                  <span class="path-label">PATH:</span>
                  <span class="path-value mono">{dataFolderInfo.path}</span>
                </div>

                <!-- STATUS GRID -->
                <div class="status-grid">
                  <div class="status-item">
                    <span class="status-label">SIZE</span>
                    <span class="status-value"
                      >{dataFolderInfo.size_display}</span
                    >
                  </div>
                  <div class="status-item">
                    <span class="status-label">FOLDER</span>
                    <span
                      class="status-value"
                      class:ok={dataFolderInfo.folder_exists}
                      >{dataFolderInfo.folder_exists
                        ? "EXISTS"
                        : "MISSING"}</span
                    >
                  </div>
                  <div class="status-item">
                    <span class="status-label">CONFIG</span>
                    <span
                      class="status-value"
                      class:ok={dataFolderInfo.config_exists}
                      >{dataFolderInfo.config_exists
                        ? "FOUND"
                        : "MISSING"}</span
                    >
                  </div>
                  <div class="status-item">
                    <span class="status-label">WALLET</span>
                    <span
                      class="status-value"
                      class:ok={dataFolderInfo.wallet_exists}
                      >{dataFolderInfo.wallet_exists
                        ? "FOUND"
                        : "MISSING"}</span
                    >
                  </div>
                </div>

                <!-- ACTION BUTTONS -->
                <div class="data-actions">
                  <button class="cyber-btn" on:click={openDataDir}
                    >OPEN FOLDER</button
                  >
                  <button class="cyber-btn" on:click={backupDataFolder}
                    >BACKUP ALL</button
                  >
                  <button
                    class="cyber-btn primary"
                    on:click={installSnapshot}
                    disabled={snapshotInstalling}
                  >
                    {snapshotInstalling ? "INSTALLING..." : "INSTALL SNAPSHOT"}
                  </button>
                  <button class="cyber-btn ghost" on:click={loadDataInfo}
                    >REFRESH</button
                  >
                </div>
              </div>

              <div class="education-section">
                <h3 class="section-title">📚 ABOUT DATA FOLDER</h3>
                <div class="edu-content">
                  <p>
                    The <strong>data folder</strong> contains all Hemp0x blockchain
                    data and wallet information.
                  </p>
                  <ul class="edu-list">
                    <li>
                      <strong>wallet.dat</strong> - Your wallet keys and transaction
                      history. BACK THIS UP!
                    </li>
                    <li>
                      <strong>hemp.conf</strong> - Node configuration file
                    </li>
                    <li>
                      <strong>blocks/</strong> - Downloaded blockchain data
                    </li>
                    <li><strong>chainstate/</strong> - Current UTXO set</li>
                    <li><strong>debug.log</strong> - Node debug log file</li>
                  </ul>
                  <p class="warning-text">
                    &#x26A0;&#xFE0F; Never share your wallet.dat file with
                    anyone.
                  </p>
                </div>
              </div>
            </div>
          {:else if activeSubTab === "SYSTEM"}
            <div class="tool-grid update-view">
              <!-- APP VER INFO -->
              <div class="update-panel">
                <h3 class="update-title">🔄 APP VER INFO</h3>

                <!-- VERSION INFO GRID -->
                <div class="version-grid">
                  <div class="version-card">
                    <span class="version-label">COMMANDER</span>
                    <span class="version-value"
                      >{updateInfo.commanderVersion}</span
                    >
                  </div>
                  <div class="version-card">
                    <span class="version-label">HEMP0XD</span>
                    <span
                      class="version-value"
                      class:ok={updateInfo.daemonFound}
                      >{updateInfo.daemonVersion}</span
                    >
                  </div>
                  <div class="version-card">
                    <span class="version-label">HEMP0X-CLI</span>
                    <span class="version-value" class:ok={updateInfo.cliFound}
                      >{updateInfo.cliVersion}</span
                    >
                  </div>
                </div>

                <!-- BINARY STATUS -->
                <div class="binary-status">
                  <h4 class="section-subtitle">BINARY STATUS</h4>
                  <div class="binary-row">
                    <span class="binary-name mono">hemp0xd</span>
                    <span
                      class="binary-status-badge"
                      class:found={updateInfo.daemonFound}
                    >
                      {updateInfo.daemonFound ? "✓ FOUND" : "✗ MISSING"}
                    </span>
                  </div>
                  <div class="binary-row">
                    <span class="binary-name mono">hemp0x-cli</span>
                    <span
                      class="binary-status-badge"
                      class:found={updateInfo.cliFound}
                    >
                      {updateInfo.cliFound ? "✓ FOUND" : "✗ MISSING"}
                    </span>
                  </div>
                  <div class="binary-row">
                    <span class="binary-name mono">hemp0x-tx</span>
                    <span
                      class="binary-status-badge"
                      class:found={updateInfo.txFound}
                    >
                      {updateInfo.txFound ? "✓ FOUND" : "✗ MISSING"}
                    </span>
                  </div>
                </div>

                <!-- UPDATE CHECK -->
                <div class="update-check-section">
                  <div class="update-actions">
                    <button class="cyber-btn ghost" on:click={extractBinaries}
                      >EXTRACT BINARIES</button
                    >
                    <div
                      class="update-fallback"
                      style="flex: 1; display: flex; align-items: center; justify-content: center;"
                    >
                      <a
                        href="https://hemp0x.com"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        &#x1F30D; Visit Hemp0x.com
                      </a>
                    </div>
                  </div>
                </div>

                <!-- CHECKSUM PLACEHOLDER -->
                <div class="checksum-section">
                  <h4 class="section-subtitle">CHECKSUM VERIFICATION</h4>
                  <div class="coming-soon">
                    &#x1F512; Coming Soon: Verify app integrity with Hemp token
                    signed releases
                  </div>
                </div>

                <!-- DAEMON SETTINGS -->
                <div class="checksum-section" style="margin-top: 1rem;">
                  <h4 class="section-subtitle">DAEMON SETTINGS</h4>
                  <div class="daemon-settings-grid">
                    <label class="setting-row">
                      <input
                        type="checkbox"
                        checked={daemonSettings.auto_start_daemon_on_launch}
                        on:change={(e) => saveDaemonSetting('auto_start_daemon_on_launch', e.target.checked)}
                      />
                      <span>Auto-start daemon on launch</span>
                    </label>
                    <label class="setting-row">
                      <input
                        type="checkbox"
                        checked={daemonSettings.keep_daemon_running_on_close}
                        on:change={(e) => saveDaemonSetting('keep_daemon_running_on_close', e.target.checked)}
                      />
                      <span>Keep daemon running when Commander closes</span>
                    </label>
                    <label class="setting-row">
                      <input
                        type="checkbox"
                        checked={daemonSettings.allow_non_bundled_core_next}
                        on:change={(e) => saveDaemonSetting('allow_non_bundled_core_next', e.target.checked)}
                      />
                      <span>Allow non-bundled Core Next builds (advanced/developer override)</span>
                    </label>
                  </div>
                </div>
              </div>

              <!-- NETWORK SETTINGS MOVED TO NETWORK TAB -->
            </div>
          {:else if activeSubTab === "NETWORK"}
            <ToolsNetwork
              {activeSubTab}
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "HISTORY"}
            <ToolsHistory
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "JOURNAL"}
            <ToolsJournal
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "CONSOLIDATE"}
            <ToolsConsolidation
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "RAW TX"}
            <ToolsRawTx
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {:else if activeSubTab === "SOLO MINING"}
            <ToolsSoloMining
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
            />
          {/if}
        </div>
      {/key}
    </div>

    <!-- NETWORK RESULT MODAL -->

    <!-- SNAPSHOT INSTALL CONFIRMATION MODAL -->
    {#if snapshotModalOpen}
      <div
        class="modal-overlay"
        transition:fade={{ duration: 150 }}
        role="button"
        tabindex="0"
        on:click|self={cancelSnapshotInstall}
        on:keydown={(e) => e.key === "Escape" && cancelSnapshotInstall()}
      >
        <div
          class="modal-staged snapshot-modal"
          transition:fly={{ y: 20, duration: 200 }}
        >
          <div class="modal-header warning">
            <h3>&#x26A0;&#xFE0F; INSTALL SNAPSHOT</h3>
          </div>
          <div class="modal-body">
            <p class="warning-text">
              This will replace your <strong>blocks</strong> and
              <strong>chainstate</strong> folders.
            </p>
            <p class="highlight-safe">Your wallet.dat will NOT be affected.</p>
            <p class="desc">Make sure you have a backup before proceeding!</p>
            <div class="snapshot-file-info">
              <span class="file-label">FILE:</span>
              <span class="file-path mono"
                >{snapshotFilePath.split(/[/\\]/).pop()}</span
              >
            </div>
          </div>
          <div class="modal-actions">
            <button class="cyber-btn primary" on:click={confirmSnapshotInstall}>
              INSTALL
            </button>
            <button class="cyber-btn ghost" on:click={cancelSnapshotInstall}>
              CANCEL
            </button>
          </div>
        </div>
      </div>
    {/if}

    {#if toastMsg}
      <div
        class="toast-popup"
        transition:fade={{ duration: 200 }}
        class:error={toastType === "error"}
        class:success={toastType === "success"}
      >
        {toastMsg}
      </div>
    {/if}
  </div>

  {#if showConfHelp}
    <div
      class="modal-overlay"
      role="button"
      tabindex="0"
      on:click|self={toggleConfHelp}
      on:keydown={(e) => e.key === "Escape" && toggleConfHelp()}
    >
      <div class="modal-staged wide">
        <div class="modal-header">
          <h3>&#x1F4D6; CONFIGURATION GUIDE</h3>
          <button class="btn-close-x" on:click={toggleConfHelp}>✕</button>
        </div>
        <div class="modal-body">
          <div class="conf-help-text">
            <p class="highlight-warning">
              &#x26A0;&#xFE0F; <strong>CRITICAL FOR WINDOWS:</strong> Set
              <code>daemon=0</code>. Setting <code>daemon=1</code> is for headless
              Linux/VPS only and will prevent the GUI from connecting to the node.
            </p>

            <h4 style="color:var(--color-primary); margin-top:1rem;">
              hemp.conf Reference
            </h4>
            <p style="font-size:0.8rem; margin-bottom:0.5rem; color:#888;">
              Complete reference for <code>hemp.conf</code>. Copy options as
              needed.
            </p>
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
</div>

{#if showConfirmModal}
  <div
    class="modal-overlay"
    role="button"
    tabindex="0"
    on:click|self={closeModal}
    on:keydown={(e) => e.key === "Escape" && closeModal()}
  >
    <div class="modal modal-frame">
      <h3 class="modal-title neon-text">{modalTitle}</h3>
      <p
        class="modal-text"
        style="font-size: 1rem; line-height: 1.6; color: #ddd; margin-bottom: 2rem;"
      >
        {modalMessage}
      </p>
      <div
        class="modal-actions"
        style="justify-content: space-between; gap: 1rem; width: 100%;"
      >
        {#each modalButtons as btn}
          <button
            class="cyber-btn {btn.style === 'ghost'
              ? 'ghost'
              : ''} {btn.style === 'danger'
              ? 'danger ghost'
              : ''} {btn.style === 'primary' ? 'primary-glow' : ''}"
            style="flex: 1;"
            on:click={btn.onClick}
          >
            {btn.label}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

<!-- ENCRYPTION INPUT MODAL -->

<!-- PROCESSING OVERLAY -->
{#if isProcessing}
  <div class="modal-overlay" style="z-index: 999999;">
    <div class="modal-frame" style="text-align:center; max-width: 400px;">
      <h3
        class="neon-text"
        style="color:var(--color-primary); margin:0 0 1rem 0;"
      >
        PLEASE WAIT
      </h3>
      <p style="color:#aaa; margin:0;">{processingMessage}</p>
      <p
        style="color:#666; font-size:0.75rem; margin-top:1.5rem; line-height:1.4;"
      >
        App will respond once the command is done.
      </p>
    </div>
  </div>
{/if}

<style lang="css">
  /* Fix for Logs View Height */
  .tool-grid.full-height {
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .terminal-screen {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: #000;
    border: 1px solid #333;
    border-radius: 6px;
    overflow: hidden;
    position: relative;
    min-height: 0;
  }
  .scanline {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      to bottom,
      rgba(255, 255, 255, 0),
      rgba(255, 255, 255, 0) 50%,
      rgba(0, 0, 0, 0.1) 50%,
      rgba(0, 0, 0, 0.1)
    );
    background-size: 100% 4px;
    pointer-events: none;
    z-index: 5;
    opacity: 0.3;
  }

  @keyframes spin {
    0% {
      transform: rotate(0deg);
    }
    100% {
      transform: rotate(360deg);
    }
  }
  .view-tools {
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
    flex: 1; /* Force expansion in flex parent */
    min-height: 0; /* KEY FIX: Allow shrinking to viewport */
    /* No negative margins needed. Global padding handled by App.svelte */
  }
  .main-frame {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 0; /* Crucial for nested scroll */
  }

  /* --- TOAST --- */
  .toast-popup {
    position: fixed;
    background: rgba(10, 10, 10, 0.95);
    border: 1px solid var(--color-primary);
    padding: 0.8rem 1.2rem;
    border-radius: 6px;
    z-index: 2000000; /* Ensure above all modals */
    max-width: 300px;
    /* CENTERED POPUP as requested */
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    bottom: auto;
    right: auto;

    box-shadow: 0 0 30px rgba(0, 0, 0, 0.8);
    font-family: var(--font-mono);
    font-size: 0.85rem;
    pointer-events: none;
  }
  .toast-popup.error {
    border-color: #ff5555;
    color: #ffaaaa;
    box-shadow: 0 0 30px rgba(255, 80, 80, 0.2);
  }
  .toast-popup.success {
    border-color: #00ff41;
    color: #fff;
    box-shadow: 0 0 30px rgba(0, 255, 65, 0.3);
  }

  /* --- HEADER --- */
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(0, 0, 0, 0.4);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .sub-tabs {
    display: flex;
    gap: 2px;
  }
  .sub-tab-btn {
    background: transparent;
    border: none;
    color: var(--color-muted);
    padding: 1rem 1.5rem;
    font-size: 0.8rem;
    letter-spacing: 1px;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }
  .sub-tab-btn:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.02);
  }
  .sub-tab-btn.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
    background: linear-gradient(
      180deg,
      rgba(0, 0, 0, 0) 0%,
      rgba(0, 255, 65, 0.05) 100%
    );
    text-shadow: 0 0 8px rgba(0, 255, 65, 0.4);
  }
  .header-status {
    padding-right: 1.5rem;
    font-size: 0.7rem;
    color: #555;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .dot {
    width: 8px;
    height: 8px;
    background: #555;
    border-radius: 50%;
  }
  .dot.online {
    background: var(--color-primary);
    box-shadow: 0 0 5px var(--color-primary);
  }

  /* --- BODY --- */
  .tools-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.5rem;
    padding-bottom: 3rem;
    position: relative;
    background: rgba(0, 0, 0, 0.2);
    display: flex;
    flex-direction: column;
  }
  .tools-body.no-scroll {
    overflow-y: hidden;
  }
  .transition-wrapper {
    flex: 1;
    height: auto;
    width: 100%;
    display: flex;
    flex-direction: column;
  }

  /* --- BUTTONS --- */
  .cyber-btn {
    background: rgba(0, 255, 65, 0.05);
    border: 1px solid var(--color-primary);
    color: var(--color-primary);
    padding: 0.8rem 1.5rem;
    letter-spacing: 1px;
    font-weight: bold;
    transition: all 0.2s;
    cursor: pointer;
    text-transform: uppercase;
    font-size: 0.8rem;
    white-space: nowrap;
  }
  .cyber-btn:hover {
    background: var(--color-primary);
    color: #000;
    box-shadow: 0 0 15px rgba(0, 255, 65, 0.4);
  }
  .cyber-btn.ghost {
    border-color: rgba(255, 255, 255, 0.2);
    color: #aaa;
    background: transparent;
  }
  .cyber-btn.ghost:hover {
    border-color: #fff;
    color: #fff;
    box-shadow: none;
    background: rgba(255, 255, 255, 0.05);
  }
  .cyber-btn.danger:hover {
    border-color: #ff5555;
    color: #ff5555;
  }

  /* Removed .cyber-btn.wide as it was reported unused */

  /* === DATA TAB STYLES === */
  .data-view {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding: 0.5rem 0;
    /* overflow-y: auto; REMOVED - Let main body scroll */
  }
  .data-panel {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 8px;
    padding: 1.2rem 1.5rem;
  }
  .data-title {
    font-size: 1rem;
    color: var(--color-primary);
    margin: 0 0 1.2rem 0;
    letter-spacing: 2px;
  }

  /* Path Row */
  .path-row {
    display: flex;
    gap: 1rem;
    align-items: center;
    padding: 0.8rem 1rem;
    background: rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(0, 255, 65, 0.1);
    border-radius: 6px;
    margin-bottom: 1rem;
  }
  .path-label {
    color: #666;
    font-size: 0.75rem;
    letter-spacing: 1px;
    flex-shrink: 0;
  }
  .path-value {
    color: var(--color-primary);
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Status Grid - 4 columns */
  .status-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.8rem;
    margin-bottom: 1.2rem;
  }
  .status-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    padding: 0.8rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    text-align: center;
  }
  .status-label {
    color: #555;
    font-size: 0.65rem;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .status-value {
    color: #888;
    font-size: 0.85rem;
    font-weight: 600;
  }
  .status-value.ok {
    color: var(--color-primary);
  }

  /* Data Actions */
  .data-actions {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }

  /* Education Section */
  .education-section {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 8px;
    padding: 1rem 1.2rem;
  }
  .section-title {
    font-size: 0.9rem;
    color: var(--color-primary);
    margin: 0 0 1rem 0;
    letter-spacing: 1px;
  }
  .edu-content {
    color: #aaa;
    font-size: 0.85rem;
    line-height: 1.6;
  }
  .edu-content p {
    margin: 0.5rem 0;
  }
  .edu-content strong {
    color: var(--color-primary);
  }
  .edu-list {
    margin: 0.8rem 0;
    padding-left: 1.5rem;
  }
  .edu-list li {
    margin: 0.4rem 0;
  }
  .edu-list li strong {
    color: #fff;
  }
  .warning-text {
    color: #ff6666;
    font-weight: bold;
  }
  /* Help Modal Content */
  .conf-help-text {
    text-align: left;
    font-size: 0.95rem; /* Increased size for readability */
    line-height: 1.5;
  }
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
    font-size: 0.92rem; /* Larger code font */
    line-height: 1.4;
  }

  /* === UPDATE TAB STYLES === */
  .update-view {
    display: flex;
    flex-direction: column;
    gap: 1rem; /* User Request: Compact layout */
    padding: 0.5rem 0;
    /* overflow-y: auto; REMOVED - Let main body scroll */
  }
  .update-panel {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.15);
    border-radius: 8px;
    padding: 0.6rem 0.8rem; /* User Request: Compact layout */
  }
  .update-title {
    font-size: 1rem;
    color: var(--color-primary);
    margin: 0 0 1.2rem 0;
    letter-spacing: 2px;
  }

  /* Version Grid - 3 columns */
  .version-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .version-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    text-align: center;
  }
  .version-label {
    color: #555;
    font-size: 0.7rem;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .version-value {
    color: #888;
    font-size: 1rem;
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .version-value.ok {
    color: var(--color-primary);
  }

  /* === MODAL === */
  /* Styles moved to components.css */

  /* Snapshot Modal Specific Styles */
  .snapshot-modal {
    max-width: 450px;
  }
  .modal-header.warning {
    background: rgba(255, 165, 0, 0.15);
    border-bottom-color: rgba(255, 165, 0, 0.3);
  }
  .modal-header.warning h3 {
    color: #ffa500;
  }
  .warning-text {
    color: #ffa500;
    font-weight: 600;
    margin-bottom: 0.75rem;
  }
  .warning-text strong {
    color: #fff;
  }
  .highlight-safe {
    color: var(--color-primary);
    font-weight: 600;
    margin-bottom: 0.5rem;
  }
  .snapshot-file-info {
    margin-top: 1.5rem;
    padding: 0.75rem 1rem;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(0, 255, 65, 0.2);
    border-radius: 8px;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .file-label {
    color: #666;
    font-size: 0.7rem;
    letter-spacing: 1px;
  }
  .file-path {
    color: var(--color-primary);
    font-size: 0.85rem;
    word-break: break-all;
  }

  /* === NEW CSS FOR v1.3 === */
  /* === NEW CSS FOR v1.3 === */
  /* Moved to components.css */

  /* Binary Status */
  .binary-status {
    margin-bottom: 1.5rem;
  }
  .section-subtitle {
    color: #666;
    font-size: 0.75rem;
    letter-spacing: 1px;
    margin: 0 0 0.8rem 0;
    text-transform: uppercase;
  }
  .binary-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.6rem 1rem;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 4px;
    margin-bottom: 0.4rem;
  }
  .binary-name {
    color: #aaa;
    font-size: 0.85rem;
  }
  .binary-status-badge {
    font-size: 0.75rem;
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    background: rgba(255, 68, 68, 0.15);
    color: #ff6666;
  }
  .binary-status-badge.found {
    background: rgba(0, 255, 65, 0.1);
    color: var(--color-primary);
  }

  /* Update Check Section */
  .update-check-section {
    margin-bottom: 1.5rem;
  }
  .update-actions {
    display: flex;
    gap: 1rem;
    margin-bottom: 0.8rem;
  }

  .update-fallback {
    text-align: center;
    padding: 0.5rem;
    margin-top: 1rem; /* Separate from content above */
  }
  .update-fallback a {
    color: var(--color-primary);
    text-decoration: none;
    font-size: 0.85rem;
    transition: opacity 0.2s;
  }
  .update-fallback a:hover {
    opacity: 0.8;
    text-decoration: underline;
  }

  /* Checksum Section */
  .checksum-section {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding-top: 1rem;
  }
  .coming-soon {
    color: #666;
    font-size: 0.85rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px dashed rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    text-align: center;
  }

  .daemon-settings-grid {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.5rem 0;
  }

  .setting-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    color: #aaa;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .setting-row input {
    width: 14px;
    height: 14px;
    accent-color: var(--color-primary);
  }

  .setting-row span {
    font-size: 0.8rem;
    line-height: 1.4;
  }

  .setting-row:hover {
    color: #fff;
  }

  /* === CONFIG EDITOR === */
  .config-editor,
  .console-output {
    flex: 1;
    width: 100%;
    height: 100%;
    background: #000;
    color: #0f0;
    border: none;
    resize: none;
    font-family: "Consolas", monospace;
    font-size: 0.9rem;
    padding: 0.8rem;
    outline: none;
    overflow-y: scroll;
    display: block;
    box-sizing: border-box;
  }
  .config-editor {
    color: #fff; /* White text for config */
  }
  .console-output::-webkit-scrollbar,
  .config-editor::-webkit-scrollbar {
    width: 8px;
  }
  .console-output::-webkit-scrollbar-track,
  .config-editor::-webkit-scrollbar-track {
    background: #111;
  }
  .console-output::-webkit-scrollbar-thumb,
  .config-editor::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 4px;
    border: 1px solid #444;
  }
  .console-output::-webkit-scrollbar-thumb:hover,
  .config-editor::-webkit-scrollbar-thumb:hover {
    background: var(--color-primary);
  }

  @media (max-width: 800px) {
  }
</style>
