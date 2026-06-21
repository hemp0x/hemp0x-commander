<script>
    import { createEventDispatcher } from "svelte";
    import {
        firstDefined,
        formatAmount,
        formatDate,
        formatInteger,
        readAddress,
    } from "./utils.js";

    export let transaction = {};

    const dispatch = createEventDispatcher();

    $: txid = String(firstDefined(transaction, ["txid", "id", "hash"], ""));
    $: inputs = firstDefined(transaction, ["inputs", "vin"], []) || [];
    $: outputs = firstDefined(transaction, ["outputs", "vout"], []) || [];
    $: confirmations = firstDefined(transaction, ["confirmations", "confirm_count"], 0);
    $: timestamp = firstDefined(transaction, ["time", "timestamp", "blocktime", "block_time"]);
    $: blockHash = firstDefined(transaction, ["blockhash", "block_hash"], "");
    $: size = firstDefined(transaction, ["size"]);
    $: vsize = firstDefined(transaction, ["vsize"]);
    $: source = firstDefined(transaction, ["source"], "--");
    $: status = Number(confirmations) > 0 ? "CONFIRMED" : "MEMPOOL";

    function navigate(type, target) {
        if (target) dispatch("navigate", { type, target: String(target) });
    }

    function inputTxid(input) {
        return String(firstDefined(input, ["txid", "transaction_id", "prev_txid"], ""));
    }

    function inputIndex(input) {
        return firstDefined(input, ["vout", "output_index", "n"], null);
    }

    function outputIndex(output, index) {
        return firstDefined(output, ["n", "vout", "index"], index);
    }

    function amountOf(item) {
        return firstDefined(item, ["value"], null);
    }

    function assetsOf(item) {
        return Array.isArray(item?.assets) ? item.assets : [];
    }

    async function copy(value, label) {
        try {
            await navigator.clipboard.writeText(String(value));
            dispatch("copied", { label });
        } catch {
            dispatch("copyerror");
        }
    }
</script>

