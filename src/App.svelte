<script>
  import { onMount } from "svelte";
  import { core } from "@tauri-apps/api";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import {
    nodeStatus,
    walletInfo as walletStore,
    networkInfo,
    systemStatus as systemStore,
    daemonRuntime,
  } from "./stores.js";

  import logoNew from "./assets/logonew.png";
  import eyeOpen from "./assets/eye-open.png";
  import eyeClosed from "./assets/eye-closed.png";
  import ViewSend from "./lib/ViewSend.svelte";
  import ViewReceive from "./lib/ViewReceive.svelte";
  import ViewAssets from "./lib/ViewAssets.svelte";
  import ViewTools from "./lib/ViewTools.svelte";
  import { APP_VERSION } from "./lib/constants.js";

  // --- STATE ---
  let activeTab = "DASHBOARD"; // DASHBOARD, SEND, RECEIVE, ASSETS, TOOLS, ABOUT

  // --- DATA (Populated from daemon) ---
  let nodeInfo = {
    state: "--",
    blocks: "--",
    headers: "--",
    peers: "--",
    diff: "--",
    synced: false, // Conservative default until sync status is confirmed
  };

  let walletInfo = {
    balance: "--",
    pending: "--",
    staked: "--",
    status: "--",
  };

  let recentTx = [];
  let lastError = "";
  let tauriReady = false;
  let sessionStamp = "";
  let uiScale = 1;
  let isRefreshing = false; // Prevent overlapping refresh calls
  let networkMode = "mainnet"; // mainnet, testnet, regtest
  let rpcFailCount = 0;
  let rpcBackoffUntil = 0; // timestamp ms until which RPC is skipped
  let lastKnownDashboard = null; // preserve last good data across transient errors
  let dashboardFailCount = 0;

  // --- PERSISTENT CONSOLE STATE ---
  let globalConsoleOutput = "";
  let globalConsoleHistory = [];

  // --- WELCOME POPUP ---
  let showWelcome = false;
  let showWelcomeOnStartup = true; // Default ON

  // --- HIDE BALANCE ---
  // Default to true (hidden) if not set, or restore from storage
  let hideBalance = localStorage.getItem("hemp0x_hideBalance") !== "false";

  function toggleHideBalance() {
    hideBalance = !hideBalance;
    localStorage.setItem("hemp0x_hideBalance", hideBalance.toString());
  }

  // --- HIDE ACTIVITY ---
  // Default to true (hidden) if not set, or restore from storage
  let hideActivity = localStorage.getItem("hemp0x_hideActivity") !== "false";

  function toggleHideActivity() {
    hideActivity = !hideActivity;
    localStorage.setItem("hemp0x_hideActivity", hideActivity.toString());
  }

  // --- DAEMON LIFECYCLE ---
  let showDaemonConflict = false;
  let conflictRuntimeStatus = null;
  let conflictResolved = false;
  let closeCleanupInProgress = false;
  let closeCleanupComplete = false;
  let appSettings = {
    auto_start_daemon_on_launch: false,
    keep_daemon_running_on_close: false,
    allow_non_bundled_core_next: false,
  };

  async function resolveDaemonConflict(choice) {
    showDaemonConflict = false;
    if (choice === "continue") {
      await core.invoke("release_daemon_ownership");
    } else if (choice === "stop_and_use_bundled") {
      try {
        await core.invoke("stop_node");
        await core.invoke("start_node");
        await core.invoke("take_daemon_ownership");
        daemonRuntime.update((d) => ({ ...d, commanderOwns: true }));
        const readiness = await core.invoke("wait_for_daemon_ready", { timeoutMs: 25000 });
        daemonRuntime.update((d) => ({ ...d, readiness }));
        if (!readiness.ready) {
          lastError = "Daemon did not become ready after start: " + (readiness.rpc_error || "timeout");
        }
      } catch (e) {
        lastError = String(e || "Failed to switch daemon");
      }
    } else {
      await core.invoke("release_daemon_ownership");
    }
    conflictResolved = true;
    daemonRuntime.update((d) => ({ ...d, conflictResolved: true }));
    setTimeout(refreshDashboard, 1500);
  }

  async function loadAppSettings() {
    try {
      appSettings = await core.invoke("load_app_settings");
      daemonRuntime.update((d) => ({
        ...d,
        settings: {
          auto_start_daemon_on_launch: appSettings.auto_start_daemon_on_launch,
          keep_daemon_running_on_close: appSettings.keep_daemon_running_on_close,
          allow_non_bundled_core_next: appSettings.allow_non_bundled_core_next,
        },
      }));
    } catch {
      // use defaults
    }
  }

  async function handleCloseRequested() {
    if (appSettings.keep_daemon_running_on_close) {
      daemonRuntime.update((d) => ({ ...d, commanderOwns: false }));
      return;
    }
    try {
      let owns = false;
      try {
        const ownership = await core.invoke("get_daemon_ownership");
        owns = ownership.commander_owns;
      } catch {
        // can't determine ownership, leave alone
      }
      if (owns) {
        await core.invoke("stop_node");
        await core.invoke("release_daemon_ownership");
      }
    } catch {
      // best-effort cleanup
    }
  }

  function closeWelcome() {
    // Save preference
    localStorage.setItem(
      "hemp0x_showWelcome",
      showWelcomeOnStartup ? "1" : "0",
    );
    showWelcome = false;
  }

  function checkWelcomePopup() {
    const stored = localStorage.getItem("hemp0x_showWelcome");
    // Default to showing if never set
    showWelcomeOnStartup = stored !== "0";
    if (showWelcomeOnStartup) {
      showWelcome = true;
    }
  }

  // System status for traffic light indicators
  // green = all systems go, yellow = syncing/partial, red = offline/error
  $: systemStatus = (() => {
    const nodeRunning = nodeInfo.state === "RUNNING";
    const walletLoaded =
      walletInfo.status !== "--" && walletInfo.status !== "UNLOADED";
    const isSynced = nodeInfo.synced === true; // From blockchain sync check

    if (nodeRunning && walletLoaded && isSynced) {
      return "green"; // All systems go
    } else if (nodeRunning && (!isSynced || !walletLoaded)) {
      return "yellow"; // Syncing or partial connection
    } else {
      return "red"; // Disconnected or error
    }
  })();
  let showWalletPrompt = false;
  let walletPromptMode = "unlock";
  let walletPromptPass = "";
  let walletPromptPassConfirm = "";
  let walletPromptDuration = "60";
  let walletPromptError = "";
  // --- DASHBOARD OVERHAUL ---
  let isActivityExpanded = false;

  function toggleActivityExpand() {
    isActivityExpanded = !isActivityExpanded;
  }

  function isActive(tab) {
    return activeTab === tab;
  }
  function setTab(tab) {
    activeTab = tab;
  }

  function setOffline(reason = "") {
    if (nodeInfo.state === "OFFLINE" && lastError === reason) return;

    lastKnownDashboard = null;
    nodeInfo = {
      state: "OFFLINE",
      blocks: "--",
      headers: "--",
      peers: "--",
      diff: "--",
      synced: false,
    };
    walletInfo = {
      balance: "--",
      pending: "--",
      staked: "--",
      status: "--",
    };
    recentTx = [];
    lastError = reason;

    // UPDATE STORES (OFFLINE)
    nodeStatus.set({
      online: false,
      version: "--",
      connections: 0,
      headers: 0,
      blocks: 0,
      verificationProgress: 0,
      error: reason,
    });
    walletStore.set({
      balance: "0.00",
      unconfirmed: 0.0,
      immature: 0.0,
      transactions: [],
      newTxCount: 0,
      status: "--",
    });
  }

  async function refreshDashboard() {
    if (isRefreshing) return;
    isRefreshing = true;
    try {
      if (!tauriReady) {
        setOffline("Tauri backend not available.");
        return;
      }

      const tryRpc = rpcFailCount < 3 && Date.now() > rpcBackoffUntil;
      let data = null;

      if (tryRpc) {
        try {
          data = await core.invoke("rpc_dashboard");
          rpcFailCount = 0;
          rpcBackoffUntil = 0;
        } catch (rpcErr) {
          rpcFailCount++;
          if (rpcFailCount >= 3) {
            rpcBackoffUntil = Date.now() + 120000;
          } else {
            rpcBackoffUntil = Date.now() + 1000;
          }
        }
      }

      if (!data) {
        data = await core.invoke("dashboard_data");
        if (tryRpc) {
          rpcFailCount = Math.min(rpcFailCount, 2);
        }
      }

      lastKnownDashboard = data;
      dashboardFailCount = 0;

      nodeInfo = data.node;
      walletInfo = data.wallet;
      recentTx = data.tx;
      lastError = "";

      nodeStatus.set({
        online: data.node.state === "RUNNING",
        version: "--",
        connections: parseInt(data.node.peers) || 0,
        headers: parseInt(data.node.headers) || 0,
        blocks: parseInt(data.node.blocks) || 0,
        verificationProgress: 0,
        error: null,
      });

      walletStore.set({
        balance: data.wallet.balance || "0.00",
        unconfirmed: parseFloat(data.wallet.pending) || 0.0,
        immature: parseFloat(data.wallet.staked) || 0.0,
        transactions: data.tx || [],
        newTxCount: 0,
        status: data.wallet.status || "--",
      });

      networkInfo.update((n) => ({
        ...n,
        difficulty: parseFloat(data.node.diff) || 0,
      }));
    } catch (err) {
      if (lastKnownDashboard) {
        dashboardFailCount++;
        nodeInfo = {
          ...lastKnownDashboard.node,
          state: dashboardFailCount >= 3 ? "OFFLINE" : lastKnownDashboard.node.state,
          synced: dashboardFailCount >= 3 ? false : lastKnownDashboard.node.synced,
        };
        walletInfo = lastKnownDashboard.wallet;
        recentTx = lastKnownDashboard.tx;
        lastError = String(err || "RPC error");
        nodeStatus.update((current) => ({
          ...current,
          online: dashboardFailCount < 3 && current.online,
          error: lastError,
        }));
      } else {
        setOffline(String(err || "RPC error"));
      }
    } finally {
      isRefreshing = false;
    }
  }

  function walletActionLabel(status) {
    if (status === "UNENCRYPTED") {
      return "ENCRYPT WALLET";
    }
    if (status === "LOCKED") {
      return "UNLOCK WALLET";
    }
    return "LOCK WALLET";
  }

  function openWalletPrompt(mode) {
    walletPromptMode = mode;
    walletPromptPass = "";
    walletPromptPassConfirm = "";
    walletPromptDuration = "60";
    walletPromptError = "";
    showWalletPrompt = true;
  }

  function closeWalletPrompt() {
    showWalletPrompt = false;
  }

  async function confirmWalletPrompt() {
    if (!tauriReady) return;
    try {
      if (walletPromptMode === "encrypt") {
        if (!walletPromptPass) {
          walletPromptError = "Password required.";
          return;
        }
        if (walletPromptPass !== walletPromptPassConfirm) {
          walletPromptError = "Passwords do not match.";
          return;
        }
        await core.invoke("wallet_encrypt", { password: walletPromptPass });
      } else if (walletPromptMode === "unlock") {
        if (!walletPromptPass) {
          walletPromptError = "Password required.";
          return;
        }
        const duration = Number(walletPromptDuration || 60);
        await core.invoke("wallet_unlock", {
          password: walletPromptPass,
          duration,
        });
      }
      // Check Network Mode
      try {
        networkMode = await core.invoke("get_network_mode");
      } catch (e) {
        console.error("Failed to get network mode", e);
      }

      closeWalletPrompt();
      await refreshDashboard();
    } catch (err) {
      walletPromptError = String(err || "Wallet action failed");
    }
  }

  async function handleWalletAction() {
    if (!tauriReady) return;
    try {
      if (walletInfo.status === "UNENCRYPTED") {
        openWalletPrompt("encrypt");
      } else if (walletInfo.status === "LOCKED") {
        openWalletPrompt("unlock");
      } else {
        await core.invoke("wallet_lock");
      }
    } catch (err) {
      lastError = String(err || "Wallet action failed");
    }
  }

  async function handleStart() {
    if (!tauriReady) return;
    try {
      await core.invoke("start_node");
      setTimeout(refreshDashboard, 1500);
    } catch (err) {
      lastError = String(err || "Failed to start node");
    }
  }

  async function handleStop() {
    if (!tauriReady) return;
    try {
      await core.invoke("stop_node");
      setTimeout(refreshDashboard, 1500);
    } catch (err) {
      lastError = String(err || "Failed to stop node");
    }
  }

  onMount(() => {
    tauriReady = true; // Force accurate ready state for UI logic
    systemStore.update((s) => ({ ...s, tauriReady: true }));

    // Show window unconditionally (Anti-Flash)
    setTimeout(async () => {
      try {
        await getCurrentWindow().show();
        await getCurrentWindow().setFocus();
      } catch (e) {
        console.warn("Could not show window:", e);
      }
    }, 150);
    sessionStamp = new Date().toISOString().replace("T", " ").slice(0, 19);
    const updateScale = () => {
      const w = window.innerWidth || 1080;
      const h = window.innerHeight || 720;

      // FIXED RATIO ZOOM: Everything locked in place.
      // Acts like resizing an image. Base resolution: 1280x800.
      let ratio = Math.min(w / 1280, h / 800);

      // No minimum floor - allows infinite shrinking to prevent cutoff.
      // Cap at 1.0 to ensure sharpness.
      ratio = Math.min(ratio, 1.0);

      uiScale = Number(ratio.toFixed(4));
    };
    updateScale();
    window.addEventListener("resize", updateScale);

    let unlistenNetwork;
    if (tauriReady) {
      // Handle close event for daemon lifecycle
      getCurrentWindow().onCloseRequested(async (event) => {
        if (appSettings.keep_daemon_running_on_close) {
          return;
        }
        if (closeCleanupComplete) {
          return;
        }
        event.preventDefault();
        if (closeCleanupInProgress) {
          return;
        }
        closeCleanupInProgress = true;
        await handleCloseRequested();
        closeCleanupComplete = true;
        await getCurrentWindow().close();
      });

      // Init daemon runtime
      (async function initDaemonRuntime() {
        try {
          await loadAppSettings();
          const status = await core.invoke("get_runtime_status");
          daemonRuntime.update((d) => ({
            ...d,
            bundledCoreNextReady: status.bundled_core_next_ready,
            probe: status.probe,
            daemon: {
              path: status.daemon.path,
              exists: status.daemon.exists,
              raw: status.daemon.raw,
              base_version: status.daemon.base_version,
              commit_hash: status.daemon.commit_hash,
              exact_core_next_match: status.daemon.exact_core_next_match,
            },
          }));

          if (status.probe.rpc_port_open) {
            conflictRuntimeStatus = status;
            try {
              const identity = await core.invoke("identify_running_daemon", {
                allowNonBundled: appSettings.allow_non_bundled_core_next,
              });
              daemonRuntime.update((d) => ({ ...d, runningIdentity: identity }));
              conflictRuntimeStatus.identity = identity;
            } catch {
              // identity probe is best-effort
            }
            showDaemonConflict = true;
          } else if (appSettings.auto_start_daemon_on_launch && status.bundled_core_next_ready) {
            try {
              await core.invoke("start_node");
              await core.invoke("take_daemon_ownership");
              daemonRuntime.update((d) => ({ ...d, commanderOwns: true }));
              const readiness = await core.invoke("wait_for_daemon_ready", { timeoutMs: 30000 });
              daemonRuntime.update((d) => ({ ...d, readiness }));
              if (!readiness.ready) {
                lastError = "Daemon did not become ready after start: " + (readiness.rpc_error || "timeout");
              }
              conflictResolved = true;
              daemonRuntime.update((d) => ({ ...d, conflictResolved: true }));
            } catch (e) {
              console.error("Failed to auto-start daemon:", e);
            }
          } else {
            conflictResolved = true;
            daemonRuntime.update((d) => ({ ...d, conflictResolved: true }));
          }

          if (appSettings.allow_non_bundled_core_next === false &&
              !status.bundled_core_next_ready &&
              status.daemon.exists) {
            console.warn("Bundled Core Next is not the exact required version");
          }
        } catch (e) {
          console.error("Daemon runtime init failed:", e);
          conflictResolved = true;
          daemonRuntime.update((d) => ({ ...d, conflictResolved: true }));
        }
      })();

      // Load Network Mode
      core
        .invoke("get_network_mode")
        .then((res) => {
          networkMode = res;
          networkInfo.update((n) => ({ ...n, chain: res }));
        })
        .catch((err) => {
          console.error("Failed to load network mode", err);
        });

      // Listen for runtime changes
      listen("network-changed", (event) => {
        if (event.payload && event.payload.mode) {
          networkMode = event.payload.mode;
          networkInfo.update((n) => ({ ...n, chain: event.payload.mode }));
        }
      }).then((fn) => {
        unlistenNetwork = fn;
      });
    }
    checkWelcomePopup(); // Show welcome popup if enabled

    // Adaptive Polling Logic for Performance
    const performPoll = async () => {
      if (conflictResolved) {
        await refreshDashboard();
      }

      let delay = 5000;
      if (systemStatus === "yellow") delay = 8000;
      else if (systemStatus === "red") delay = 5000;

      timer = setTimeout(performPoll, delay);
    };

    let timer = setTimeout(performPoll, 100);

    return () => {
      clearTimeout(timer);
      window.removeEventListener("resize", updateScale);
      if (typeof unlistenNetwork === "function") unlistenNetwork();
    };
  });
