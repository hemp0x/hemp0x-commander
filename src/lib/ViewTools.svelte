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
  import ToolsJournal from "./tools/ToolsJournal.svelte";
  import ToolsHistory from "./tools/ToolsHistory.svelte";
  import ToolsConsolidation from "./tools/ToolsConsolidation.svelte";
  import ToolsRawTx from "./tools/ToolsRawTx.svelte";
  import ToolsSoloMining from "./tools/ToolsSoloMining.svelte";
  import ContentLibraryPanel from "./content/ContentLibraryPanel.svelte";
  import IpfsHub from "./content/IpfsHub.svelte";
  import SystemHub from "./content/SystemHub.svelte";
  import CommanderLoader from "./ui/CommanderLoader.svelte";
  import { nodeStatus, daemonRuntime } from "../stores.js";
  import { addToastNotification } from "./stores/notifications.js";
  import { cidViewerTarget, ipfsHubSection } from "./stores/contentLibrary.js";
  import { systemHubSection } from "./stores/systemHub.js";

  let activeSubTab = "CONSOLE";
  let tauriReady = false;
  let ipfsOpenCid = null;
  let lastCidTarget = null;
  $: if ($cidViewerTarget && $cidViewerTarget !== lastCidTarget) {
      lastCidTarget = $cidViewerTarget;
      activeSubTab = "IPFS";
      ipfsHubSection.set("cid-viewer");
      ipfsOpenCid = $cidViewerTarget;
  }

  function openLibraryFromPicker() {
    activeSubTab = "IPFS";
    ipfsHubSection.set("library");
  }

  function openWalletFromAnywhere() {
    activeSubTab = "WALLET";
  }

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

  let isProcessing = false;
  let processingMessage = "Processing...";

  let showConfirmModal = false;
  let modalTitle = "";
  let modalMessage = "";
  let modalButtons = []; // { label, style, onClick }

  function openModal(title, message, buttons) {
    modalTitle = title;
    modalMessage = message;
    modalButtons = buttons;
    showConfirmModal = true;
  }

  function closeModal() {
    showConfirmModal = false;
  }

  onMount(() => {
    window.addEventListener("commander-open-content-library", openLibraryFromPicker);
    window.addEventListener("commander-open-tools-wallet", openWalletFromAnywhere);
    tauriReady = typeof core?.isTauri === "function" ? core.isTauri() : false;
  });

  onDestroy(() => {
    window.removeEventListener("commander-open-content-library", openLibraryFromPicker);
    window.removeEventListener("commander-open-tools-wallet", openWalletFromAnywhere);
    clearTimeout(toastTimer);
  });
</script>

<div class="view-tools">
  <div class="glass-panel panel-strong cyber-panel main-frame">
    <!-- HEADER / TABS -->
    <header class="panel-header no-border">
      <div class="sub-tabs">
        {#each ["CONSOLE", "WALLET", "SYSTEM", "HISTORY", "JOURNAL", "CONSOLIDATE", "RAW TX", "SOLO MINING", "IPFS"] as tab}
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
      class:no-scroll={activeSubTab === "CONSOLE"}
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
          {:else if activeSubTab === "SYSTEM"}
            <SystemHub on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)} />
          {:else if activeSubTab === "HISTORY"}
            <ToolsHistory
              on:toast={(e) => showToast(e.detail.msg, e.detail.type, e.detail.notify !== false)}
              on:switch={(e) => (activeSubTab = e.detail)}
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
          {:else if activeSubTab === "IPFS"}
            <IpfsHub openCid={ipfsOpenCid} />
          {/if}
        </div>
      {/key}
    </div>

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

</div>

{#if showConfirmModal}
  <div
    class="modal-overlay"
    role="button"
    tabindex="0"
    on:keydown={(e) => e.key === "Escape" && closeModal()}
  >
    <div
      class="modal-staged compact"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation={() => {}}
    >
      <div class="modal-header">
        <h3>{modalTitle}</h3>
      </div>
      <div class="modal-body">
        <p style="margin:0; color:#ccc; font-size:0.8rem; line-height:1.5;">
          {modalMessage}
        </p>
      </div>
      <div class="modal-actions">
        {#each modalButtons as btn}
          <button
            class="cyber-btn {btn.style === 'ghost'
              ? 'ghost'
              : ''} {btn.style === 'danger'
              ? 'danger ghost'
              : ''} {btn.style === 'primary' ? 'primary-glow' : ''}"
            on:click={btn.onClick}
          >
            {btn.label}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

<!-- PROCESSING OVERLAY -->
{#if isProcessing}
  <div class="loader-overlay">
    <div class="loader-panel">
      <div style="display:flex; justify-content:center; margin-bottom:0.75rem;">
        <CommanderLoader compact={true} label="" detail="" />
      </div>
      <h3>PLEASE WAIT</h3>
      <p>{processingMessage}</p>
      <p style="color:#666; font-size:0.7rem; margin-top:0.75rem; line-height:1.4;">
        App will respond once the command is done.
      </p>
    </div>
  </div>
{/if}

<style lang="css">
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
    gap: 0.4rem;
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
    top: 4.5rem;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(2, 4, 3, 0.98);
    border: 1px solid rgba(0, 255, 65, 0.3);
    padding: 0.7rem 1.1rem;
    border-radius: 6px;
    z-index: 2000000; /* Ensure above all modals */
    max-width: min(420px, 90vw);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.8);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: #ccc;
    pointer-events: none;
  }
  .toast-popup.error {
    border-color: rgba(255, 85, 85, 0.45);
    color: #ffaaaa;
  }
  .toast-popup.success {
    border-color: rgba(0, 255, 65, 0.45);
    color: #c8ffd0;
  }

  /* --- HEADER --- */
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(0, 0, 0, 0.45);
    border-bottom: 1px solid rgba(0, 255, 65, 0.08);
  }
  .sub-tabs {
    display: flex;
    gap: 2px;
    flex-wrap: wrap;
  }
  .sub-tab-btn {
    background: transparent;
    border: none;
    color: var(--color-muted);
    padding: 0.85rem 1.25rem;
    font-size: 0.78rem;
    letter-spacing: 1px;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    white-space: nowrap;
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
      rgba(0, 255, 65, 0.06) 100%
    );
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
  }

  /* --- BODY --- */
  .tools-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.5rem;
    position: relative;
    background: rgba(0, 0, 0, 0.2);
    display: flex;
    flex-direction: column;
  }
  .tools-body.no-scroll {
    overflow-y: hidden;
    padding-bottom: 0;
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
  }
  .cyber-btn:hover {
    background: var(--color-primary);
    color: #000;
    box-shadow: 0 0 10px rgba(0, 255, 65, 0.25);
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

</style>
