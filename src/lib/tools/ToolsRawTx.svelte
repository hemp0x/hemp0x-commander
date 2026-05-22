<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { systemStatus } from "../../stores.js";

    $: tauriReady = $systemStatus.tauriReady;
    const dispatch = createEventDispatcher();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    let activeSection = "DECODE";
    let rawHex = "";
    let decodeResult = null;
    let decodeError = "";
    let decodeLoading = false;

    let mempoolResult = null;
    let mempoolError = "";
    let mempoolLoading = false;
    let mempoolUnsupported = false;

    let builderInputs = [{ txid: "", vout: 0 }];
    let builderOutputs = [{ address: "", amount: "" }];
    let buildResult = null;
    let buildError = "";
    let buildLoading = false;
    let showBuilderWarnings = false;

    function isValidHex(hex) {
        return hex.length > 0 && hex.length % 2 === 0 && /^[0-9a-fA-F]+$/.test(hex);
    }

    async function handleDecode() {
        const hex = rawHex.trim();
        if (!hex) {
            showToast("Enter raw transaction hex to decode", "error");
            return;
        }
        if (!isValidHex(hex)) {
            decodeError = "Invalid hex: must be even-length hex characters";
            return;
        }
        decodeLoading = true;
        decodeError = "";
        decodeResult = null;
        try {
            decodeResult = await core.invoke("decode_raw_transaction", { rawHex: hex });
            showToast("Transaction decoded", "success");
        } catch (err) {
            decodeError = String(err);
            showToast("Decode failed", "error");
        }
        decodeLoading = false;
    }

    async function handleMempoolCheck() {
        const hex = rawHex.trim();
        if (!hex) {
            showToast("Enter raw transaction hex first (Decode section)", "error");
            return;
        }
        if (!isValidHex(hex)) {
            mempoolError = "Invalid hex";
            return;
        }
        mempoolLoading = true;
        mempoolError = "";
        mempoolResult = null;
        mempoolUnsupported = false;
        try {
            mempoolResult = await core.invoke("test_mempool_accept", { rawHex: hex });
            showToast("Mempool check complete", "success");
        } catch (err) {
            const msg = String(err);
            if (msg.includes("Method not found") || msg.includes("unknown command")) {
                mempoolUnsupported = true;
                mempoolError = "testmempoolaccept is not supported by this node version";
            } else {
                mempoolError = msg;
            }
            showToast("Mempool check failed", "error");
        }
        mempoolLoading = false;
    }

    function addInput() {
        builderInputs = [...builderInputs, { txid: "", vout: 0 }];
    }

    function removeInput(index) {
        if (builderInputs.length <= 1) return;
        builderInputs = builderInputs.filter((_, i) => i !== index);
    }

    function addOutput() {
        builderOutputs = [...builderOutputs, { address: "", amount: "" }];
    }

    function removeOutput(index) {
        if (builderOutputs.length <= 1) return;
        builderOutputs = builderOutputs.filter((_, i) => i !== index);
    }

    function clearBuilder() {
        builderInputs = [{ txid: "", vout: 0 }];
        builderOutputs = [{ address: "", amount: "" }];
        buildResult = null;
        buildError = "";
    }

    function handleBuilderInput(index, field, value) {
        builderInputs[index][field] = value;
        builderInputs = builderInputs;
    }

    function handleBuilderOutput(index, field, value) {
        builderOutputs[index][field] = value;
        builderOutputs = builderOutputs;
    }

    async function handleBuild() {
        buildError = "";
        buildResult = null;

        const validInputs = builderInputs.filter((i) => i.txid.trim() !== "");
        const validOutputs = builderOutputs.filter((o) => o.address.trim() !== "" && o.amount.trim() !== "");

        if (validInputs.length === 0) {
            buildError = "At least one input with a valid txid is required";
            showToast("Missing inputs", "error");
            return;
        }

        if (validOutputs.length === 0) {
            buildError = "At least one output with address and amount is required";
            showToast("Missing outputs", "error");
            return;
        }

        for (const input of validInputs) {
            if (input.txid.length !== 64 || !/^[0-9a-fA-F]+$/.test(input.txid)) {
                buildError = `Invalid txid: "${input.txid}" - must be 64 hex characters`;
                showToast("Invalid txid", "error");
                return;
            }
        }

        for (const output of validOutputs) {
            const amt = parseFloat(output.amount);
            if (isNaN(amt) || amt <= 0) {
                buildError = `Invalid amount: "${output.amount}" - must be a positive number`;
                showToast("Invalid amount", "error");
                return;
            }
        }

        const inputsPayload = validInputs.map((i) => ({ txid: i.txid.trim(), vout: Number(i.vout) }));
        const seenOutputs = new Set();
        const outputsPayload = [];
        for (const o of validOutputs) {
            const address = o.address.trim();
            if (seenOutputs.has(address)) {
                buildError = `Duplicate output address: "${address}"`;
                showToast("Duplicate output address", "error");
                return;
            }
            seenOutputs.add(address);
            outputsPayload.push({ address, amount: parseFloat(o.amount).toFixed(8) });
        }

        buildLoading = true;
        try {
            const inputsJson = JSON.stringify(inputsPayload);
            const outputsJson = JSON.stringify(outputsPayload);
            buildResult = await core.invoke("create_unsigned_raw_transaction", {
                inputsJson,
                outputsJson,
            });
            showBuilderWarnings = true;
            showToast("Unsigned transaction built", "success");
        } catch (err) {
            buildError = String(err);
            showToast("Build failed", "error");
        }
        buildLoading = false;
    }

    function copyToClipboard(text) {
        if (navigator.clipboard) {
            navigator.clipboard.writeText(text).then(() => {
                showToast("Copied to clipboard", "success");
            }).catch(() => {
                showToast("Copy failed - select and copy manually", "error");
            });
        }
    }

    function clearAll() {
        rawHex = "";
        decodeResult = null;
        decodeError = "";
        mempoolResult = null;
        mempoolError = "";
        mempoolUnsupported = false;
        buildResult = null;
        buildError = "";
        builderInputs = [{ txid: "", vout: 0 }];
        builderOutputs = [{ address: "", amount: "" }];
        showBuilderWarnings = false;
    }