<article class="transaction-view" aria-label="Transaction details">
    <header class="result-header">
        <div>
            <span class="eyebrow mono">TRANSACTION</span>
            <div class="identity-row">
                <button
                    type="button"
                    class="identity-link mono"
                    on:click={() => navigate("transaction", txid)}
                    title="Open transaction"
                >
                    {txid || "--"}
                </button>
                <button
                    type="button"
                    class="icon-btn"
                    on:click={() => copy(txid, "TXID")}
                    title="Copy transaction ID"
                    aria-label="Copy transaction ID"
                >
                    &#x2398;
                </button>
            </div>
        </div>
        <span class:pending={status === "MEMPOOL"} class="status-badge">
            <span class="status-dot"></span>{status}
        </span>
    </header>

    <section class="summary-strip" aria-label="Transaction summary">
        <div class="metric">
            <span>CONFIRMATIONS</span>
            <strong>{formatInteger(confirmations)}</strong>
        </div>
        <div class="metric">
            <span>TIME</span>
            <strong>{formatDate(timestamp)}</strong>
        </div>
        <div class="metric">
            <span>SIZE</span>
            <strong>{size == null ? "--" : `${formatInteger(size)} B`}</strong>
        </div>
        <div class="metric">
            <span>VIRTUAL SIZE</span>
            <strong>{vsize == null ? "--" : `${formatInteger(vsize)} vB`}</strong>
        </div>
    </section>

    <section class="metadata" aria-label="Block metadata">
        <div>
            <span>LOOKUP SOURCE</span>
            <strong class="mono">{source}</strong>
        </div>
        <div>
            <span>BLOCK HASH</span>
            <span class="hash-line">
                <strong class="mono">{blockHash || "--"}</strong>
                {#if blockHash}
                    <button
                        type="button"
                        class="icon-btn quiet"
                        on:click={() => copy(blockHash, "Block hash")}
                        title="Copy block hash"
                        aria-label="Copy block hash"
                    >
                        &#x2398;
                    </button>
                {/if}
            </span>
        </div>
    </section>

    <div class="flow-grid">
        <section class="flow-section">
            <header class="section-heading">
                <h3>INPUTS</h3>
                <span>{inputs.length}</span>
            </header>
            {#if inputs.some((input) => !input.coinbase && amountOf(input) == null)}
                <p class="input-value-note">
                    Input values belong to previous outputs and are not included
                    in the decoded transaction. Older source transactions may
                    also be unavailable on pruned nodes.
                </p>
            {/if}
            <div class="flow-list">
                {#if inputs.length === 0}
                    <div class="empty-row">No input records returned.</div>
                {:else}
                    {#each inputs as input, index}
                        {@const address = readAddress(input)}
                        {@const previousTxid = inputTxid(input)}
                        <div class="flow-row">
                            <div class="row-index mono">
                                {input.coinbase ? "CB" : String(index).padStart(2, "0")}
                            </div>
                            <div class="row-main">
                                {#if input.coinbase}
                                    <span class="source-label">COINBASE</span>
                                    <span class="mono muted break">{input.coinbase}</span>
                                {:else}
                                    {#if address}
                                        <button
                                            type="button"
                                            class="data-link mono"
                                            on:click={() => navigate("address", address)}
                                            title={address}
                                        >
                                            {address}
                                        </button>
                                    {:else}
                                        <span class="muted">Address unavailable</span>
                                    {/if}
                                    {#if previousTxid}
                                        <button
                                            type="button"
                                            class="sub-link mono"
                                            on:click={() => navigate("transaction", previousTxid)}
                                            title={previousTxid}
                                        >
                                            {previousTxid.slice(0, 14)}...:{inputIndex(input) ?? "--"}
                                        </button>
                                    {/if}
                                    {#if input.sequence != null}
                                        <span class="sequence mono">
                                            SEQ {input.sequence}
                                        </span>
                                    {/if}
                                {/if}
                            </div>
                            <div class="row-value">
                                {#if amountOf(input) != null}
                                    <strong>{formatAmount(amountOf(input))}</strong>
                                    <span>HEMP</span>
                                {/if}
                                {#each assetsOf(input) as asset}
                                    <strong>{formatAmount(asset.amount)}</strong>
                                    <span>{asset.name || "ASSET"}</span>
                                {/each}
                                {#if amountOf(input) == null && assetsOf(input).length === 0}
                                    <span>VALUE UNAVAILABLE</span>
                                {/if}
                            </div>
                        </div>
                    {/each}
                {/if}
            </div>
        </section>

        <section class="flow-section">
            <header class="section-heading">
                <h3>OUTPUTS</h3>
                <span>{outputs.length}</span>
            </header>
            <div class="flow-list">
                {#if outputs.length === 0}
                    <div class="empty-row">No output records returned.</div>
                {:else}
                    {#each outputs as output, index}
                        {@const address = readAddress(output)}
                        <div class="flow-row">
                            <div class="row-index mono">
                                {String(outputIndex(output, index)).padStart(2, "0")}
                            </div>
                            <div class="row-main">
                                {#if address}
                                    <button
                                        type="button"
                                        class="data-link mono"
                                        on:click={() => navigate("address", address)}
                                        title={address}
                                    >
                                        {address}
                                    </button>
                                {:else}
                                    <span class="muted">Non-address output</span>
                                {/if}
                                <span class="output-state">
                                    {output.scriptType || "SCRIPT TYPE UNKNOWN"}
                                </span>
                            </div>
                            <div class="row-value">
                                {#if amountOf(output) != null}
                                    <strong>{formatAmount(amountOf(output))}</strong>
                                    <span>HEMP</span>
                                {/if}
                                {#each assetsOf(output) as asset}
                                    <strong>{formatAmount(asset.amount)}</strong>
                                    <span>{asset.name || "ASSET"}</span>
                                {/each}
                            </div>
                        </div>
                    {/each}
                {/if}
            </div>
        </section>
    </div>
</article>

<style>
    .transaction-view {
        min-width: 0;
        color: var(--color-highlight, #e0ffee);
    }

    .result-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 1rem;
        padding: 1.1rem 1.25rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.16);
        background: rgba(0, 0, 0, 0.34);
    }

    .eyebrow,
    .metric span,
    .metadata > div > span:first-child {
        display: block;
        margin-bottom: 0.38rem;
        color: rgba(255, 255, 255, 0.42);
        font-size: 0.64rem;
        font-weight: 700;
        letter-spacing: 1px;
    }

    .identity-row,
    .hash-line {
        display: flex;
        min-width: 0;
        align-items: center;
        gap: 0.45rem;
    }

    .identity-link,
    .data-link,
    .sub-link {
        min-width: 0;
        padding: 0;
        overflow: hidden;
        border: 0;
        border-radius: 0;
        background: transparent;
        box-shadow: none;
        color: var(--color-highlight, #e0ffee);
        font-size: 0.78rem;
        font-weight: 500;
        letter-spacing: 0;
        text-align: left;
        text-overflow: ellipsis;
        text-transform: none;
        white-space: nowrap;
    }

    .identity-link {
        max-width: min(62vw, 660px);
        color: var(--color-primary, #00ff41);
        font-size: 0.88rem;
    }

    .identity-link:hover,
    .data-link:hover,
    .sub-link:hover {
        color: var(--color-primary, #00ff41);
        background: transparent;
        box-shadow: none;
        transform: none;
        text-decoration: underline;
    }

    .icon-btn {
        display: inline-grid;
        width: 1.8rem;
        height: 1.8rem;
        flex: 0 0 auto;
        place-items: center;
        padding: 0;
        border-color: rgba(0, 255, 65, 0.18);
        color: var(--color-primary, #00ff41);
        font-size: 0.9rem;
        letter-spacing: 0;
    }

    .icon-btn.quiet {
        width: 1.55rem;
        height: 1.55rem;
    }

    .status-badge {
        display: inline-flex;
        align-items: center;
        gap: 0.42rem;
        flex: 0 0 auto;
        padding: 0.34rem 0.55rem;
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 4px;
        background: rgba(0, 255, 65, 0.06);
        color: var(--color-primary, #00ff41);
        font: 700 0.63rem var(--font-mono, monospace);
        letter-spacing: 0.7px;
    }

    .status-badge.pending {
        border-color: rgba(0, 204, 255, 0.28);
        background: rgba(0, 204, 255, 0.06);
        color: var(--color-process, #00ccff);
    }

    .status-dot {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: currentColor;
        box-shadow: 0 0 6px currentColor;
    }

    .summary-strip {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    }

    .metric {
        min-width: 0;
        padding: 0.85rem 1rem;
        border-right: 1px solid rgba(255, 255, 255, 0.07);
    }

    .metric:last-child {
        border-right: 0;
    }

    .metric strong {
        display: block;
        overflow: hidden;
        color: #fff;
        font: 600 0.78rem var(--font-mono, monospace);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .metadata {
        display: grid;
        grid-template-columns: minmax(130px, 0.25fr) minmax(0, 1fr);
        gap: 1rem;
        padding: 0.8rem 1rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        background: rgba(0, 255, 65, 0.018);
    }

    .metadata strong {
        display: block;
        min-width: 0;
        overflow: hidden;
        color: rgba(255, 255, 255, 0.78);
        font-size: 0.72rem;
        font-weight: 500;
        letter-spacing: 0;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .flow-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 1px;
        background: rgba(0, 255, 65, 0.12);
    }

    .flow-section {
        min-width: 0;
        background: rgba(4, 7, 5, 0.98);
    }

    .section-heading {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.72rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    }

    .section-heading h3 {
        margin: 0;
        color: rgba(255, 255, 255, 0.72);
        font-size: 0.68rem;
        letter-spacing: 1px;
    }

    .section-heading span {
        color: var(--color-primary-dim, #008f11);
        font: 700 0.67rem var(--font-mono, monospace);
    }

    .flow-list {
        min-width: 0;
    }

    .input-value-note {
        margin: 0;
        padding: 0.55rem 1rem;
        border-bottom: 1px solid rgba(0, 204, 255, 0.1);
        background: rgba(0, 204, 255, 0.025);
        color: rgba(255, 255, 255, 0.46);
        font-size: 0.6rem;
        line-height: 1.45;
    }

    .flow-row {
        display: grid;
        grid-template-columns: 2rem minmax(0, 1fr) minmax(80px, auto);
        align-items: center;
        gap: 0.65rem;
        min-height: 3.65rem;
        padding: 0.62rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.055);
    }

    .flow-row:hover {
        background: rgba(0, 255, 65, 0.028);
    }

    .row-index {
        color: rgba(0, 255, 65, 0.55);
        font-size: 0.65rem;
    }

    .row-main,
    .row-value {
        display: flex;
        min-width: 0;
        flex-direction: column;
        gap: 0.28rem;
    }

    .row-value {
        align-items: flex-end;
        text-align: right;
    }

    .row-value strong {
        color: #fff;
        font: 600 0.76rem var(--font-mono, monospace);
    }

    .row-value span,
    .source-label,
    .output-state {
        color: rgba(255, 255, 255, 0.38);
        font-size: 0.58rem;
        font-weight: 700;
        letter-spacing: 0.7px;
    }

    .data-link {
        color: rgba(255, 255, 255, 0.82);
        font-size: 0.7rem;
    }

    .sub-link {
        color: rgba(255, 255, 255, 0.34);
        font-size: 0.61rem;
    }

    .output-state {
        color: rgba(0, 255, 65, 0.5);
    }

    .sequence {
        color: rgba(255, 255, 255, 0.25);
        font-size: 0.56rem;
    }

    .muted {
        color: rgba(255, 255, 255, 0.35);
        font-size: 0.67rem;
    }

    .break {
        overflow-wrap: anywhere;
    }

    .empty-row {
        padding: 1.4rem 1rem;
        color: rgba(255, 255, 255, 0.34);
        font-size: 0.7rem;
        text-align: center;
    }

    @media (max-width: 760px) {
        .summary-strip {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }

        .metric:nth-child(2) {
            border-right: 0;
        }

        .metric:nth-child(-n + 2) {
            border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        }

        .flow-grid {
            grid-template-columns: minmax(0, 1fr);
        }

        .identity-link {
            max-width: 58vw;
        }
    }

    @media (max-width: 470px) {
        .result-header {
            align-items: stretch;
            flex-direction: column;
        }

        .status-badge {
            align-self: flex-start;
        }

        .metadata {
            grid-template-columns: minmax(0, 1fr);
        }

        .identity-link {
            max-width: calc(100vw - 8.5rem);
        }

        .flow-row {
            grid-template-columns: 1.5rem minmax(0, 1fr);
        }

        .row-value {
            grid-column: 2;
            align-items: flex-start;
            flex-direction: row;
            gap: 0.4rem;
            text-align: left;
        }
    }
</style>
