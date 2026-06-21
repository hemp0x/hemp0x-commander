<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import CommanderLoader from "../ui/CommanderLoader.svelte";
    import CopyIcon from "../ui/CopyIcon.svelte";
    import {
        firstDefined,
        formatAmount,
        formatDate,
        formatInteger,
    } from "./utils.js";

    export let addressData = {};
    export let address = "";

    const dispatch = createEventDispatcher();
    const PAGE_SIZE = 25;
    let currentPage = 0;
    let lastHistorySource = "";
    let pageItems = [];
    let pageTotal = 0;
    let pageLoading = false;
    let pageError = "";
    let pagePruned = false;
    let pageRequestId = 0;

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
    $: historySource = `${resolvedAddress}:${transactionCapability?.state || ""}:${transactionCapability?.totalCount || 0}`;
    $: if (historySource !== lastHistorySource) {
        lastHistorySource = historySource;
        currentPage = 0;
        pageItems = [];
        pageTotal = Number(transactionCapability?.totalCount) || 0;
        pageError = "";
        pagePruned = false;
        if (
            resolvedAddress &&
            transactionCapability?.state === "supported" &&
            Number(transactionCapability?.totalCount) > 0
        ) {
            loadTransactionPage(0);
        }
    }
    $: transactionIndexUnsupported =
        transactionCapability?.state === "unsupported" ||
        addressData?.addressTxidsSupported === false;
    $: balanceIndexUnsupported =
        balanceCapability?.state === "unsupported" ||
        addressData?.addressBalanceSupported === false;
    $: balanceIndexError = balanceCapability?.state === "error";
    $: transactionIndexError = transactionCapability?.state === "error";
    $: transactionCount =
        pageTotal || Number(transactionCapability?.totalCount) || 0;
    $: pageCount = Math.max(1, Math.ceil(transactionCount / PAGE_SIZE));
    $: pageStart = currentPage * PAGE_SIZE;
    $: pageEnd = Math.min(pageStart + pageItems.length, transactionCount);
    $: labels = Array.isArray(addressData?.labels) ? addressData.labels : [];

    function normalizeAtomicAmount(value, atomicUnits) {
        if (value === null || value === undefined || value === "") return null;
        const number = Number(value);
        if (!Number.isFinite(number)) return value;
        return atomicUnits ? number / 100_000_000 : number;
    }

    function navigate(type, target) {
        if (target) dispatch("navigate", { type, target: String(target) });
    }

    function txidOf(tx) {
        if (typeof tx === "string") return tx;
        return String(firstDefined(tx, ["txid", "id", "hash"], ""));
    }

    function amountOf(tx) {
        return firstDefined(
            tx,
            ["netAmount", "amount", "value", "balance_delta", "net"],
            null,
        );
    }

    function confirmationsOf(tx) {
        return firstDefined(tx, ["confirmations", "confirm_count"], 0);
    }

    function timeOf(tx) {
        return firstDefined(tx, ["time", "timestamp", "blocktime", "block_time"]);
    }

    async function loadTransactionPage(page) {
        if (!resolvedAddress) return;
        const requestedAddress = resolvedAddress;
        const requestId = ++pageRequestId;
        currentPage = page;
        pageLoading = true;
        pageError = "";
        try {
            const result = await core.invoke("get_address_transactions_page", {
                address: requestedAddress,
                offset: page * PAGE_SIZE,
                limit: PAGE_SIZE,
            });
            if (
                requestId !== pageRequestId ||
                requestedAddress !== resolvedAddress
            ) return;
            currentPage = page;
            pageItems = Array.isArray(result?.items) ? result.items : [];
            pageTotal = Number(result?.total) || 0;
            pagePruned = Boolean(result?.pruned);
        } catch (err) {
            if (requestId !== pageRequestId) return;
            pageError = String(err || "Address history page failed to load.");
            pageItems = [];
        } finally {
            if (requestId === pageRequestId) pageLoading = false;
        }
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
                    <CopyIcon />
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
                <CopyIcon size={11} />
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
            <span>
                {transactionCount === 0
                    ? "0 LOADED"
                    : `${pageStart + 1}-${pageEnd} OF ${formatInteger(transactionCount)}`}
            </span>
        </header>

        {#if pagePruned}
            <div class="history-limit-notice">
                Pruned node mode: indexed amounts and confirmations remain
                available. Raw transaction details may be unavailable for older
                entries.
            </div>
        {/if}

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
        {:else if pageLoading}
            <div class="history-loader">
                <CommanderLoader
                    label="Loading address history"
                    detail={`Transactions ${pageStart + 1}-${pageStart + PAGE_SIZE}`}
                />
            </div>
        {:else if pageError}
            <div class="index-state">
                <span class="state-code error mono">ERR</span>
                <div>
                    <strong>ADDRESS HISTORY PAGE FAILED</strong>
                    <p>{pageError}</p>
                    <button
                        type="button"
                        class="retry-page"
                        on:click={() => loadTransactionPage(currentPage)}
                    >
                        RETRY
                    </button>
                </div>
            </div>
        {:else if transactionCount === 0}
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
                {#each pageItems as tx}
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
                                    <CopyIcon size={11} />
                                </button>
                            {/if}
                        </div>
                        <span class="time-cell">
                            {timeOf(tx) == null
                                ? tx.detailStatus === "pruned"
                                    ? "PRUNED"
                                    : "UNAVAILABLE"
                                : formatDate(timeOf(tx))}
                        </span>
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
            {#if pageCount > 1}
                <div class="pagination">
                    <button
                        type="button"
                        on:click={() => loadTransactionPage(Math.max(0, currentPage - 1))}
                        disabled={pageLoading || currentPage === 0}
                    >
                        PREVIOUS
                    </button>
                    <span class="mono">PAGE {currentPage + 1} / {pageCount}</span>
                    <button
                        type="button"
                        on:click={() =>
                            loadTransactionPage(Math.min(pageCount - 1, currentPage + 1))}
                        disabled={pageLoading || currentPage >= pageCount - 1}
                    >
                        NEXT
                    </button>
                </div>
            {/if}
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
    .history-limit-notice {
        padding: 0.5rem 1rem;
        border-bottom: 1px solid rgba(0, 204, 255, 0.1);
        background: rgba(0, 204, 255, 0.025);
        color: rgba(255, 255, 255, 0.45);
        font-size: 0.62rem;
    }
    .history-loader {
        display: grid;
        min-height: 12rem;
        place-items: center;
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
    .pagination {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: 0.65rem;
        padding: 0.65rem 1rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.24);
    }
    .pagination button {
        min-width: 5.5rem;
        padding: 0.38rem 0.65rem;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 4px;
        background: rgba(0, 255, 65, 0.035);
        color: var(--color-primary, #00ff41);
        font-size: 0.58rem;
        letter-spacing: 0.7px;
    }
    .pagination button:hover:not(:disabled) {
        border-color: rgba(0, 255, 65, 0.48);
        background: rgba(0, 255, 65, 0.08);
        box-shadow: none;
        transform: none;
    }
    .pagination button:disabled {
        opacity: 0.35;
        cursor: not-allowed;
    }
    .pagination span {
        color: rgba(255, 255, 255, 0.4);
        font-size: 0.58rem;
    }
    .retry-page {
        margin-top: 0.65rem;
        padding: 0.35rem 0.7rem;
        border: 1px solid rgba(255, 90, 90, 0.25);
        border-radius: 4px;
        background: rgba(255, 90, 90, 0.04);
        color: #ff8888;
        font-size: 0.58rem;
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
