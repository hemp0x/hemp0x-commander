<script>
    import { onMount, createEventDispatcher, tick } from "svelte";
    import { core } from "@tauri-apps/api";
    import CommanderLoader from "../ui/CommanderLoader.svelte";
    import AddressView from "./AddressView.svelte";
    import TransactionView from "./TransactionView.svelte";
    import {
        classifyExplorerTarget,
        hasUnsupportedIndexFlag,
        isUnsupportedIndexError,
    } from "./utils.js";

    export let initialQuery = "";
    export let initialTarget = "";
    export let autoSearch = true;

    const dispatch = createEventDispatcher();

    let query = "";
    let activeTarget = "";
    let result = null;
    let resultType = null;
    let loading = false;
    let error = "";
    let unsupportedIndex = false;
    let mounted = false;
    let copyNotice = "";
    let copyNoticeTimer;
    let lastPropTarget = "";
    let requestId = 0;

    $: incomingTarget = String(initialTarget || initialQuery || "").trim();
    $: if (mounted && incomingTarget && incomingTarget !== lastPropTarget) {
        lastPropTarget = incomingTarget;
        query = incomingTarget;
        if (autoSearch) search(incomingTarget);
    }

    onMount(() => {
        mounted = true;
        if (incomingTarget) {
            lastPropTarget = incomingTarget;
            query = incomingTarget;
            if (autoSearch) search(incomingTarget);
        }

        return () => clearTimeout(copyNoticeTimer);
    });

    function clearState() {
        result = null;
        resultType = null;
        activeTarget = "";
        error = "";
        unsupportedIndex = false;
    }

    async function search(target = query) {
        const normalized = String(target || "").trim();
        query = normalized;
        error = "";
        unsupportedIndex = false;

        const type = classifyExplorerTarget(normalized);
        if (!type) {
            result = null;
            resultType = null;
            activeTarget = "";
            error =
                "Enter a 64-character transaction ID or a valid Hemp0x R-address.";
            return;
        }

        const currentRequest = ++requestId;
        loading = true;
        result = null;
        resultType = null;
        activeTarget = normalized;
        await tick();

        try {
            const command =
                type === "transaction"
                    ? "get_transaction_detail"
                    : "get_address_detail";
            const args =
                type === "transaction"
                    ? { txid: normalized }
                    : { address: normalized };
            const response = await core.invoke(command, args);

            if (currentRequest !== requestId) return;
            if (hasUnsupportedIndexFlag(response)) {
                unsupportedIndex = true;
                result = null;
                return;
            }

            result = response?.data ?? response;
            resultType = type;
            dispatch("loaded", { type, target: normalized, result });
        } catch (err) {
            if (currentRequest !== requestId) return;
            if (isUnsupportedIndexError(err)) {
                unsupportedIndex = true;
            } else {
                error = friendlyError(err, type);
            }
        } finally {
            if (currentRequest === requestId) loading = false;
        }
    }

    function friendlyError(err, type) {
        const message = String(err?.message || err || "").trim();
        if (!message) return `Unable to load ${type} data from the local node.`;
        if (/not found|no such|unknown transaction/i.test(message)) {
            return `${type === "transaction" ? "Transaction" : "Address"} data was not found in the local node index.`;
        }
        if (/connect|offline|rpc|daemon/i.test(message)) {
            return "The local node is unavailable. Start or reconnect the daemon and try again.";
        }
        return message;
    }

    function handleSubmit(event) {
        event.preventDefault();
        search();
    }

    function handleNavigate(event) {
        const { type, target } = event.detail;
        query = target;
        dispatch("navigate", { type, target });
        search(target);
    }

    function handleCopied(event) {
        clearTimeout(copyNoticeTimer);
        copyNotice = `${event.detail.label} copied`;
        copyNoticeTimer = setTimeout(() => {
            copyNotice = "";
        }, 1600);
    }

    function handleCopyError() {
        clearTimeout(copyNoticeTimer);
        copyNotice = "Copy failed";
        copyNoticeTimer = setTimeout(() => {
            copyNotice = "";
        }, 1600);
    }

    function openAssetBrowser() {
        window.dispatchEvent(new CustomEvent("commander-open-asset-browser"));
    }
</script>

