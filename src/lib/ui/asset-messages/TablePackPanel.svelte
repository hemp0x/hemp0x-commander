<script>
    import { createEventDispatcher } from "svelte";

    /**
     * @typedef {{
     *   name: string,
     *   version: string,
     *   fingerprint_sha256: string,
     *   origin?: "builtin"|"custom",
     *   active?: boolean,
     *   builtin?: boolean,
     *   path?: string|null
     * }} TablePackSummary
     */

    const dispatch = createEventDispatcher();

    /** @type {TablePackSummary[]} */
    export let packs = [];
    export let selectionFingerprint = "";
    /** @type {TablePackSummary | null} */
    export let selectedPack = null;
    export let busy = false;
    export let error = "";
    export let status = "";
    export let showExport = true;
    export let showImport = true;
    export let showReset = true;
    export let showDelete = true;
    export let noteText = "Export the official JSON, edit it, then import it as a custom pack. Token 255 must stay empty in every dictionary.";
    export let activePackLabel = "";
    export let activePackFingerprint = "";
    export let activePackFingerprintShort = "";
    export let activePackStatusTitle = "";

    /** @param {TablePackSummary | null | undefined} pack */
    function packLabel(pack) {
        if (!pack) return "Official HOXSHTV1.0";
        if (pack.builtin || pack.origin !== "custom") return pack.name;
        const version = pack.version && pack.version !== pack.name ? ` v${pack.version}` : "";
        return `Custom - ${pack.name}${version}`;
    }
</script>

<div class="table-pack-panel">
    <div class="table-pack-head">
        <span title={activePackStatusTitle}>Active: {activePackLabel}</span>
        {#if activePackFingerprintShort}
            <span class="mono" title={activePackFingerprint}>{activePackFingerprintShort}</span>
        {/if}
    </div>
    <div class="table-pack-controls">
        <select
            class="table-pack-select"
            style="background-color: #050706; color: #9cffad;"
            on:change={(e) => {
                const target = e.target;
                if (target instanceof HTMLSelectElement && target.value) {
                    const pack = packs.find(p => p.fingerprint_sha256 === target.value);
                    dispatch("selectPack", { fingerprint: target.value, pack });
                }
            }}
            disabled={busy}
            value={selectionFingerprint}
        >
            {#each packs as pack}
                <option value={pack.fingerprint_sha256}>
                    {packLabel(pack)}
                </option>
            {/each}
        </select>
        {#if showExport}
            <button type="button" on:click={() => dispatch("export")} disabled={busy}>EXPORT OFFICIAL</button>
        {/if}
        {#if showImport}
            <button type="button" on:click={() => dispatch("import")} disabled={busy}>IMPORT PACK</button>
        {/if}
        {#if showReset}
            <button type="button" on:click={() => dispatch("reset")} disabled={busy}>USE OFFICIAL</button>
        {/if}
        {#if showDelete}
            <button
                type="button"
                class="danger-action"
                on:click={() => dispatch("delete")}
                disabled={busy || !selectedPack || selectedPack.builtin}
            >DELETE</button>
        {/if}
    </div>
    <div class="table-pack-note">
        {noteText}
    </div>
    {#if status}
        <div class="table-pack-status" class:busy>{status}</div>
    {/if}
    {#if error}
        <div class="table-pack-error">{error}</div>
    {/if}
</div>

<style>
    .table-pack-panel {
        border: 1px solid rgba(0, 255, 65, 0.14);
        border-radius: 6px;
        background: rgba(0, 255, 65, 0.035);
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        margin-top: 0.35rem;
        padding: 0.55rem;
    }
    .table-pack-head {
        align-items: center;
        color: #b9ffd0;
        display: flex;
        flex-wrap: wrap;
        font-size: 0.68rem;
        gap: 0.5rem;
        justify-content: space-between;
    }
    .mono { font-family: var(--font-mono); font-size: 0.55rem; color: #999; }
    .table-pack-controls {
        display: grid;
        gap: 0.35rem;
        grid-template-columns: minmax(10rem, 0.8fr) repeat(4, auto);
    }
    .table-pack-controls select,
    .table-pack-controls button {
        background: rgba(0, 10, 4, 0.92);
        background-color: rgba(0, 10, 4, 0.92);
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        color: #cfd8cf;
        font-family: var(--font-mono);
        font-size: 0.58rem;
        min-height: 1.8rem;
        padding: 0.25rem 0.45rem;
    }
    .table-pack-controls select,
    .table-pack-select {
        appearance: none;
        -webkit-appearance: none;
        background: #050706 !important;
        background-color: #050706 !important;
        color: #9cffad !important;
        color-scheme: dark;
        max-width: 23rem;
        padding-right: 1.6rem;
    }
    .table-pack-controls select option,
    .table-pack-select option { background: #050706; color: var(--color-highlight); }
    .table-pack-controls button {
        color: var(--color-primary);
        cursor: pointer;
        font-weight: 700;
        letter-spacing: 0.6px;
    }
    .table-pack-controls button:hover:not(:disabled) { background: rgba(0, 255, 65, 0.1); }
    .table-pack-controls button.danger-action {
        border-color: rgba(255, 84, 84, 0.32);
        color: #ff8080;
    }
    .table-pack-controls button.danger-action:hover:not(:disabled) { background: rgba(255, 84, 84, 0.1); }
    .table-pack-controls button:disabled,
    .table-pack-controls select:disabled { cursor: not-allowed; opacity: 0.5; }
    .table-pack-note { color: #8d968d; font-size: 0.62rem; line-height: 1.35; }
    .table-pack-status {
        align-items: center;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 5px;
        color: #9cffad;
        display: flex;
        font-size: 0.62rem;
        gap: 0.45rem;
        margin-top: 0.35rem;
        padding: 0.35rem 0.45rem;
    }
    .table-pack-status.busy::before {
        animation: table-pack-spin 0.8s linear infinite;
        border: 2px solid rgba(0, 255, 65, 0.18);
        border-top-color: var(--color-primary);
        border-radius: 999px;
        content: "";
        display: inline-block;
        flex: 0 0 auto;
        height: 0.75rem;
        width: 0.75rem;
    }
    @keyframes table-pack-spin { to { transform: rotate(360deg); } }
    .table-pack-error {
        border: 1px solid rgba(255, 80, 80, 0.25);
        border-radius: 5px;
        color: #ff8a8a;
        font-size: 0.62rem;
        padding: 0.35rem 0.45rem;
    }
    @media (max-width: 880px) { .table-pack-controls { grid-template-columns: 1fr; } }
</style>