</script>

<main class="shell" style={`--ui-scale: ${uiScale};`}>
  <!-- HEADER -->
  <header class="top-bar">
    <div class="brand">
      <img src={logoNew} alt="Hemp0x" class="logo" />
      <div class="brand-info">
        <h1 class="app-title">
          HEMP0X COMMANDER <span class="version">{APP_VERSION}</span>
        </h1>
        <div class="app-status">SECURE SESSION - {sessionStamp}</div>
      </div>
    </div>

    <nav class="main-nav">
      {#each ["DASHBOARD", "SEND", "RECEIVE", "ASSETS", "TOOLS", "ABOUT"] as tab}
        <button
          class="tab-btn"
          class:active={isActive(tab)}
          type="button"
          on:click={() => setTab(tab)}
        >
          {tab}
        </button>
      {/each}
    </nav>

    <div class="window-controls">
      <div class="status-stack">
        <div class="traffic-lights">
          <!-- Status Traffic Lights -->
          <span
            class="status-dot green"
            class:active={systemStatus === "green"}
            title="All Systems Go"
          ></span>
          <span
            class="status-dot yellow"
            class:active={systemStatus === "yellow"}
            title="Syncing/Partial"
          ></span>
          <span
            class="status-dot red"
            class:active={systemStatus === "red"}
            title="Node Offline"
          ></span>
        </div>
        {#if networkMode !== "mainnet"}
          <div class="network-warning-text">
            {networkMode.toUpperCase()}
          </div>
        {/if}
      </div>
    </div>
  </header>

  <!-- CONTENT AREA -->
  <div class="content" class:no-padding={activeTab === "TOOLS"}>
    <!-- DASHBOARD VIEW -->
    <!-- DASHBOARD VIEW -->
    <div class="view-wrapper" class:show={activeTab === "DASHBOARD"}>
      <div class="view-dashboard">
        <!-- TOP ROW -->
        <div class="row-top">
          <!-- NODE PANEL -->
          <div class="glass-panel node-card cyber-panel">
            <header class="panel-header">
              <span class="hud-title mono">[ NODE STATUS ]</span>
              <div class="stat-pill">
                <span
                  class="led"
                  class:running={nodeInfo.state === "RUNNING"}
                  class:stopped={nodeInfo.state !== "RUNNING"}
                ></span>
                <span class="value"
                  >{nodeInfo.state === "RUNNING" ? "RUNNING" : "STOPPED"}</span
                >
              </div>
            </header>

            <div class="panel-content">
              <div class="stat-grid-compact">
                <div class="stat-pair">
                  <span class="label">BLOCK HEIGHT</span>
                  <span class="mono">{nodeInfo.blocks}</span>
                </div>
                <div class="stat-pair">
                  <span class="label">PEERS</span>
                  <span class="mono">{nodeInfo.peers}</span>
                </div>
                <div class="stat-pair">
                  <span class="label">DIFF</span>
                  <span class="mono">{nodeInfo.diff}</span>
                </div>
              </div>
            </div>

            <div class="panel-actions">
              <button class="btn-xs" on:click={handleStart}>START</button>
              <button class="btn-xs ghost" on:click={handleStop}>STOP</button>
            </div>
          </div>

          <!-- WALLET PANEL -->
          <div class="glass-panel wallet-card cyber-panel">
            <header class="panel-header">
              <div class="header-left">
                <span class="hud-title mono">[ WALLET ]</span>
                <button
                  class="btn-eye"
                  title={hideBalance ? "Show Balance" : "Hide Balance"}
                  on:click={toggleHideBalance}
                >
                  <img src={hideBalance ? eyeClosed : eyeOpen} alt="toggle" />
                </button>
              </div>
              <div
                class="status-chip"
                class:status-red={walletInfo.status === "UNENCRYPTED"}
                class:status-green={walletInfo.status !== "UNENCRYPTED" &&
                  walletInfo.status !== "--"}
              >
                {walletInfo.status}
              </div>
            </header>

            <div class="panel-content wallet-content compact-wallet">
              <!-- Added compact-wallet class -->
              <div class="balance-hero-small">
                <div class="val neon-glow" class:blurred={hideBalance}>
                  {hideBalance ? "******" : walletInfo.balance}
                  <span class="unit">HEMP</span>
                </div>
                <div class="sub">AVAILABLE BALANCE</div>
              </div>
            </div>

            <div class="panel-actions wallet-actions">
              <button
                class="btn-xs"
                class:disabled={nodeInfo.state !== "RUNNING" ||
                  walletInfo.status === "--"}
                disabled={nodeInfo.state !== "RUNNING" ||
                  walletInfo.status === "--"}
                on:click={handleWalletAction}
              >
                {nodeInfo.state !== "RUNNING"
                  ? "NOT CONNECTED"
                  : walletActionLabel(walletInfo.status)}
              </button>
            </div>
          </div>
        </div>

        <!-- BOTTOM ROW: ACTIVITY -->
        <div class="row-bottom">
          <div
            class="glass-panel activity-card cyber-panel"
            class:expanded={isActivityExpanded}
          >
            <header class="panel-header">
              <div class="header-left">
                <span class="hud-title mono">[ RECENT ACTIVITY ]</span>
                <button
                  class="btn-eye"
                  title={hideActivity ? "Show Activity" : "Hide Activity"}
                  on:click={toggleHideActivity}
                >
                  <img src={hideActivity ? eyeClosed : eyeOpen} alt="toggle" />
                </button>
                <button
                  class="btn-eye btn-expand"
                  title={isActivityExpanded ? "Collapse" : "Expand Full Screen"}
                  on:click={toggleActivityExpand}
                >
                  <!-- Simple ^ chevron or box icon -->
                  <span class="expand-icon"
                    >{isActivityExpanded ? "▼" : "▲"}</span
                  >
                </button>
              </div>
              <span class="hint mono">LAST 50 TRANSACTIONS</span>
            </header>

            <div class="header-row">
              <span>Date</span>
              <span>Type</span>
              <span>Amount</span>
              <span>Conf</span>
              <span>TXID</span>
            </div>

            <div
              class="scroll-body custom-scroll"
              class:activity-mask={hideActivity}
            >
              {#each recentTx as tx}
                <div class="data-row">
                  <span class="mono dim">{tx.date}</span>
                  <span class="type {tx.type}">{tx.type}</span>
                  <span
                    class="mono amount {tx.amount.startsWith('-')
                      ? 'neg'
                      : 'pos'}"
                  >
                    {tx.amount}
                  </span>
                  <span class="mono dim">{tx.conf}</span>
                  <span class="mono txid" title={tx.txid}>{tx.txid}</span>
                </div>
              {/each}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="view-wrapper" class:show={activeTab === "SEND"}>
      <ViewSend />
    </div>

    <div class="view-wrapper" class:show={activeTab === "RECEIVE"}>
      <ViewReceive />
    </div>

    <div class="view-wrapper" class:show={activeTab === "ASSETS"}>
      <ViewAssets />
    </div>

    <div class="view-wrapper" class:show={activeTab === "TOOLS"}>
      <ViewTools
        bind:consoleOutput={globalConsoleOutput}
        bind:consoleHistory={globalConsoleHistory}
      />
    </div>

    <div class="view-wrapper" class:show={activeTab === "ABOUT"}>
      <div class="about-page glass-panel panel-strong">
        <div class="about-header">
          <img src={logoNew} alt="Hemp0x Logo" class="about-logo" />
          <div class="about-title-block">
            <h1 class="about-title">HEMP0X COMMANDER</h1>
            <span class="about-version">{APP_VERSION}</span>
          </div>
          <label class="welcome-toggle">
            <input
              type="checkbox"
              bind:checked={showWelcomeOnStartup}
              on:change={() =>
                localStorage.setItem(
                  "hemp0x_showWelcome",
                  showWelcomeOnStartup ? "1" : "0",
                )}
            />
            <span>Show welcome on startup</span>
          </label>
        </div>

        <div class="about-section">
          <h3 class="section-header">📖 ABOUT</h3>
          <p class="about-text">
            All-In-One Manager for Hemp0x blockchain.<br />
            This application controls <strong>hemp0xd</strong> and
            <strong>hemp0x-cli</strong> binaries built from the Hemp0x repository.
          </p>
          <p class="about-credit">
            Forked from Raven Coin (12-18-25)<br />
            Special thanks to: <strong>cc2002cc</strong> and the Ravencoin community
            for making this all possible
          </p>
        </div>

        <div class="about-section disclaimer-section">
          <h3 class="section-header">⚠️ DISCLAIMER</h3>
          <p class="disclaimer-text">
            This software allows you to make <strong class="danger"
              >irreversible changes</strong
            >
            to your wallet and blockchain data.
          </p>
          <p class="disclaimer-text">
            By using this application, you acknowledge and agree that:
          </p>
          <ul class="disclaimer-list">
            <li>You use this software <strong>at your own risk</strong></li>
            <li>
              We are <strong>not responsible</strong> for any lost funds or data
            </li>
            <li>
              You should always <strong>backup your wallet</strong> before making
              changes
            </li>
            <li>
              Encrypted wallets require you to remember your password - there is
              no recovery
            </li>
          </ul>
        </div>

        <div class="about-footer">
          <span class="neon">Hemp0x - We Build Together</span>
          <a
            href="https://hemp0x.com"
            target="_blank"
            rel="noopener noreferrer"
            class="about-link">hemp0x.com</a
          >
        </div>
      </div>
    </div>

    <!-- FOOTER -->
    <div class="app-footer">
      <a href="https://hemp0x.com" target="_blank" class="footer-link"
        >Hemp0x.com</a
      >
    </div>
  </div>

  {#if showWalletPrompt}
    <button
      class="modal-backdrop"
      type="button"
      aria-label="Close wallet prompt"
      on:click={closeWalletPrompt}
    ></button>
    <div class="modal">
      <h3 class="modal-title">
        {walletPromptMode === "encrypt" ? "Encrypt Wallet" : "Unlock Wallet"}
      </h3>
      {#if walletPromptMode === "encrypt"}
        <p class="modal-text">
          Encryption is permanent. If you lose this password, funds are lost.
        </p>
      {/if}
      <label class="modal-label" for="wallet-pass">Password</label>
      <input
        id="wallet-pass"
        type="password"
        class="modal-input"
        bind:value={walletPromptPass}
        placeholder="Enter password"
        on:keydown={(e) => e.key === "Enter" && confirmWalletPrompt()}
      />
      {#if walletPromptMode === "encrypt"}
        <label class="modal-label" for="wallet-pass-confirm"
          >Confirm Password</label
        >
        <input
          id="wallet-pass-confirm"
          type="password"
          class="modal-input"
          bind:value={walletPromptPassConfirm}
          placeholder="Confirm password"
        />
      {:else}
        <label class="modal-label" for="wallet-duration"
          >Duration (seconds)</label
        >
        <input
          id="wallet-duration"
          type="number"
          class="modal-input"
          bind:value={walletPromptDuration}
          min="1"
        />
      {/if}
      {#if walletPromptError}
        <div class="modal-error" role="alert">{walletPromptError}</div>
      {/if}
      <div class="modal-actions">
        <button class="btn-sm ghost" on:click={closeWalletPrompt}>Cancel</button
        >
        <button class="btn-sm" on:click={confirmWalletPrompt}>
          {walletPromptMode === "encrypt" ? "Encrypt" : "Unlock"}
        </button>
      </div>
    </div>
  {/if}

  <!-- WELCOME POPUP -->
  {#if showWelcome}
    <div class="welcome-overlay">
      <div class="welcome-modal">
        <div class="welcome-header">
          <h2>Welcome To Hemp0x Commander</h2>
          <span class="welcome-version">{APP_VERSION}</span>
        </div>
        <div class="welcome-body">
          <p class="welcome-text">
            This software allows you to make <strong
              >irreversible changes</strong
            > to your wallet.
          </p>
          <p class="welcome-caution">
            ⚠️ Use with caution. See the <strong>ABOUT</strong> page for more information
            before use.
          </p>
          <p class="welcome-disclaimer">
            We are not responsible for any lost funds.
          </p>
        </div>
        <div class="welcome-footer">
          <label class="welcome-checkbox">
            <input type="checkbox" bind:checked={showWelcomeOnStartup} />
            <span class="checkmark"></span>
            Show this message on startup
          </label>
          <button class="welcome-btn" on:click={closeWelcome}
            >[ CONTINUE ]</button
          >
        </div>
      </div>
    </div>
  {/if}

  <!-- DAEMON CONFLICT MODAL -->
  {#if showDaemonConflict && conflictRuntimeStatus}
    <div class="welcome-overlay">
      <div class="welcome-modal">
        <div class="welcome-header">
          <h2>Daemon Already Running</h2>
        </div>
        <div class="welcome-body">
          <p class="welcome-text">
            A Hemp0x daemon was detected on the default ports (RPC: {conflictRuntimeStatus.probe.default_rpc_port}, P2P: {conflictRuntimeStatus.probe.default_p2p_port}).
          </p>

          {#if conflictRuntimeStatus.identity}
            {#if conflictRuntimeStatus.identity.rpc_authenticated}
              <p class="welcome-text" style="color: {conflictRuntimeStatus.identity.is_required_core_next ? '#4caf50' : '#ff9800'};">
                {conflictRuntimeStatus.identity.status}
              </p>
              {#if conflictRuntimeStatus.identity.base_version}
                <p class="welcome-text" style="font-size: 0.85rem; color: #aaa;">
                  Version: {conflictRuntimeStatus.identity.base_version}
                  {#if conflictRuntimeStatus.identity.subversion}
                    / {conflictRuntimeStatus.identity.subversion}
                  {/if}
                  (Protocol: {conflictRuntimeStatus.identity.protocol_version})
                </p>
              {/if}
            {:else}
              <p class="welcome-caution">
                {conflictRuntimeStatus.identity.status}
              </p>
            {/if}
          {:else}
            <p class="welcome-caution">
              A daemon is listening on the default RPC port, but Commander could not verify its version.
            </p>
          {/if}

          {#if !conflictRuntimeStatus.bundled_core_next_ready}
            <p class="welcome-caution">
              The bundled daemon does not match the required Core Next build
              ({conflictRuntimeStatus.required_base_version}-{conflictRuntimeStatus.required_commit_hash}).
              {#if conflictRuntimeStatus.daemon.base_version}
                Bundled binary: v{conflictRuntimeStatus.daemon.base_version}
                {#if conflictRuntimeStatus.daemon.commit_hash}
                  ({conflictRuntimeStatus.daemon.commit_hash})
                {/if}
              {:else}
                Bundled binary: unrecognized version
              {/if}
            </p>
          {/if}
          <p class="welcome-text" style="font-size: 0.85rem; color: #aaa;">
            Choose how to proceed:
          </p>
        </div>
        <div class="welcome-footer" style="flex-wrap: wrap; gap: 0.5rem;">
          <button class="btn-xs" style="flex: 1;" on:click={() => resolveDaemonConflict('continue')}>
            Continue with existing daemon
          </button>
          {#if conflictRuntimeStatus.bundled_core_next_ready}
            <button class="btn-xs" style="flex: 1;" on:click={() => resolveDaemonConflict('stop_and_use_bundled')}>
              Stop it and use bundled Core Next
            </button>
          {/if}
          <button class="btn-xs ghost" style="flex: 1;" on:click={() => resolveDaemonConflict('cancel')}>
            Stay offline
          </button>
        </div>
      </div>
    </div>
  {/if}
</main>

<style>
  /* --- LAYOUT SHELL --- */
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    position: relative;
    background: #000; /* PURE VOID */
    /* REMOVED FIXED SCALING - Allow Responsive Layout */
    /* transform: scale(var(--ui-scale, 1)); */
    /* transform-origin: top left; */
    /* width: calc(100% / var(--ui-scale, 1)); */
    /* height: calc(100% / var(--ui-scale, 1)); */
    overflow: hidden;
  }

  /* --- HEADER --- */
  .top-bar {
    height: 72px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05); /* Slightly brighter separator */
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(12px);
    z-index: 50;
    -webkit-app-region: no-drag;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .logo {
    height: 42px;
    width: 42px;
    border-radius: 12px;
    box-shadow: 0 0 25px rgba(0, 255, 65, 0.25); /* Stronger logo glow */
    filter: brightness(1.1);
  }
  .app-title {
    margin: 0;
    font-size: 1.05rem;
    letter-spacing: 2px;
    color: #fff;
    text-shadow: 0 0 15px rgba(0, 255, 65, 0.3); /* Title glow */
  }
  .version {
    font-size: 0.8rem;
    color: var(--color-muted);
    opacity: 0.7;
    text-shadow: none;
  }
  .app-status {
    color: var(--color-primary-dim);
    letter-spacing: 1px;
    margin-top: 4px;
    text-shadow: 0 0 5px rgba(0, 255, 65, 0.3);
    white-space: nowrap; /* Prevent wrap */
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Status Traffic Light Dots */
  .status-dot {
    display: inline-block;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    margin-left: 4px;
    opacity: 0.4; /* Dim when inactive */
    transition: all 0.3s ease;
    cursor: help;
  }
  /* Neon colors matching app theme */
  .status-dot.green {
    background: #00ff41; /* App primary green */
  }
  .status-dot.yellow {
    background: #ffdd00; /* Vibrant neon yellow */
  }
  .status-dot.red {
    background: #ff4444; /* Like STOPPED indicator */
  }
  /* Active glow effect */
  .status-dot.active {
    opacity: 1;
    animation: statusPulse 2s ease-in-out infinite;
  }
  .status-dot.green.active {
    box-shadow:
      0 0 6px #00ff41,
      0 0 12px #00ff41,
      0 0 18px rgba(0, 255, 65, 0.5);
  }
  .status-dot.yellow.active {
    box-shadow:
      0 0 6px #ffdd00,
      0 0 12px #ffdd00,
      0 0 18px rgba(255, 221, 0, 0.5);
  }
  .status-dot.red.active {
    box-shadow:
      0 0 6px #ff4444,
      0 0 12px #ff4444,
      0 0 18px rgba(255, 68, 68, 0.5);
  }
  @keyframes statusPulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.75;
    }
  }

  .main-nav {
    display: flex;
    gap: 0.5rem;
    height: 100%;
    align-items: flex-end;
    position: relative;
    z-index: 60;
    pointer-events: auto;
    -webkit-app-region: no-drag;
  }
  .tab-btn {
    pointer-events: auto;
    -webkit-app-region: no-drag;
  }

  .window-controls {
    display: flex;
    gap: 8px;
  }

  /* --- CONTENT CONTAINER --- */
  .content {
    flex: 1;
    position: relative;
    overflow: hidden; /* No Global Scroll */
    padding: 0.8rem 1.5rem 1.5rem 1.5rem;
    display: flex;
    flex-direction: column;
  }
  .content.no-padding {
    padding-bottom: 0;
  }

  /* --- COMMON PANEL STYLE --- */
  .glass-panel {
    background: rgba(10, 14, 12, 0.7); /* DARKER */
    backdrop-filter: blur(15px); /* HEAVIER BLUR */
    border: 1px solid rgba(0, 255, 65, 0.15); /* NEON BORDER */
    border-top: 1px solid rgba(0, 255, 65, 0.3); /* HIGHLIGHT */
    border-bottom: 1px solid rgba(0, 0, 0, 0.8);
    border-radius: 16px;
    box-shadow:
      0 20px 50px rgba(0, 0, 0, 0.7),
      0 0 20px rgba(0, 255, 65, 0.05); /* GLOW */
    padding: 1.1rem;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }

  /* Glass Reflection Shine */
  .glass-panel::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.6),
      /* Brighter shine */ transparent
    );
    opacity: 0.7;
  }

  .panel-title {
    margin: 0 0 1.1rem 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.85); /* Whiter title */
    letter-spacing: 2px;
    padding-bottom: 0.8rem;
    display: flex;
    justify-content: space-between;
    font-weight: 600;

    /* THE ETCHED EFFECT ENHANCED */
    text-shadow: 0 2px 4px rgba(0, 0, 0, 1);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.3);
  }
  .hint {
    font-size: 0.7rem;
    text-transform: none;
    opacity: 0.5;
    font-weight: 400;
  }

  /* --- GLOBAL VIEW TRANSITION --- */
  .fade-in {
    animation: viewFadeIn 0.25s ease-out forwards;
  }
  @keyframes viewFadeIn {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* --- DASHBOARD GRID --- */
  /* --- DASHBOARD GRID --- */
  .view-dashboard {
    flex: 1; /* GROWS to fill vertical void */
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
    height: 100%; /* Ensure it takes full height */
    min-height: 0;
    overflow: hidden; /* Prevent global scroll */
    padding-right: 4px; /* Space for scrollbar */
    animation: viewFadeIn 0.25s ease-out forwards;
  }

  .row-top {
    flex: 0 0 auto; /* Allow flexible height */
    display: flex;
    gap: 1.2rem;
    /* height: 30vh;  Removed restrictive height */
    /* max-height: 210px; Removed restrictive cap */
    min-height: 130px; /* ULTRA COMPACT (was 160px) */
    transition: height 0.2s;
  }
  .row-bottom {
    flex: 1; /* GROWS to take remaining space */
    display: flex;
    min-height: 0; /* Critical for scrolling */
  }
  .activity-card {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0; /* Allow shrinking to viewport */
    /* max-height: 390px; REMOVED CAP for growth */
    overflow: hidden; /* Container doesn't overflow */
    transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1); /* PRO TRANSITION */
  }

  /* --- ACTIVITY EXPANSION OVERLAY --- */
  /* --- ACTIVITY EXPANSION OVERLAY --- */
  .activity-card.expanded {
    position: fixed; /* Break out of grid */
    top: 0; /* Cover EVERYTHING including header */
    left: 0;
    right: 0; /* Use right: 0 instead of width: 100vw to avoid scrollbar overflow */
    bottom: 0;
    z-index: 9999; /* Ensure top of stack */
    margin: 0.5rem; /* Tiny margin to show rounded corners against edge */
    border-radius: 12px; /* RADIUS ADDED */
    border: none;
    background: rgba(0, 0, 0, 0.85); /* Semi-transparent for blur */
    backdrop-filter: blur(20px); /* GLASS EFFECT */
    padding: 1.5rem; /* Better framing */
    box-sizing: border-box; /* SAFETY: Include padding/border in width */
    border: 1px solid rgba(0, 255, 65, 0.3); /* GREEN BORDER */
    box-shadow: 0 0 50px rgba(0, 0, 0, 0.8); /* Deep shadow for float effect */
  }
  .activity-card.expanded .panel-header {
    background: transparent;
    border-bottom: 1px solid rgba(0, 255, 65, 0.2);
    padding-right: 2rem; /* FIX CUTOFF TEXT */
  }

  .btn-expand {
    margin-left: 8px;
    color: var(--color-primary);
    font-size: 0.8rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .expand-icon {
    font-family: Arial, sans-serif; /* simpler font for arrow */
    font-size: 0.7rem;
  }

  /* --- CYBER PANELS (The Hybrid) --- */
  .cyber-panel {
    /* Base Glass Panel traits inherited from global .glass-panel if redundant, 
       but we make them distinct here to match the HUD vibe */
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 0; /* Header/Footer/Content split */
    border: 1px solid rgba(0, 255, 65, 0.2);
    background: rgba(8, 12, 10, 0.85);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
  }

  .wallet-card {
    flex: 1.4;
  }
  .node-card {
    flex: 1;
  }

  /* STATUS LED DOT */
  .stat-pill {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .stat-pill .value {
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 1px;
    color: var(--color-primary);
  }
  .led {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    transition: all 0.3s ease;
  }
  .led.running {
    background: #00ff41;
    box-shadow:
      0 0 8px #00ff41,
      0 0 16px rgba(0, 255, 65, 0.4);
    animation: pulse-green 2s ease-in-out infinite;
  }
  .led.stopped {
    background: #ff3333;
    box-shadow:
      0 0 6px #ff3333,
      0 0 12px rgba(255, 51, 51, 0.3);
  }
  @keyframes pulse-green {
    0%,
    100% {
      box-shadow:
        0 0 8px #00ff41,
        0 0 16px rgba(0, 255, 65, 0.4);
    }
    50% {
      box-shadow:
        0 0 12px #00ff41,
        0 0 24px rgba(0, 255, 65, 0.6);
    }
  }

  /* PANEL HEADER */
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.8rem 1.2rem;
    background: rgba(0, 0, 0, 0.4);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05); /* Removed flex-shrink: 0, it is default in flex items but can be explicit for safety */
    flex-shrink: 0; /* SAFEMODE: HEADER ALWAYS VISIBLE */
  }
  .hud-title {
    color: var(--color-muted);
    font-size: 0.75rem;
    letter-spacing: 1px;
    opacity: 0.8;
  }

  /* WINDOW CONTROLS */
  .status-stack {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    position: relative; /* Anchor for absolute text */
  }
  .traffic-lights {
    display: flex;
    gap: 8px;
  }

  .network-warning-text {
    position: absolute;
    top: 100%; /* Push directly below the lights */
    right: 50%; /* Center relative to stack */
    transform: translateX(50%); /* Correct centering */
    margin-top: 6px; /* Lowered slightly */
    color: #ff3333;
    font-size: 0.5rem;
    font-weight: normal;
    letter-spacing: 1px;
    text-transform: uppercase;
    text-shadow: 0 0 5px rgba(255, 50, 50, 0.6);
    animation: pulse-red 2s infinite;
    white-space: nowrap;
    pointer-events: none; /* Let clicks pass through */
  }

  @keyframes pulse-red {
    0% {
      opacity: 0.7;
    }
    50% {
      opacity: 1;
      text-shadow: 0 0 8px rgba(255, 0, 0, 0.8);
    }
    100% {
      opacity: 0.7;
    }
  }

  .network-badge {
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-weight: bold;
    letter-spacing: 1px;
    margin-right: 1rem;
    border: 1px solid rgba(255, 255, 255, 0.2);
  }
  .network-badge.regtest {
    background: rgba(0, 191, 255, 0.2);
    color: #00bfff;
    border-color: #00bfff;
    box-shadow: 0 0 10px rgba(0, 191, 255, 0.2);
  }

  /* PANEL CONTENT */
  .panel-content {
    flex: 1;
    padding: 0.5rem 1rem; /* ULTRA COMPACT PADDING (was 0.8rem) */
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: center; /* Center content vertically */
    /* padding-bottom removed, rely on flex gap */
  }

  /* COMPACT WALLET TWEAK */
  .compact-wallet .balance-hero-small .val {
    font-size: 2.5rem; /* Slightly smaller if needed, but flex gap handles it */
    margin-bottom: -4px; /* Tighten line height gap */
  }

  /* PERSISTENT VIEWS */
  .view-wrapper {
    display: none;
    flex: 1;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0; /* Crucial for nested scroll */
    animation: fade-in 0.2s ease-out;
    will-change: transform, opacity;
  }
  .view-wrapper.show {
    display: flex;
  }
  .wallet-content {
    justify-content: center;
    align-items: center;
    text-align: center;
  }

  /* PANEL ACTIONS */
  .panel-actions {
    padding: 0.8rem 1.2rem;
    background: rgba(0, 0, 0, 0.2);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    gap: 0.8rem;
  }
  .panel-actions.right {
    justify-content: flex-end;
  }

  /* SMALL ACTION BUTTONS */
  .btn-xs {
    padding: 0.5rem 1rem;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 1px;
    text-transform: uppercase;
    background: rgba(0, 255, 65, 0.15);
    border: 1px solid var(--color-primary);
    color: var(--color-primary);
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s ease;
  }
  .btn-xs:hover {
    background: rgba(0, 255, 65, 0.25);
    box-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
  }
  .btn-xs.ghost {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.2);
    color: var(--color-muted);
  }
  .btn-xs.ghost:hover {
    border-color: #ff4444;
    color: #ff4444;
    box-shadow: 0 0 8px rgba(255, 68, 68, 0.3);
  }
  .btn-xs.disabled,
  .btn-xs:disabled {
    background: rgba(100, 100, 100, 0.2);
    border-color: #555;
    color: #666;
    cursor: not-allowed;
    box-shadow: none;
  }
  .btn-xs.disabled:hover,
  .btn-xs:disabled:hover {
    background: rgba(100, 100, 100, 0.2);
    box-shadow: none;
  }

  /* --- NODE CARD STATS --- */
  .stat-grid-compact {
    display: flex;
    flex-direction: column;
    gap: 0.5rem; /* TIGHT STATS GAP (was 1rem) */
    justify-content: center;
    height: 100%;
  }
  .stat-pair {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    padding-bottom: 0.5rem;
  }
  .stat-pair .label {
    font-size: 0.7rem;
    color: var(--color-muted);
    letter-spacing: 1px;
  }
  .stat-pair .mono {
    font-size: 1rem;
    color: #fff;
  }

  /* --- WALLET HERO SMALL --- */
  .balance-hero-small .val {
    font-size: 1.8rem; /* ULTRA COMPACT (was 2.5rem) */
    font-weight: 700;
    font-family: var(--font-mono);
    color: #fff;
    line-height: 1;
    margin-bottom: -2px; /* Tighten Gap */
  }
  .balance-hero-small .unit {
    font-size: 1rem;
    color: var(--color-primary);
  }
  .balance-hero-small .sub {
    font-size: 0.65rem;
    color: var(--color-muted);
    color: var(--color-muted);
    letter-spacing: 2px;
    margin-top: 0.2rem; /* TIGHT MARGIN */
  }
  .wallet-metrics {
    display: flex;
    gap: 1rem;
    font-size: 0.7rem;
    color: var(--color-muted);
    margin-top: 1rem;
    opacity: 0.7;
  }

  /* --- ACTIVITY LIST --- */
  .header-row {
    display: grid;
    grid-template-columns: 120px 100px 150px 80px 1fr;
    padding: 0.6rem 1.2rem;
    border-bottom: 1px solid rgba(0, 255, 65, 0.15);
    color: var(--color-muted);
    font-size: 0.7rem;
    text-transform: uppercase;
    font-weight: 600;
    background: rgba(0, 255, 65, 0.02);
  }
  .scroll-body {
    flex: 1;
    overflow-y: scroll; /* Force track visibility */
    padding: 0;
    min-height: 0;
    /* Optional: Right border to separate scrollbar area */
    border-right: 1px solid rgba(255, 255, 255, 0.02);
  }

  /* WebKit Scrollbar (Chrome/Edge/WebView2) */
  .scroll-body::-webkit-scrollbar {
    width: 8px; /* Slightly wider for visibility */
    height: 8px; /* Horizontal scrollbar if needed */
  }
  .scroll-body::-webkit-scrollbar-track {
    background: rgba(0, 255, 65, 0.06); /* VISIBLE TRACK (Faint Green) */
    border-left: 1px solid rgba(0, 255, 65, 0.1); /* Separator line */
  }
  .scroll-body::-webkit-scrollbar-thumb {
    background: rgba(0, 255, 65, 0.3);
    border: 1px solid rgba(0, 255, 65, 0.1);
    border-radius: 0; /* Boxy tech look */
  }
  .scroll-body::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 255, 65, 0.6);
    box-shadow: 0 0 10px rgba(0, 255, 65, 0.4);
  }
  .data-row {
    display: grid;
    grid-template-columns: 120px 100px 150px 80px 1fr;
    padding: 0.8rem 1.2rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    font-size: 0.9rem;
    align-items: center;
    transition: background 0.2s;
  }
  .data-row:hover {
    background: rgba(0, 255, 65, 0.05);
  }

  .status-chip {
    font-size: 0.7rem;
    font-weight: bold;
    padding: 0.2rem 0.6rem;
    border-radius: 4px;
    border: 1px solid var(--color-primary);
    color: var(--color-primary);
    transition: all 0.3s ease;
  }
  .status-chip.status-red {
    color: #ff4444;
    border-color: rgba(255, 68, 68, 0.5);
    background: rgba(255, 68, 68, 0.1);
    box-shadow: 0 0 10px rgba(255, 68, 68, 0.2);
  }
  .status-chip.status-green {
    color: #00ff41;
    border-color: rgba(0, 255, 65, 0.5);
    background: rgba(0, 255, 65, 0.1);
    box-shadow: 0 0 10px rgba(0, 255, 65, 0.2);
  }

  /* --- BUTTONS --- */
  .btn-xs {
    padding: 0.3rem 0.8rem;
    font-size: 0.7rem;
  }
  .btn-xs.ghost {
    border-color: rgba(255, 255, 255, 0.2);
  }

  .amount.pos {
    color: var(--color-primary);
    text-shadow: 0 0 5px rgba(0, 255, 65, 0.3);
  }
  .amount.neg {
    color: #ff8888;
  }

  .dim {
    color: var(--color-muted);
    font-size: 0.9rem;
  }
  .txid {
    color: #888;
    font-size: 0.85rem;
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error-line {
    margin-top: 0.5rem;
    color: var(--color-danger);
    font-size: 0.75rem;
    opacity: 0.85;
  }

  /* --- ANIMATION --- */
  .fade-in {
    animation: fadeIn 0.4s ease-out;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .coming-soon {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    border: 1px dashed rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    font-family: var(--font-mono);
    color: var(--color-muted);
  }

  /* --- FOOTER LINK --- */
  .app-footer {
    text-align: center;
    padding: 0.5rem 0;
    margin-top: auto;
    z-index: 20;
    flex-shrink: 0;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(6px);
    z-index: 100;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(420px, 90vw);
    background: rgba(8, 12, 10, 0.98); /* DARKER & OPAQUE */
    border: 1px solid rgba(0, 255, 65, 0.4); /* NEON BORDER */
    border-radius: 16px;
    padding: 1.25rem 1.75rem; /* Slightly more padding */
    box-shadow:
      0 0 50px rgba(0, 255, 65, 0.15),
      /* OUTER GLOW */ 0 30px 80px rgba(0, 0, 0, 0.9); /* DEPTH SHADOW */
    z-index: 101;
  }
  .modal-title {
    margin: 0 0 0.5rem 0;
    color: #fff;
    letter-spacing: 1px;
  }
  .modal-text {
    margin: 0 0 0.75rem 0;
    color: var(--color-muted);
    font-size: 0.85rem;
  }
  .modal-label {
    display: block;
    margin: 0.5rem 0 0.35rem 0;
    color: var(--color-muted);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  .modal-input {
    width: 100%;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #fff;
    padding: 0.6rem 0.75rem;
    border-radius: 10px;
    outline: none;
  }
  .modal-input:focus {
    border-color: var(--color-primary);
    box-shadow: 0 0 12px rgba(0, 255, 65, 0.15);
  }
  .modal-error {
    margin-top: 0.75rem;
    color: var(--color-danger);
    font-size: 0.8rem;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1rem;
  }
  .btn-sm.ghost {
    background: transparent;
    color: var(--color-muted);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }
  .footer-link {
    font-family: var(--font-mono);
    color: var(--color-primary-dim);
    font-size: 0.75rem;
    text-decoration: none;
    letter-spacing: 1px;
    opacity: 0.6;
    transition: all 0.2s;
  }
  .footer-link:hover {
    color: var(--color-primary);
    text-shadow: 0 0 12px rgba(0, 255, 65, 0.8); /* Stronger hover glow */
    opacity: 1;
  }

  /* --- WELCOME POPUP --- */
  .welcome-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    animation: fadeIn 0.3s ease-out;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .welcome-modal {
    background: linear-gradient(
      145deg,
      rgba(10, 10, 10, 0.97) 0%,
      rgba(5, 5, 5, 0.99) 100%
    );
    border: 1px solid rgba(0, 255, 65, 0.25);
    border-radius: 12px;
    box-shadow:
      0 0 40px rgba(0, 255, 65, 0.15),
      0 0 80px rgba(0, 0, 0, 0.8);
    max-width: 480px;
    width: 90%;
    animation: slideUp 0.3s ease-out;
  }
  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .welcome-header {
    padding: 1.5rem 1.5rem 1rem;
    border-bottom: 1px solid rgba(0, 255, 65, 0.1);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .welcome-header h2 {
    margin: 0;
    color: var(--color-primary);
    font-size: 1.1rem;
    font-weight: 600;
    letter-spacing: 1px;
  }
  .welcome-version {
    color: #555;
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .welcome-body {
    padding: 1.5rem;
  }
  .welcome-text {
    color: #ccc;
    font-size: 0.95rem;
    margin: 0 0 1rem 0;
    line-height: 1.5;
  }
  .welcome-text strong {
    color: #ff6b6b;
  }
  .welcome-caution {
    color: #ffcc00;
    font-size: 0.9rem;
    margin: 0 0 1rem 0;
    padding: 0.8rem;
    background: rgba(255, 204, 0, 0.08);
    border: 1px solid rgba(255, 204, 0, 0.2);
    border-radius: 6px;
  }
  .welcome-caution strong {
    color: var(--color-primary);
  }
  .welcome-disclaimer {
    color: #888;
    font-size: 0.85rem;
    margin: 0;
    font-style: italic;
  }
  .welcome-footer {
    padding: 1rem 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .welcome-checkbox {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #888;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .welcome-checkbox input {
    width: 16px;
    height: 16px;
    accent-color: var(--color-primary);
  }
  .welcome-btn {
    background: linear-gradient(135deg, var(--color-primary) 0%, #00cc33 100%);
    border: none;
    color: #000;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    font-weight: 700;
    padding: 0.8rem 1.5rem;
    border-radius: 6px;
    cursor: pointer;
    letter-spacing: 1px;
    transition: all 0.2s;
    box-shadow: 0 0 20px rgba(0, 255, 65, 0.3);
  }
  .welcome-btn:hover {
    transform: scale(1.02);
    box-shadow: 0 0 30px rgba(0, 255, 65, 0.5);
  }

  /* --- ABOUT PAGE --- */
  .about-page {
    flex: 1; /* KEY FIX: Fill available space */
    height: 100%; /* Ensure full height logic applies */
    display: flex;
    flex-direction: column;
    overflow-y: auto; /* Internal scroll ONLY */
    padding: 1.5rem;
    padding-bottom: 5rem; /* KEY FIX: Extra padding to prevent border cutoff */
    margin: 4px; /* KEY FIX: Pull in from edges to show border/shadow */
    min-height: 0; /* Allow shrinking */
  }
  .about-header {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid rgba(0, 255, 65, 0.1);
    margin-bottom: 1.5rem;
  }
  .about-logo {
    width: 100px;
    height: 100px;
    border-radius: 16px;
    box-shadow: 0 0 30px rgba(0, 255, 65, 0.2);
    object-fit: contain;
  }
  .about-title-block {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .about-title {
    margin: 0;
    color: white;
    font-size: 1.5rem;
    letter-spacing: 3px;
  }
  .about-version {
    color: var(--color-primary);
    font-family: var(--font-mono);
    font-size: 0.9rem;
  }
  .welcome-toggle {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #888;
    font-size: 0.75rem;
    cursor: pointer;
  }
  .welcome-toggle input {
    width: 14px;
    height: 14px;
    accent-color: var(--color-primary);
  }
  .welcome-toggle:hover {
    color: #aaa;
  }
  .about-section {
    margin-bottom: 1.5rem;
  }
  .section-header {
    color: var(--color-primary);
    font-size: 0.85rem;
    margin: 0 0 0.8rem 0;
    letter-spacing: 2px;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .about-text {
    color: #bbb;
    font-size: 0.9rem;
    line-height: 1.6;
    margin: 0 0 0.8rem 0;
  }
  .about-text strong {
    color: var(--color-primary);
  }
  .about-credit {
    color: #888;
    font-size: 0.85rem;
    line-height: 1.5;
    margin: 0;
  }
  .about-credit strong {
    color: #aaa;
  }
  .disclaimer-section {
    background: rgba(255, 100, 100, 0.03);
    border: 1px solid rgba(255, 100, 100, 0.1);
    border-radius: 8px;
    padding: 1rem;
  }
  .disclaimer-text {
    color: #ccc;
    font-size: 0.85rem;
    line-height: 1.5;
    margin: 0 0 0.6rem 0;
  }
  .disclaimer-text .danger {
    color: #ff6b6b;
  }
  .disclaimer-list {
    margin: 0;
    padding-left: 1.5rem;
    color: #aaa;
    font-size: 0.85rem;
    line-height: 1.8;
  }
  .disclaimer-list li {
    margin-bottom: 0.3rem;
  }
  .disclaimer-list strong {
    color: #ffcc00;
  }
  .settings-section {
    background: rgba(0, 255, 65, 0.02);
    border: 1px solid rgba(0, 255, 65, 0.1);
    border-radius: 8px;
    padding: 1rem;
  }
  .about-footer {
    margin-top: auto;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .about-link {
    color: var(--color-primary);
    text-decoration: none;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    transition: opacity 0.2s;
  }
  .about-link:hover {
    opacity: 0.8;
    text-decoration: underline;
  }
  /* --- HEADER RESPONSIVENESS --- */
  .status-stack {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-shrink: 0; /* KEY FIX: Never crush lights */
  }

  @media (max-width: 800px) {
    .app-title {
      font-size: 0.9rem;
    }
    .version {
      display: none;
    }
    .brand-info {
      display: flex;
      flex-direction: column;
      justify-content: center;
    }
    /* Responsive Activity Grid */
    .header-row,
    .data-row {
      grid-template-columns: 90px 80px 120px 60px 1fr;
      font-size: 0.8rem;
      padding: 0.5rem 0.8rem;
    }
    .row-top {
      flex-direction: column;
      gap: 0.8rem;
      min-height: auto;
    }
    .node-card,
    .wallet-card {
      flex: none;
    }
  }

  @media (max-width: 600px) {
    .header-row,
    .data-row {
      grid-template-columns: 70px 70px 100px 50px 1fr;
      font-size: 0.7rem;
      padding: 0.4rem 0.6rem;
    }
    .tab-btn {
      padding: 0.5rem 0.6rem;
      font-size: 0.7rem;
    }
    .logo {
      height: 32px;
      width: 32px;
    }
  }

  @media (max-height: 700px) {
    .top-bar {
      height: 60px;
      padding: 0 1rem;
    }
    .row-top {
      min-height: 100px;
    }
    .panel-header {
      padding: 0.5rem 0.8rem;
    }
    .panel-content {
      padding: 0.3rem 0.8rem;
    }
    .stat-pair {
      padding-bottom: 0.3rem;
    }
    .balance-hero-small .val {
      font-size: 1.5rem;
    }
  }
</style>