<div class="explorer-shell">
    <header class="explorer-toolbar">
        <div class="title-lockup">
            <span class="terminal-mark mono">&gt;_</span>
            <div>
                <h2>LOCAL EXPLORER</h2>
                <span class="subtitle mono">NODE INDEX / PRIVATE LOOKUP</span>
            </div>
        </div>

        <form class="search-form" on:submit={handleSubmit}>
            <label for="explorer-query">TXID OR R-ADDRESS</label>
            <div class="search-control">
                <span class="search-glyph" aria-hidden="true"></span>
                <input
                    id="explorer-query"
                    type="text"
                    bind:value={query}
                    placeholder="Search transaction or address..."
                    autocomplete="off"
                    spellcheck="false"
                    aria-label="Transaction ID or Hemp0x address"
                />
                {#if query}
                    <button
                        type="button"
                        class="clear-btn"
                        on:click={() => {
                            query = "";
                            clearState();
                        }}
                        title="Clear search"
                        aria-label="Clear search"
                    >
                        &times;
                    </button>
                {/if}
                <button type="submit" class="search-btn" disabled={loading || !query.trim()}>
                    {loading ? "WAIT" : "GO"}
                </button>
            </div>
        </form>
        <button
            type="button"
            class="asset-browser-btn"
            on:click={openAssetBrowser}
            title="Browse network assets"
        >
            ASSETS
        </button>
    </header>

    <main class="explorer-content" aria-live="polite">
        {#if loading}
            <div class="state-panel loading-state">
                <CommanderLoader
                    label="Querying local index"
                    detail={activeTarget}
                />
            </div>
        {:else if unsupportedIndex}
            <div class="state-panel unsupported-state">
                <span class="state-code mono">IDX</span>
                <div>
                    <strong>LOCAL INDEX UNAVAILABLE</strong>
                    <p>
                        This lookup requires the node transaction or address index.
                        Enable the required index and reindex the chain before retrying.
                    </p>
                    <button type="button" class="retry-btn" on:click={() => search(activeTarget)}>
                        RETRY
                    </button>
                </div>
            </div>
        {:else if error}
            <div class="state-panel error-state">
                <span class="state-code mono">ERR</span>
                <div>
                    <strong>LOOKUP FAILED</strong>
                    <p>{error}</p>
                    {#if activeTarget}
                        <button type="button" class="retry-btn" on:click={() => search(activeTarget)}>
                            RETRY
                        </button>
                    {/if}
                </div>
            </div>
        {:else if result && resultType === "transaction"}
            <TransactionView
                transaction={result}
                on:navigate={handleNavigate}
                on:copied={handleCopied}
                on:copyerror={handleCopyError}
            />
        {:else if result && resultType === "address"}
            <AddressView
                addressData={result}
                address={activeTarget}
                on:navigate={handleNavigate}
                on:copied={handleCopied}
                on:copyerror={handleCopyError}
            />
        {:else}
            <div class="welcome-state">
                <div class="vault-rings" aria-hidden="true">
                    <span></span>
                    <span></span>
                    <i>H</i>
                </div>
                <strong>LOCAL CHAIN INTELLIGENCE</strong>
                <p>
                    Inspect a transaction or Hemp0x address directly through your
                    connected node.
                </p>
                <div class="accepted-types mono">
                    <span>64 HEX / TXID</span>
                    <span>R... / ADDRESS</span>
                </div>
            </div>
        {/if}
    </main>

    <footer class="explorer-footer">
        <span class="connection-label">
            <i></i>LOCAL RPC
        </span>
        <span class="privacy-note mono">NO THIRD-PARTY EXPLORER REQUESTS</span>
        {#if copyNotice}
            <span class:error={copyNotice === "Copy failed"} class="copy-notice">
                {copyNotice}
            </span>
        {/if}
    </footer>
</div>

<style>
    .explorer-shell {
        display: flex;
        width: 100%;
        height: 100%;
        min-height: 360px;
        min-width: 0;
        overflow: hidden;
        flex-direction: column;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 8px;
        background:
            linear-gradient(rgba(0, 255, 65, 0.012) 1px, transparent 1px),
            rgba(3, 6, 4, 0.96);
        background-size: 100% 4px;
        box-shadow: 0 16px 36px rgba(0, 0, 0, 0.62);
        color: var(--color-highlight, #e0ffee);
    }

    .explorer-toolbar {
        display: grid;
        grid-template-columns: auto minmax(260px, 540px) auto;
        align-items: end;
        gap: 1.5rem;
        flex: 0 0 auto;
        padding: 0.85rem 1rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.16);
        background: rgba(0, 0, 0, 0.58);
    }

    .title-lockup {
        display: flex;
        align-items: center;
        gap: 0.7rem;
        min-width: 0;
    }

    .terminal-mark {
        color: var(--color-primary, #00ff41);
        font-size: 1.05rem;
        text-shadow: 0 0 10px rgba(0, 255, 65, 0.4);
    }

    .title-lockup h2 {
        margin: 0;
        color: rgba(255, 255, 255, 0.88);
        font-size: 0.77rem;
        letter-spacing: 1.2px;
    }

    .subtitle {
        display: block;
        margin-top: 0.2rem;
        color: rgba(0, 255, 65, 0.42);
        font-size: 0.55rem;
        letter-spacing: 0.7px;
    }

    .search-form {
        min-width: 0;
    }
    .asset-browser-btn {
        height: 2.35rem;
        padding: 0 0.8rem;
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 5px;
        background: rgba(0, 255, 65, 0.04);
        color: var(--color-primary, #00ff41);
        font-size: 0.62rem;
        letter-spacing: 0.8px;
    }
    .asset-browser-btn:hover {
        border-color: rgba(0, 255, 65, 0.55);
        background: rgba(0, 255, 65, 0.1);
        box-shadow: 0 0 12px rgba(0, 255, 65, 0.1);
        transform: none;
    }

    .search-form label {
        display: block;
        margin-bottom: 0.28rem;
        color: rgba(255, 255, 255, 0.38);
        font-size: 0.55rem;
        font-weight: 700;
        letter-spacing: 0.8px;
    }

    .search-control {
        display: grid;
        grid-template-columns: 2rem minmax(0, 1fr) auto auto;
        align-items: center;
        height: 2.35rem;
        overflow: hidden;
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 5px;
        background: rgba(0, 0, 0, 0.58);
        transition: border-color 0.18s, box-shadow 0.18s;
    }

    .search-control:focus-within {
        border-color: rgba(0, 255, 65, 0.62);
        box-shadow: 0 0 15px rgba(0, 255, 65, 0.1);
    }

    .search-glyph {
        position: relative;
        width: 0.72rem;
        height: 0.72rem;
        justify-self: center;
        border: 1px solid rgba(0, 255, 65, 0.58);
        border-radius: 50%;
    }

    .search-glyph::after {
        content: "";
        position: absolute;
        right: -0.28rem;
        bottom: -0.2rem;
        width: 0.4rem;
        height: 1px;
        background: rgba(0, 255, 65, 0.58);
        transform: rotate(45deg);
    }

    .search-control input {
        width: 100%;
        min-width: 0;
        height: 100%;
        padding: 0;
        border: 0;
        outline: 0;
        background: transparent;
        color: #fff;
        font: 0.72rem var(--font-mono, monospace);
        letter-spacing: 0;
    }

    .search-control input::placeholder {
        color: rgba(255, 255, 255, 0.25);
    }

    .clear-btn {
        width: 1.7rem;
        height: 100%;
        padding: 0;
        border: 0;
        background: transparent;
        box-shadow: none;
        color: rgba(255, 255, 255, 0.4);
        font-size: 1rem;
        letter-spacing: 0;
    }

    .clear-btn:hover {
        background: transparent;
        box-shadow: none;
        color: #fff;
        transform: none;
    }

    .search-btn {
        align-self: stretch;
        min-width: 3.4rem;
        padding: 0 0.75rem;
        border-width: 0 0 0 1px;
        border-color: rgba(0, 255, 65, 0.18);
        border-radius: 0;
        background: rgba(0, 255, 65, 0.07);
        color: var(--color-primary, #00ff41);
        font: 700 0.63rem var(--font-mono, monospace);
        letter-spacing: 0.8px;
    }

    .search-btn:hover {
        transform: none;
    }

    .search-btn:disabled {
        cursor: default;
        opacity: 0.35;
    }

    .explorer-content {
        min-height: 0;
        overflow: auto;
        flex: 1 1 auto;
    }

    .state-panel,
    .welcome-state {
        display: flex;
        min-height: 100%;
        align-items: center;
        justify-content: center;
        padding: 2rem;
    }

    .state-panel {
        gap: 1rem;
        text-align: left;
    }

    .state-panel > div {
        max-width: 34rem;
    }

    .state-panel strong,
    .welcome-state > strong {
        color: rgba(255, 255, 255, 0.82);
        font-size: 0.75rem;
        letter-spacing: 1px;
    }

    .state-panel p,
    .welcome-state p {
        margin: 0.45rem 0 0;
        color: rgba(255, 255, 255, 0.42);
        font-size: 0.7rem;
        line-height: 1.55;
    }

    .state-code {
        display: grid;
        width: 2.6rem;
        height: 2.6rem;
        flex: 0 0 auto;
        place-items: center;
        border: 1px solid rgba(255, 91, 91, 0.3);
        border-radius: 5px;
        background: rgba(255, 50, 50, 0.05);
        color: #ff6b6b;
        font-size: 0.68rem;
        font-weight: 700;
    }

    .unsupported-state .state-code {
        border-color: rgba(0, 204, 255, 0.28);
        background: rgba(0, 204, 255, 0.05);
        color: var(--color-process, #00ccff);
    }

    .retry-btn {
        margin-top: 0.85rem;
        padding: 0.42rem 0.7rem;
        font-size: 0.6rem;
    }

    .welcome-state {
        min-height: 20rem;
        flex-direction: column;
        text-align: center;
    }

    .vault-rings {
        position: relative;
        display: grid;
        width: 5.2rem;
        height: 5.2rem;
        margin-bottom: 1.2rem;
        place-items: center;
        border: 1px solid rgba(0, 255, 65, 0.17);
        border-radius: 50%;
        box-shadow:
            inset 0 0 22px rgba(0, 255, 65, 0.04),
            0 0 25px rgba(0, 255, 65, 0.05);
    }

    .vault-rings::before,
    .vault-rings span {
        content: "";
        position: absolute;
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 50%;
    }

    .vault-rings::before {
        inset: 0.48rem;
    }

    .vault-rings span:first-child {
        inset: 1.05rem;
        border-style: dashed;
    }

    .vault-rings span:nth-child(2) {
        inset: 1.55rem;
        background: rgba(0, 255, 65, 0.04);
    }

    .vault-rings i {
        position: relative;
        color: var(--color-primary, #00ff41);
        font: normal 700 1.05rem var(--font-mono, monospace);
        text-shadow: 0 0 8px rgba(0, 255, 65, 0.45);
    }

    .welcome-state p {
        max-width: 28rem;
    }

    .accepted-types {
        display: flex;
        gap: 0.7rem;
        margin-top: 1rem;
        color: rgba(0, 255, 65, 0.42);
        font-size: 0.58rem;
    }

    .accepted-types span {
        padding: 0.28rem 0.45rem;
        border: 1px solid rgba(0, 255, 65, 0.12);
        border-radius: 4px;
        background: rgba(0, 255, 65, 0.025);
    }

    .explorer-footer {
        position: relative;
        display: flex;
        min-height: 2rem;
        align-items: center;
        gap: 1rem;
        flex: 0 0 auto;
        padding: 0.45rem 1rem;
        border-top: 1px solid rgba(0, 255, 65, 0.12);
        background: rgba(0, 0, 0, 0.46);
        color: rgba(255, 255, 255, 0.3);
        font-size: 0.55rem;
        font-weight: 700;
        letter-spacing: 0.7px;
    }

    .connection-label {
        display: flex;
        align-items: center;
        gap: 0.38rem;
        color: rgba(0, 255, 65, 0.55);
    }

    .connection-label i {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: var(--color-primary, #00ff41);
        box-shadow: 0 0 6px rgba(0, 255, 65, 0.8);
    }

    .privacy-note {
        margin-left: auto;
    }

    .copy-notice {
        position: absolute;
        right: 1rem;
        bottom: calc(100% + 0.5rem);
        padding: 0.42rem 0.58rem;
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 4px;
        background: rgba(2, 8, 4, 0.96);
        color: var(--color-primary, #00ff41);
        box-shadow: 0 8px 20px rgba(0, 0, 0, 0.55);
        font-size: 0.62rem;
    }

    .copy-notice.error {
        border-color: rgba(255, 70, 70, 0.25);
        color: #ff7070;
    }

    @media (max-width: 720px) {
        .explorer-toolbar {
            grid-template-columns: minmax(0, 1fr) auto;
            align-items: stretch;
            gap: 0.7rem;
        }
        .search-form {
            grid-column: 1 / -1;
            grid-row: 2;
        }

        .privacy-note {
            display: none;
        }
    }

    @media (max-width: 430px) {
        .state-panel {
            align-items: flex-start;
            flex-direction: column;
        }

        .accepted-types {
            align-items: center;
            flex-direction: column;
        }
    }
</style>