</script>

<div class="rawtx-view">
    <div class="rawtx-header-bar">
        <span class="header-label mono">RAW TRANSACTION EDITOR</span>
        <span class="header-sub">&#9888; ADVANCED TOOL - Decode, inspect, and build unsigned raw transactions</span>
    </div>

    <div class="section-tabs">
        {#each ["DECODE", "BUILDER"] as section}
            <button
                class="section-tab-btn"
                class:active={activeSection === section}
                on:click={() => (activeSection = section)}
            >
                {section}
            </button>
        {/each}
        <div class="section-tab-spacer"></div>
        <button class="cyber-btn ghost clear-all-btn" on:click={clearAll}>CLEAR ALL</button>
    </div>

    {#key activeSection}
        <div class="section-body" in:fly={{ y: 10, duration: 200 }}>
            {#if activeSection === "DECODE"}
                <div class="decode-section">
                    <div class="field-row">
                        <label for="raw-hex-input">RAW TRANSACTION HEX</label>
                        <div class="input-wrapper">
                            <textarea
                                id="raw-hex-input"
                                class="input-glass mono hex-input"
                                bind:value={rawHex}
                                placeholder="Paste raw transaction hex here..."
                                spellcheck="false"
                                on:input={() => {
                                    decodeResult = null;
                                    decodeError = "";
                                    mempoolResult = null;
                                    mempoolError = "";
                                    mempoolUnsupported = false;
                                }}
                            ></textarea>
                        </div>
                    </div>

                    <div class="decode-actions">
                        <button
                            class="cyber-btn"
                            on:click={handleDecode}
                            disabled={decodeLoading || !rawHex.trim()}
                        >
                            {decodeLoading ? "DECODING..." : "[ DECODE ]"}
                        </button>
                        <button
                            class="cyber-btn ghost"
                            on:click={handleMempoolCheck}
                            disabled={mempoolLoading || !rawHex.trim()}
                        >
                            {mempoolLoading ? "CHECKING..." : "[ MEMPOOL CHECK ]"}
                        </button>
                        {#if rawHex.trim()}
                            <button class="cyber-btn ghost" on:click={() => copyToClipboard(rawHex.trim())}>
                                COPY RAW
                            </button>
                        {/if}
                    </div>

                    {#if decodeError}
                        <div class="result-panel error-panel">
                            <span class="result-label">ERROR</span>
                            <pre class="result-text error">{decodeError}</pre>
                        </div>
                    {/if}

                    {#if decodeResult}
                        <div class="result-panel success-panel">
                            <div class="result-header">
                                <span class="result-label">DECODED TRANSACTION</span>
                                <button class="cyber-btn ghost small" on:click={() => copyToClipboard(JSON.stringify(decodeResult, null, 2))}>
                                    COPY JSON
                                </button>
                            </div>
                            <div class="decoded-summary">
                                {#if decodeResult.txid}
                                    <div class="summary-row">
                                        <span class="summary-key">TXID</span>
                                        <span class="summary-value mono">{decodeResult.txid}</span>
                                    </div>
                                {/if}
                                <div class="summary-row">
                                    <span class="summary-key">VERSION</span>
                                    <span class="summary-value mono">{decodeResult.version || "N/A"}</span>
                                </div>
                                <div class="summary-row">
                                    <span class="summary-key">LOCKTIME</span>
                                    <span class="summary-value mono">{decodeResult.locktime || 0}</span>
                                </div>
                                {#if decodeResult.vsize}
                                    <div class="summary-row">
                                        <span class="summary-key">VSIZE</span>
                                        <span class="summary-value mono">{decodeResult.vsize}</span>
                                    </div>
                                {/if}
                                {#if decodeResult.weight}
                                    <div class="summary-row">
                                        <span class="summary-key">WEIGHT</span>
                                        <span class="summary-value mono">{decodeResult.weight}</span>
                                    </div>
                                {/if}
                            </div>
                            <pre class="result-json">{JSON.stringify(decodeResult, null, 2)}</pre>
                        </div>
                    {/if}

                    {#if mempoolResult}
                        <div class="result-panel info-panel">
                            <span class="result-label">MEMPOOL ACCEPTANCE</span>
                            <pre class="result-json">{JSON.stringify(mempoolResult, null, 2)}</pre>
                        </div>
                    {/if}

                    {#if mempoolError}
                        <div class="result-panel error-panel">
                            <span class="result-label">MEMPOOL CHECK</span>
                            <pre class="result-text">{mempoolUnsupported ? "WARNING: " : ""}{mempoolError}</pre>
                        </div>
                    {/if}

                    {#if !decodeResult && !decodeError && !mempoolResult && !mempoolError}
                        <div class="result-panel hint-panel">
                            <span class="hint-text"
                                >Paste raw transaction hex above to decode and inspect. You can also run a mempool policy check to see if the node would accept the transaction.</span
                            >
                        </div>
                    {/if}
                </div>
            {:else}
                <div class="builder-section">
                    <div class="warning-banner">
                        <span>&#9888; STRUCTURED UNSIGNED BUILDER - This tool builds unsigned raw transactions. The output is PREVIEW-ONLY. Signing and broadcasting are not performed here. Review fees, inputs, and outputs carefully.</span>
                    </div>

                    <div class="builder-columns">
                        <div class="builder-col inputs-col">
                            <div class="builder-col-header">
                                <span class="col-title">INPUTS</span>
                                <button class="cyber-btn ghost small" on:click={addInput}>+ ADD INPUT</button>
                            </div>
                            <div class="builder-list">
                                {#each builderInputs as input, i}
                                    <div class="builder-row">
                                        <div class="builder-row-header">
                                            <span class="row-label mono">INPUT #{i + 1}</span>
                                            <button
                                                class="row-remove-btn"
                                                on:click={() => removeInput(i)}
                                                disabled={builderInputs.length <= 1}
                                            >&times;</button>
                                        </div>
                                        <div class="builder-field">
                                            <label for="txid-{i}">TXID</label>
                                            <input
                                                id="txid-{i}"
                                                class="input-glass mono full-width"
                                                placeholder="64-character txid..."
                                                value={input.txid}
                                                on:input={(e) => handleBuilderInput(i, 'txid', e.target.value)}
                                            />
                                        </div>
                                        <div class="builder-field">
                                            <label for="vout-{i}">VOUT</label>
                                            <input
                                                id="vout-{i}"
                                                class="input-glass mono"
                                                type="number"
                                                min="0"
                                                step="1"
                                                value={input.vout}
                                                on:input={(e) => handleBuilderInput(i, 'vout', parseInt(e.target.value) || 0)}
                                            />
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        </div>

                        <div class="builder-col outputs-col">
                            <div class="builder-col-header">
                                <span class="col-title">OUTPUTS</span>
                                <button class="cyber-btn ghost small" on:click={addOutput}>+ ADD OUTPUT</button>
                            </div>
                            <div class="builder-list">
                                {#each builderOutputs as output, i}
                                    <div class="builder-row">
                                        <div class="builder-row-header">
                                            <span class="row-label mono">OUTPUT #{i + 1}</span>
                                            <button
                                                class="row-remove-btn"
                                                on:click={() => removeOutput(i)}
                                                disabled={builderOutputs.length <= 1}
                                            >&times;</button>
                                        </div>
                                        <div class="builder-field">
                                            <label for="out-addr-{i}">ADDRESS</label>
                                            <input
                                                id="out-addr-{i}"
                                                class="input-glass mono full-width"
                                                placeholder="Destination address..."
                                                value={output.address}
                                                on:input={(e) => handleBuilderOutput(i, 'address', e.target.value)}
                                            />
                                        </div>
                                        <div class="builder-field">
                                            <label for="out-amt-{i}">AMOUNT (HEMP)</label>
                                            <input
                                                id="out-amt-{i}"
                                                class="input-glass mono"
                                                placeholder="0.00"
                                                value={output.amount}
                                                on:input={(e) => handleBuilderOutput(i, 'amount', e.target.value)}
                                            />
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    </div>

                    <div class="builder-action-row">
                        <button
                            class="cyber-btn primary"
                            on:click={handleBuild}
                            disabled={buildLoading}
                        >
                            {buildLoading ? "BUILDING..." : "[ BUILD UNSIGNED RAW TX ]"}
                        </button>
                        <button class="cyber-btn ghost" on:click={clearBuilder}>CLEAR BUILDER</button>
                    </div>

                    {#if showBuilderWarnings}
                        <div class="builder-warnings">
                            <span class="warning-text">
                                &#9888; <strong>IMPORTANT:</strong> This raw transaction is <strong>unsigned</strong> and <strong>unfunded</strong>. Fees cannot be determined until all UTXO amounts are known. To complete this transaction: (1) Fund inputs with real UTXOs, (2) Sign the transaction, (3) Broadcast. Never share unsigned or signed hex with untrusted parties.
                            </span>
                        </div>
                    {/if}

                    {#if buildError}
                        <div class="result-panel error-panel">
                            <span class="result-label">BUILD ERROR</span>
                            <pre class="result-text error">{buildError}</pre>
                        </div>
                    {/if}

                    {#if buildResult}
                        <div class="result-panel success-panel">
                            <div class="result-header">
                                <span class="result-label">UNSIGNED RAW TRANSACTION</span>
                                <button class="cyber-btn ghost small" on:click={() => copyToClipboard(buildResult.raw_hex)}>
                                    COPY HEX
                                </button>
                            </div>
                            <div class="build-summary">
                                <div class="summary-row">
                                    <span class="summary-key">INPUTS</span>
                                    <span class="summary-value mono">{buildResult.input_count}</span>
                                </div>
                                <div class="summary-row">
                                    <span class="summary-key">OUTPUTS</span>
                                    <span class="summary-value mono">{buildResult.output_count}</span>
                                </div>
                                <div class="summary-row warning">
                                    <span class="summary-key">WARNING</span>
                                    <span class="summary-value">{buildResult.fee_warning}</span>
                                </div>
                            </div>
                            <label for="built-hex-display">RAW HEX</label>
                            <textarea
                                id="built-hex-display"
                                class="input-glass mono built-hex-display"
                                readonly
                                value={buildResult.raw_hex}
                                rows="4"
                            ></textarea>
                            <label for="built-decode-display" style="margin-top: 0.5rem;">DECODED</label>
                            <pre class="result-json">{JSON.stringify(buildResult.decoded, null, 2)}</pre>
                        </div>
                    {/if}
                </div>
            {/if}
        </div>
    {/key}
</div>

<style>
    .rawtx-view {
        display: flex;
        flex-direction: column;
        height: 100%;
        gap: 0.5rem;
        overflow: hidden;
    }

    .rawtx-header-bar {
        display: flex;
        align-items: baseline;
        gap: 1rem;
        flex-shrink: 0;
    }

    .header-label {
        font-size: 0.85rem;
        font-weight: 700;
        color: #ffaa00;
        letter-spacing: 1px;
    }

    .header-sub {
        font-size: 0.7rem;
        color: #666;
        font-family: var(--font-mono);
    }

    .section-tabs {
        display: flex;
        gap: 4px;
        align-items: center;
        flex-shrink: 0;
    }

    .section-tab-btn {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.08);
        color: #666;
        padding: 0.4rem 1.2rem;
        font-size: 0.7rem;
        letter-spacing: 1px;
        cursor: pointer;
        font-family: var(--font-mono);
        border-radius: 4px;
        transition: all 0.2s;
    }

    .section-tab-btn:hover {
        color: #fff;
        border-color: rgba(255, 255, 255, 0.2);
        background: rgba(255, 255, 255, 0.02);
    }

    .section-tab-btn.active {
        color: #ffaa00;
        border-color: #ffaa00;
        background: rgba(255, 170, 0, 0.08);
        text-shadow: 0 0 6px rgba(255, 170, 0, 0.3);
    }

    .section-tab-spacer {
        flex: 1;
    }

    .clear-all-btn {
        padding: 0.3rem 0.8rem;
        font-size: 0.65rem;
    }

    .section-body {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }

    .decode-section, .builder-section {
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
    }

    .field-row {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }

    .field-row label, .builder-field label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1px;
        text-transform: uppercase;
    }

    .input-wrapper {
        display: flex;
        flex-direction: column;
    }

    .input-glass {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #fff;
        padding: 0.4rem 0.6rem;
        font-size: 0.75rem;
        border-radius: 4px;
        outline: none;
        font-family: var(--font-mono);
    }

    .input-glass:focus {
        border-color: #ffaa00;
    }

    .full-width {
        width: 100%;
        box-sizing: border-box;
    }

    .hex-input {
        min-height: 80px;
        resize: vertical;
        font-family: monospace;
        word-break: break-all;
    }

    .decode-actions {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
    }

    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.4rem 0.8rem;
        font-size: 0.7rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        white-space: nowrap;
        font-family: var(--font-mono);
        border-radius: 4px;
    }

    .cyber-btn:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
    }

    .cyber-btn:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .cyber-btn.ghost {
        border-color: rgba(255, 255, 255, 0.2);
        color: #aaa;
        background: transparent;
    }

    .cyber-btn.ghost:hover:not(:disabled) {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }

    .cyber-btn.primary {
        background: rgba(255, 170, 0, 0.1);
        border-color: #ffaa00;
        color: #ffaa00;
        padding: 0.5rem 1.2rem;
        font-size: 0.75rem;
    }

    .cyber-btn.primary:hover:not(:disabled) {
        background: #ffaa00;
        color: #000;
        box-shadow: 0 0 15px rgba(255, 170, 0, 0.4);
    }

    .cyber-btn.small {
        padding: 0.2rem 0.6rem;
        font-size: 0.6rem;
    }

    .result-panel {
        background: rgba(0, 0, 0, 0.3);
        border-radius: 6px;
        padding: 0.6rem 0.8rem;
        overflow: hidden;
    }

    .result-panel.success-panel {
        border: 1px solid rgba(0, 255, 65, 0.15);
    }

    .result-panel.error-panel {
        border: 1px solid rgba(255, 85, 85, 0.2);
    }

    .result-panel.info-panel {
        border: 1px solid rgba(0, 180, 255, 0.15);
    }

    .result-panel.hint-panel {
        border: 1px dashed rgba(255, 255, 255, 0.05);
    }

    .result-label {
        display: block;
        font-size: 0.65rem;
        color: #888;
        letter-spacing: 1px;
        text-transform: uppercase;
        margin-bottom: 0.4rem;
    }

    .success-panel .result-label {
        color: var(--color-primary);
    }

    .error-panel .result-label {
        color: #ff5555;
    }

    .info-panel .result-label {
        color: #00b4ff;
    }

    .result-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.4rem;
    }

    .result-header .result-label {
        margin-bottom: 0;
    }

    .result-json {
        margin: 0;
        padding: 0.5rem;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        font-size: 0.7rem;
        color: #aaffaa;
        overflow-x: auto;
        max-height: 300px;
        overflow-y: auto;
        font-family: monospace;
        white-space: pre-wrap;
        word-break: break-all;
    }

    .result-text {
        margin: 0;
        padding: 0.5rem;
        background: rgba(0, 0, 0, 0.5);
        border-radius: 4px;
        font-size: 0.75rem;
        color: #ffaa88;
        font-family: monospace;
        white-space: pre-wrap;
        word-break: break-all;
    }

    .result-text.error {
        color: #ff8888;
    }

    .decoded-summary, .build-summary {
        display: flex;
        flex-wrap: wrap;
        gap: 0.3rem 1.5rem;
        padding: 0.4rem 0.5rem;
        background: rgba(0, 255, 65, 0.04);
        border: 1px solid rgba(0, 255, 65, 0.1);
        border-radius: 4px;
        margin-bottom: 0.5rem;
    }

    .summary-row {
        display: flex;
        gap: 0.5rem;
        align-items: baseline;
    }

    .summary-row.warning {
        flex-basis: 100%;
    }

    .summary-key {
        font-size: 0.6rem;
        color: #666;
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }

    .summary-value {
        font-size: 0.7rem;
        color: #ddd;
        word-break: break-all;
    }

    .summary-row.warning .summary-value {
        color: #ffaa00;
        font-size: 0.65rem;
    }

    .hint-text {
        color: #555;
        font-size: 0.75rem;
        line-height: 1.4;
    }

    .built-hex-display {
        width: 100%;
        min-height: 60px;
        resize: vertical;
        font-size: 0.65rem;
        margin-top: 0.3rem;
    }

    .warning-banner {
        background: rgba(255, 170, 0, 0.12);
        border: 1px solid rgba(255, 170, 0, 0.3);
        border-radius: 4px;
        padding: 0.5rem 0.7rem;
        font-size: 0.7rem;
        color: #ffaa00;
        line-height: 1.4;
        font-family: var(--font-mono);
        letter-spacing: 0.3px;
        flex-shrink: 0;
    }

    .builder-columns {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.8rem;
    }

    .builder-col-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.4rem;
    }

    .col-title {
        font-size: 0.7rem;
        color: var(--color-primary);
        letter-spacing: 1px;
        font-weight: 600;
    }

    .builder-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        max-height: 400px;
        overflow-y: auto;
    }

    .builder-row {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 6px;
        padding: 0.5rem;
    }

    .builder-row-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.4rem;
    }

    .row-label {
        font-size: 0.65rem;
        color: #888;
    }

    .row-remove-btn {
        background: none;
        border: 1px solid rgba(255, 85, 85, 0.3);
        color: #ff5555;
        font-size: 1rem;
        line-height: 1;
        padding: 0 5px;
        border-radius: 3px;
        cursor: pointer;
        font-family: inherit;
    }

    .row-remove-btn:hover:not(:disabled) {
        background: rgba(255, 85, 85, 0.15);
    }

    .row-remove-btn:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    .builder-field {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
        margin-bottom: 0.35rem;
    }

    .builder-field:last-child {
        margin-bottom: 0;
    }

    .builder-field .input-glass {
        width: 100%;
        box-sizing: border-box;
    }

    .builder-action-row {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .builder-warnings {
        background: rgba(255, 68, 68, 0.08);
        border: 1px solid rgba(255, 68, 68, 0.2);
        border-radius: 4px;
        padding: 0.5rem 0.7rem;
    }

    .warning-text {
        color: #ff9999;
        font-size: 0.7rem;
        line-height: 1.3;
        display: block;
    }

    .warning-text strong {
        color: #ff4444;
    }

    @media (max-width: 800px) {
        .builder-columns {
            grid-template-columns: 1fr;
        }
    }
</style>
