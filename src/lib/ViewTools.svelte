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
    tauriReady = typeof core?.isTauri === "function" ? core.isTauri() : false;
  });

  onDestroy(() => {
    window.removeEventListener("commander-open-content-library", openLibraryFromPicker);
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

</style>
