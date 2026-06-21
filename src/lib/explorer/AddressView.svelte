<script>
    import { createEventDispatcher } from "svelte";
    import {
        firstDefined,
        formatAmount,
        formatDate,
        formatInteger,
    } from "./utils.js";

    export let addressData = {};
    export let address = "";

    const dispatch = createEventDispatcher();

    $: resolvedAddress = String(firstDefined(addressData, ["address"], address));
    $: balanceCapability =
        addressData?.balance && typeof addressData.balance === "object"
            ? addressData.balance
            : null;
    $: transactionCapability =
        addressData?.transactions &&
        !Array.isArray(addressData.transactions) &&
        typeof addressData.transactions === "object"
            ? addressData.transactions
            : null;
    $: balance = normalizeAtomicAmount(
        balanceCapability
            ? balanceCapability.balance
            : firstDefined(addressData, ["balance", "current_balance"], null),
        Boolean(balanceCapability),
    );
    $: received = normalizeAtomicAmount(
        balanceCapability
            ? balanceCapability.received
            : firstDefined(addressData, ["received", "total_received"], null),
        Boolean(balanceCapability),
    );
    $: sent =
        firstDefined(addressData, ["sent", "total_sent"], null) ??
        (received != null && balance != null ? received - balance : null);
    $: transactions = normalizeTransactions(
        transactionCapability?.txids ??
            firstDefined(addressData, ["transactions", "txs", "history"], []),
    );
    $: transactionIndexUnsupported =
        transactionCapability?.state === "unsupported" ||
        addressData?.addressTxidsSupported === false;
    $: balanceIndexUnsupported =
        balanceCapability?.state === "unsupported" ||
        addressData?.addressBalanceSupported === false;
    $: balanceIndexError = balanceCapability?.state === "error";
    $: transactionIndexError = transactionCapability?.state === "error";
    $: transactionCount = transactions.length;
    $: labels = Array.isArray(addressData?.labels) ? addressData.labels : [];

    function normalizeAtomicAmount(value, atomicUnits) {
        if (value === null || value === undefined || value === "") return null;
        const number = Number(value);
        if (!Number.isFinite(number)) return value;
        return atomicUnits ? number / 100_000_000 : number;
    }

    function normalizeTransactions(value) {
        if (!Array.isArray(value)) return [];
        return value.map((item) =>
            typeof item === "string" ? { txid: item } : item,
        );
    }

    function navigate(type, target) {
        if (target) dispatch("navigate", { type, target: String(target) });
    }

    function txidOf(tx) {
        if (typeof tx === "string") return tx;
        return String(firstDefined(tx, ["txid", "id", "hash"], ""));
    }

    function amountOf(tx) {
        return firstDefined(tx, ["amount", "value", "balance_delta", "net"], null);
    }

    function confirmationsOf(tx) {
        return firstDefined(tx, ["confirmations", "confirm_count"], 0);
    }

    function timeOf(tx) {
        return firstDefined(tx, ["time", "timestamp", "blocktime", "block_time"]);
    }

    function directionOf(tx) {
        const explicit = firstDefined(tx, ["direction", "type", "category"], "");
        if (explicit) return String(explicit).toUpperCase();
        const amount = Number(amountOf(tx));
        if (Number.isFinite(amount)) return amount < 0 ? "SENT" : "RECEIVED";
        return "TRANSFER";
    }

    function isOutgoing(tx) {
        const direction = directionOf(tx);
        return direction.includes("SEND") || direction.includes("OUT");
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

<article class="address-view" aria-label="Address details">
    <header class="result-header">
        <div class="identity-block">
            <span class="eyebrow mono">HEMP0X ADDRESS</span>
            <div class="identity-row">
                <button
                    type="button"
                    class="identity-link mono"
                    on:click={() => navigate("address", resolvedAddress)}
                    title="Open address"
                >
                    {resolvedAddress || "--"}
                </button>
                <button
                    type="button"
                    class="icon-btn"
                    on:click={() => copy(resolvedAddress, "Address")}
                    title="Copy address"
                    aria-label="Copy address"
                >
                    &#x2398;
                </button>
            </div>
        </div>
        <div class="balance-lockup">
            <span>CONFIRMED BALANCE</span>
            <strong>{formatAmount(balance)} <small>HEMP</small></strong>
        </div>
    </header>

    <div class="address-tags">
        <span class:invalid={addressData?.isValid === false}>
            {addressData?.isValid === false ? "INVALID" : "VALID ADDRESS"}
        </span>
        {#if addressData?.isMine}<span>WALLET OWNED</span>{/if}
        {#if addressData?.isWatchOnly}<span>WATCH ONLY</span>{/if}
        {#each labels as label}<span class="label-tag">{label}</span>{/each}
    </div>

    {#if addressData?.scriptPubKey}
        <div class="script-line">
            <span>SCRIPT PUBKEY</span>
            <strong class="mono">{addressData.scriptPubKey}</strong>
            <button
                type="button"
                class="copy-inline"
                on:click={() => copy(addressData.scriptPubKey, "Script pubkey")}
                title="Copy script pubkey"
                aria-label="Copy script pubkey"
            >
                &#x2398;
            </button>
        </div>
    {/if}

    {#if balanceIndexUnsupported || balanceIndexError}
        <div class:error={balanceIndexError} class="partial-notice">
            <span class="mono">{balanceIndexError ? "ERR" : "IDX"}</span>
            <p>
                {balanceIndexError
                    ? balanceCapability?.error || "Address balance lookup failed."
                    : "Balance and received totals require the local address index. Ownership metadata is still available."}
            </p>
        </div>
    {/if}

    <section class="summary-strip" aria-label="Address summary">
        <div class="metric">
            <span>TOTAL RECEIVED</span>
            <strong>{balanceIndexUnsupported ? "--" : formatAmount(received)} HEMP</strong>
        </div>
        <div class="metric">
            <span>TOTAL SENT</span>
            <strong>{balanceIndexUnsupported ? "--" : formatAmount(sent)} HEMP</strong>
        </div>
        <div class="metric">
            <span>TRANSACTIONS</span>
            <strong>{formatInteger(transactionCount)}</strong>
        </div>
    </section>

    <section class="history-section">
        <header class="section-heading">
            <h3>TRANSACTION HISTORY</h3>
            <span>{transactions.length} LOADED</span>
        </header>

        {#if transactionIndexUnsupported || transactionIndexError}
            <div class="index-state">
                <span class:error={transactionIndexError} class="state-code mono">
                    {transactionIndexError ? "ERR" : "IDX"}
                </span>
                <div>
                    <strong>
                        {transactionIndexError
                            ? "ADDRESS HISTORY LOOKUP FAILED"
                            : "ADDRESS HISTORY INDEX UNAVAILABLE"}
                    </strong>
                    <p>
                        {transactionIndexError
                            ? transactionCapability?.error || "The node could not load address transaction history."
                            : "The node validated this address, but transaction history requires the local address index."}
                    </p>
                </div>
            </div>
        {:else if transactions.length === 0}
            <div class="empty-state">
                <span class="empty-mark mono">[ -- ]</span>
                <strong>No indexed transactions</strong>
                <p>This address has no transaction records in the local index.</p>
            </div>
        {:else}
            <div class="transaction-table">
                <div class="table-header">
                    <span>TRANSACTION</span>
                    <span>TIME</span>
                    <span>CONF</span>
                    <span class="right">NET AMOUNT</span>
                </div>
                {#each transactions as tx}
                    {@const txid = txidOf(tx)}
                    <div class="transaction-row">
                        <div class="tx-cell">
                            <span class:outgoing={isOutgoing(tx)} class="direction">
                                {directionOf(tx)}
                            </span>
                            <button
                                type="button"
                                class="data-link mono"
                                on:click={() => navigate("transaction", txid)}
                                title={txid}
                            >
                                {txid || "--"}
                            </button>
                            {#if txid}
                                <button
                                    type="button"
                                    class="copy-inline"
                                    on:click={() => copy(txid, "TXID")}
                                    title="Copy transaction ID"
                                    aria-label="Copy transaction ID"
                                >
                                    &#x2398;
                                </button>
                            {/if}
                        </div>
                        <span class="time-cell">{formatDate(timeOf(tx))}</span>
                        <span class:pending={Number(confirmationsOf(tx)) === 0} class="confirm-cell mono">
                            {formatInteger(confirmationsOf(tx))}
                        </span>
                        <strong class:negative={isOutgoing(tx)} class="amount-cell mono">
                            {isOutgoing(tx) && Number(amountOf(tx)) > 0 ? "-" : ""}{formatAmount(amountOf(tx))}
                            <small>{firstDefined(tx, ["asset", "asset_name", "unit"], "HEMP")}</small>
                        </strong>
                    </div>
                {/each}
            </div>
        {/if}
    </section>
</article>

<style>
    .address-view {
        min-width: 0;
        color: var(--color-highlight, #e0ffee);
    }

    .result-header {
        display: flex;
        align-items: flex-end;
        justify-content: space-between;
        gap: 1.5rem;
        padding: 1.15rem 1.25rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.16);
        background: rgba(0, 0, 0, 0.34);
    }

    .identity-block {
        min-width: 0;
    }

    .eyebrow,
    .balance-lockup > span,
    .metric span {
        display: block;
        margin-bottom: 0.38rem;
        color: rgba(255, 255, 255, 0.42);
        font-size: 0.64rem;
        font-weight: 700;
        letter-spacing: 1px;
    }

    .identity-row {
        display: flex;
        min-width: 0;
        align-items: center;
        gap: 0.45rem;
    }

    .identity-link,
    .data-link {
        min-width: 0;
        padding: 0;
        overflow: hidden;
        border: 0;
        border-radius: 0;
        background: transparent;
        box-shadow: none;
        color: var(--color-primary, #00ff41);
        font: 500 0.88rem var(--font-mono, monospace);
        letter-spacing: 0;
        text-align: left;
        text-overflow: ellipsis;
        text-transform: none;
        white-space: nowrap;
    }

    .identity-link {
        max-width: min(56vw, 620px);
    }

    .identity-link:hover,
    .data-link:hover {
        color: var(--color-primary, #00ff41);
        background: transparent;
        box-shadow: none;
        transform: none;
        text-decoration: underline;
    }

    .icon-btn,
    .copy-inline {
        display: inline-grid;
        flex: 0 0 auto;
        place-items: center;
        padding: 0;
        color: var(--color-primary, #00ff41);
        letter-spacing: 0;
    }

    .icon-btn {
        width: 1.8rem;
        height: 1.8rem;
        border-color: rgba(0, 255, 65, 0.18);
        font-size: 0.9rem;
    }

    .balance-lockup {
        flex: 0 0 auto;
        text-align: right;
    }

    .balance-lockup strong {
        color: #fff;
        font: 600 1.18rem var(--font-mono, monospace);
        text-shadow: 0 0 12px rgba(0, 255, 65, 0.12);
    }

    .balance-lockup small,
    .amount-cell small {
        color: var(--color-primary-dim, #008f11);
        font-size: 0.62rem;
        letter-spacing: 0.8px;
    }

    .summary-strip {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    }

    .address-tags {
        display: flex;
        gap: 0.4rem;
        flex-wrap: wrap;
        padding: 0.52rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 255, 65, 0.018);
    }

    .address-tags span {
        padding: 0.22rem 0.42rem;
        border: 1px solid rgba(0, 255, 65, 0.14);
        border-radius: 4px;
        color: rgba(0, 255, 65, 0.58);
        font-size: 0.56rem;
        font-weight: 700;
        letter-spacing: 0.55px;
    }

    .address-tags .label-tag {
        color: rgba(255, 255, 255, 0.5);
    }

    .address-tags .invalid {
        border-color: rgba(255, 70, 70, 0.2);
        color: #ff7070;
    }

    .script-line {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) 1.4rem;
        align-items: center;
        gap: 0.65rem;
        padding: 0.52rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.18);
    }

    .script-line > span {
        color: rgba(255, 255, 255, 0.34);
        font-size: 0.56rem;
        font-weight: 700;
        letter-spacing: 0.7px;
    }

    .script-line strong {
        min-width: 0;
        overflow: hidden;
        color: rgba(255, 255, 255, 0.55);
        font-size: 0.62rem;
        font-weight: 500;
        letter-spacing: 0;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .partial-notice {
        display: flex;
        align-items: center;
        gap: 0.65rem;
        padding: 0.58rem 1rem;
        border-bottom: 1px solid rgba(0, 204, 255, 0.12);
        background: rgba(0, 204, 255, 0.035);
    }

    .partial-notice > span {
        color: var(--color-process, #00ccff);
        font-size: 0.6rem;
        font-weight: 700;
    }

    .partial-notice p {
        margin: 0;
        color: rgba(255, 255, 255, 0.46);
        font-size: 0.64rem;
        line-height: 1.4;
    }

    .partial-notice.error {
        border-color: rgba(255, 70, 70, 0.12);
        background: rgba(255, 70, 70, 0.035);
    }

    .partial-notice.error > span {
        color: #ff7070;
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

    .section-heading {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.78rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        background: rgba(0, 255, 65, 0.018);
    }

    .section-heading h3 {
        margin: 0;
        color: rgba(255, 255, 255, 0.72);
        font-size: 0.68rem;
        letter-spacing: 1px;
    }

    .section-heading span {
        color: var(--color-primary-dim, #008f11);
        font: 700 0.62rem var(--font-mono, monospace);
        letter-spacing: 0.5px;
    }

    .transaction-table {
        min-width: 0;
        overflow-x: auto;
    }

    .table-header,
    .transaction-row {
        display: grid;
        grid-template-columns: minmax(250px, 1.5fr) minmax(145px, 0.7fr) 64px minmax(130px, 0.6fr);
        align-items: center;
        gap: 0.85rem;
        min-width: 680px;
        padding: 0 1rem;
    }

    .table-header {
        height: 2rem;
        color: rgba(255, 255, 255, 0.34);
        font-size: 0.58rem;
        font-weight: 700;
        letter-spacing: 0.8px;
    }

    .transaction-row {
        min-height: 3.45rem;
        border-top: 1px solid rgba(255, 255, 255, 0.055);
    }

    .transaction-row:hover {
        background: rgba(0, 255, 65, 0.028);
    }

    .tx-cell {
        display: grid;
        grid-template-columns: 4.8rem minmax(0, 1fr) 1.4rem;
        align-items: center;
        gap: 0.45rem;
        min-width: 0;
    }

    .direction {
        color: var(--color-primary, #00ff41);
        font-size: 0.57rem;
        font-weight: 700;
        letter-spacing: 0.6px;
    }

    .direction.outgoing {
        color: #ff7070;
    }

    .data-link {
        color: rgba(255, 255, 255, 0.78);
        font-size: 0.68rem;
    }

    .copy-inline {
        width: 1.35rem;
        height: 1.35rem;
        border: 0;
        background: transparent;
        box-shadow: none;
        font-size: 0.72rem;
    }

    .copy-inline:hover {
        background: transparent;
        box-shadow: none;
        transform: none;
    }

    .time-cell {
        color: rgba(255, 255, 255, 0.48);
        font-size: 0.68rem;
    }

    .confirm-cell {
        color: var(--color-primary, #00ff41);
        font-size: 0.7rem;
    }

    .confirm-cell.pending {
        color: var(--color-process, #00ccff);
    }

    .amount-cell {
        color: var(--color-primary, #00ff41);
        font-size: 0.73rem;
        text-align: right;
    }

    .amount-cell.negative {
        color: #ff7070;
    }

    .right {
        text-align: right;
    }

    .empty-state {
        display: flex;
        min-height: 12rem;
        align-items: center;
        justify-content: center;
        flex-direction: column;
        padding: 2rem;
        color: rgba(255, 255, 255, 0.4);
        text-align: center;
    }

    .index-state {
        display: flex;
        min-height: 10rem;
        align-items: center;
        justify-content: center;
        gap: 0.85rem;
        padding: 2rem;
    }

    .state-code {
        display: grid;
        width: 2.6rem;
        height: 2.6rem;
        flex: 0 0 auto;
        place-items: center;
        border: 1px solid rgba(0, 204, 255, 0.28);
        border-radius: 5px;
        background: rgba(0, 204, 255, 0.05);
        color: var(--color-process, #00ccff);
        font-size: 0.65rem;
        font-weight: 700;
    }

    .state-code.error {
        border-color: rgba(255, 70, 70, 0.25);
        background: rgba(255, 70, 70, 0.05);
        color: #ff7070;
    }

    .index-state strong {
        color: rgba(255, 255, 255, 0.72);
        font-size: 0.7rem;
        letter-spacing: 0.8px;
    }

    .index-state p {
        max-width: 30rem;
        margin: 0.35rem 0 0;
        color: rgba(255, 255, 255, 0.4);
        font-size: 0.67rem;
        line-height: 1.5;
    }

    .empty-mark {
        margin-bottom: 0.7rem;
        color: rgba(0, 255, 65, 0.38);
    }

    .empty-state strong {
        color: rgba(255, 255, 255, 0.68);
        font-size: 0.78rem;
    }

    .empty-state p {
        margin: 0.35rem 0 0;
        font-size: 0.68rem;
    }

    @media (max-width: 640px) {
        .result-header {
            align-items: stretch;
            flex-direction: column;
        }

        .identity-link {
            max-width: calc(100vw - 8.5rem);
        }

        .balance-lockup {
            text-align: left;
        }

        .summary-strip {
            grid-template-columns: minmax(0, 1fr);
        }

        .metric {
            border-right: 0;
            border-bottom: 1px solid rgba(255, 255, 255, 0.07);
        }
    }
</style>
