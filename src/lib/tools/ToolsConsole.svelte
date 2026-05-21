<script>
    import { tick, createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { nodeStatus, networkInfo } from "../../stores.js";

    export let consoleOutput = "";
    export let consoleHistory = [];
    const dispatch = createEventDispatcher();

    $: isNodeOnline = $nodeStatus.online;
    $: networkMode = $networkInfo.chain;
    export let isProcessing = false;
    export let processingMessage = "";

    const DANGER_COMMANDS = new Set([
        "dumpprivkey", "dumpwallet", "importprivkey", "importwallet",
        "walletpassphrase", "encryptwallet", "sendtoaddress", "sendmany",
        "transfer", "issue", "reissue", "stop",
    ]);

    const commandGroups = {
        "Wallet & Payments": [
            "getbalance", "getunconfirmedbalance", "getwalletinfo",
            "listtransactions", "listunspent", "listlockunspent", "lockunspent",
            "listaccounts", "listwallets", "listaddressgroupings",
            "listreceivedbyaccount", "listreceivedbyaddress", "getnewaddress",
            "getaccountaddress", "getaccount", "setaccount",
            "getreceivedbyaccount", "getreceivedbyaddress", "getaddressesbyaccount",
            "sendtoaddress", "sendfrom", "sendmany", "settxfee", "validateaddress",
            "signmessage", "verifymessage", "backupwallet", "encryptwallet",
            "walletpassphrase", "walletlock", "dumpwallet", "importwallet",
        ],
        Assets: [
            "listassets", "listmyassets", "getassetdata", "issue", "issueunique",
            "issuerestrictedasset", "issuequalifierasset", "reissue",
            "reissuerestrictedasset", "transfer", "transferfromaddress",
            "transferfromaddresses", "transferqualifier", "listaddressrestrictions",
            "listglobalrestrictions", "checkaddressrestriction",
            "checkglobalrestriction", "freezeaddress", "unfreezeaddress",
            "freezerestrictedasset", "unfreezerestrictedasset",
            "viewmyrestrictedaddresses", "viewmytaggedaddresses", "addtagtoaddress",
            "removetagfromaddress", "listtagsforaddress", "distributereward",
            "getdistributestatus",
        ],
        "Mining & Chain": [
            "getblockcount", "getbestblockhash", "getdifficulty",
            "getnetworkhashps", "getmininginfo", "getblock", "getblockhash",
            "getblockheader", "getchaintips", "getblockchaininfo", "getmempoolinfo",
            "getrawmempool", "gettxoutsetinfo", "generate", "generatetoaddress",
            "getgenerate", "setgenerate", "submitblock",
        ],
        Network: [
            "getinfo", "getpeerinfo", "getconnectioncount", "getnettotals",
            "getnetworkinfo", "ping", "addnode", "disconnectnode", "setban",
            "listbanned", "clearbanned", "setnetworkactive",
        ],
        "Advanced / Raw": [
            "createrawtransaction", "decoderawtransaction", "signrawtransaction",
            "sendrawtransaction", "getrawtransaction", "fundrawtransaction",
            "combinerawtransaction", "dumpprivkey", "importprivkey", "importaddress",
            "importpubkey", "importmulti", "importprunedfunds", "removeprunedfunds",
            "addmultisigaddress", "createmultisig", "signmessagewithprivkey",
        ],
        "System & Debug": [
            "help", "stop", "uptime", "getmemoryinfo", "getrpcinfo",
            "getcacheinfo", "checkaddresstag", "isvalidverifierstring",
            "getverifierstring",
        ],
    };

    const commands = Object.values(commandGroups).flat().sort();

    let sessionIdCounter = 0;
    let sessions = [];
    let activeIndex = 0;

    function createSession(type = "CLI") {
        sessionIdCounter++;
        const t = type === "CLI" ? "CLI" : "Shell";
        const name = `${t} ${sessionIdCounter}`;
        return {
            id: sessionIdCounter,
            name,
            output: "",
            input: "",
            shellMode: type !== "CLI",
            historyIndex: -1,
            shellWarningShown: false,
            kind: type,
        };
    }

    sessions = [createSession("CLI")];

    $: activeSession = sessions[activeIndex];
    $: consoleOutput = activeSession ? activeSession.output : "";
    $: cmdPlaceholder = shellMode ? "type a shell command" : "command args";

    let consoleRef;
    let inputRef;
    let selectedCommand = "";
    let cmdLine = "";
    let shellMode = false;
    let prettyJson = false;
    let outputSearch = "";
    let showHistoryPanel = false;
    let commandSearch = "";
    let editingTabId = null;
    let editTabName = "";

    $: filteredCommands = commandSearch
        ? commands.filter(c => c.includes(commandSearch.toLowerCase()))
        : [];

    $: filteredOutput = (() => {
        const output = activeSession ? activeSession.output : "";
        if (!output || !outputSearch) return { text: output, matchCount: 0, totalLines: 0 };
        const lines = output.split("\n");
        const lower = outputSearch.toLowerCase();
        const matched = lines.filter(l => l.toLowerCase().includes(lower));
        if (matched.length === 0) return { text: "", matchCount: 0, totalLines: lines.length };
        return { text: matched.join("\n"), matchCount: matched.length, totalLines: lines.length };
    })();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    async function appendOutput(text) {
        if (!text) return;
        if (!activeSession) return;
        activeSession.output = activeSession.output
            ? `${activeSession.output}\n${text}`
            : text;
        // Ensure Svelte notices the session output change for textarea re-render
        sessions = [...sessions];
        await tick();
        if (consoleRef) {
            consoleRef.scrollTop = consoleRef.scrollHeight;
        }
    }

    function syncActiveToConsoleOutput() {
        const session = sessions[activeIndex];
        if (session) {
            consoleOutput = session.output;
            cmdLine = session.input;
            shellMode = session.shellMode;
        }
    }

    function switchTab(index) {
        const current = sessions[activeIndex];
        if (current) {
            current.input = cmdLine;
        }
        activeIndex = index;
        syncActiveToConsoleOutput();
        editingTabId = null;
        showHistoryPanel = false;
        tick().then(() => {
            if (consoleRef) consoleRef.scrollTop = consoleRef.scrollHeight;
            if (inputRef) inputRef.focus();
        });
    }

    function addSession() {
        const current = sessions[activeIndex];
        if (current) {
            current.input = cmdLine;
        }
        const session = createSession(shellMode ? "Shell" : "CLI");
        sessions = [...sessions, session];
        activeIndex = sessions.length - 1;
        syncActiveToConsoleOutput();
        tick().then(() => { if (inputRef) inputRef.focus(); });
    }

    function closeSession(index) {
        if (sessions.length <= 1) return;
        let nextActive = activeIndex;
        if (index < activeIndex) {
            nextActive = activeIndex - 1;
        } else if (index === activeIndex) {
            nextActive = Math.min(activeIndex, sessions.length - 2);
        }
        sessions = sessions.filter((_, i) => i !== index);
        activeIndex = nextActive;
        syncActiveToConsoleOutput();
        tick().then(() => { if (inputRef) inputRef.focus(); });
    }

    function startRename(index) {
        editingTabId = sessions[index].id;
        editTabName = sessions[index].name;
    }

    function finishRename(index) {
        const name = editTabName.trim();
        if (name) {
            sessions[index].name = name;
        }
        editingTabId = null;
    }

    function cancelRename() {
        editingTabId = null;
    }

    function handleRenameKeydown(event, index) {
        if (event.key === "Enter") {
            finishRename(index);
        } else if (event.key === "Escape") {
            cancelRename();
        }
    }

    async function runCommand() {
        if (!activeSession) return;

        try {
            const line = cmdLine.trim();
            if (!line) {
                appendOutput("Enter a command to run");
                showToast("No command entered", "error");
                return;
            }

            if (consoleHistory[consoleHistory.length - 1] !== line) {
                consoleHistory = [...consoleHistory, line];
            }
            activeSession.historyIndex = consoleHistory.length;

            isProcessing = true;
            processingMessage = "Running Command...";

            let res;
            const prompt = shellMode ? "$" : ">";
            appendOutput(`${prompt} ${line}`);

            if (shellMode) {
                res = await core.invoke("run_shell_command", { command: line });
            } else {
                const splitAt = line.search(/\s/);
                const cmd = splitAt === -1 ? line : line.slice(0, splitAt);
                const cmdArgs = splitAt === -1 ? "" : line.slice(splitAt + 1);
                res = await core.invoke("run_cli_command", {
                    command: cmd,
                    args: cmdArgs,
                });
            }

            let displayText = res || "(no output)";

            if (prettyJson && displayText && displayText !== "(no output)") {
                try {
                    const parsed = JSON.parse(displayText);
                    displayText = JSON.stringify(parsed, null, 2);
                } catch (_) {
                    // not JSON, leave as-is
                }
            }

            isProcessing = false;
            processingMessage = "";
            appendOutput(displayText);
            showToast("Command Executed", "success");
            cmdLine = "";
            activeSession.input = "";
            selectedCommand = "";
        } catch (err) {
            isProcessing = false;
            processingMessage = "";
            appendOutput(`Error: ${err}`);
            showToast("Command Failed", "error");
        }
    }

    function handleCommandSelect(event) {
        selectedCommand = event.target.value;
        const cmd = selectedCommand || "";
        cmdLine = cmd;
        if (cmd) {
            if (DANGER_COMMANDS.has(cmd.toLowerCase())) {
                appendOutput(
                    `[!] ${cmd} is a sensitive command. Verify all arguments before executing.`
                );
            }
            setTimeout(() => { selectedCommand = ""; }, 100);
        }
    }

    function handleCommandSearchSelect(cmd) {
        cmdLine = cmd;
        commandSearch = "";
        if (DANGER_COMMANDS.has(cmd.toLowerCase())) {
            appendOutput(
                `[!] ${cmd} is a sensitive command. Verify all arguments before executing.`
            );
        }
        tick().then(() => { if (inputRef) inputRef.focus(); });
    }

    function toggleShellMode() {
        if (!activeSession) return;
        if (!activeSession.shellMode && !activeSession.shellWarningShown) {
            activeSession.shellWarningShown = true;
            appendOutput(
                "[!] SHELL MODE: Commands run directly on this computer. " +
                "Pasted commands can be dangerous. " +
                "Secrets typed here may remain in output and history. " +
                "Use only when you understand the command and trust the input."
            );
        }
        activeSession.shellMode = !activeSession.shellMode;
        shellMode = activeSession.shellMode;
    }

    function replaceLastToken(value) {
        const idx = cmdLine.search(/\S+$/);
        if (idx === -1) {
            cmdLine = value;
        } else {
            cmdLine = cmdLine.slice(0, idx) + value;
        }
    }

    async function handleAutocomplete() {
        if (!activeSession) return;

        if (shellMode) {
            try {
                const matches = await core.invoke("shell_autocomplete", { line: cmdLine });
                if (!matches || matches.length === 0) return;
                if (matches.length === 1) {
                    replaceLastToken(matches[0]);
                } else {
                    appendOutput(`Matches:\n${matches.join("\n")}`);
                }
            } catch (err) {
                appendOutput(`Error: ${err}`);
            }
            return;
        }

        const input = cmdLine.trim();
        if (!input) return;
        const token = input.split(/\s+/)[0];
        const options = commands.filter(c => c.startsWith(token));
        if (options.length === 1) {
            replaceLastToken(options[0]);
        } else if (options.length > 1) {
            appendOutput(`Matches:\n${options.join("\n")}`);
        }
    }

    function handlePromptKeydown(event) {
        if (event.key === "Enter") {
            event.preventDefault();
            runCommand();
            return;
        }
        if (event.key === "ArrowUp") {
            if (consoleHistory.length === 0) return;
            event.preventDefault();
            if (!activeSession) return;
            activeSession.historyIndex = Math.max(0, activeSession.historyIndex - 1);
            cmdLine = consoleHistory[activeSession.historyIndex] || "";
            activeSession.input = cmdLine;
            return;
        }
        if (event.key === "ArrowDown") {
            if (consoleHistory.length === 0) return;
            event.preventDefault();
            if (!activeSession) return;
            activeSession.historyIndex = Math.min(
                consoleHistory.length,
                activeSession.historyIndex + 1
            );
            cmdLine = activeSession.historyIndex >= consoleHistory.length
                ? ""
                : consoleHistory[activeSession.historyIndex];
            activeSession.input = cmdLine;
            return;
        }
        if (event.key === "Tab") {
            event.preventDefault();
            handleAutocomplete();
        }
    }

    function loadHistoryItem(line) {
        cmdLine = line;
        if (activeSession) activeSession.input = line;
        showHistoryPanel = false;
        tick().then(() => { if (inputRef) inputRef.focus(); });
    }

    function clearOutputSearch() {
        outputSearch = "";
    }
</script>

<div class="console-view full-height">
    <div class="tab-row">
        <div class="tabs">
            {#each sessions as session, i}
                <div
                    class="tab-btn"
                    class:active={i === activeIndex}
                    class:shell={session.shellMode}
                    role="button"
                    tabindex="0"
                    on:click={() => switchTab(i)}
                    on:keydown={(e) => e.key === 'Enter' && switchTab(i)}
                    on:dblclick={() => startRename(i)}
                    title="Double-click to rename"
                >
                    {#if editingTabId === session.id}
                        <input
                            class="tab-rename-input"
                            bind:value={editTabName}
                            on:keydown={(e) => handleRenameKeydown(e, i)}
                            on:blur={() => finishRename(i)}
                            size="10"
                        />
                    {:else}
                        <span class="tab-label">{session.name}</span>
                    {/if}
                    {#if sessions.length > 1}
                        <button
                            class="tab-close"
                            type="button"
                            on:click|stopPropagation={() => closeSession(i)}
                            on:keydown={(e) => e.key === 'Enter' && closeSession(i)}
                            title="Close session"
                        >&times;</button>
                    {/if}
                </div>
            {/each}
        </div>
        <button class="tab-add-btn" on:click={addSession} title="New session">+</button>
    </div>

    <div class="terminal-screen">
        <div class="scanline"></div>
        <div class="output-toolbar">
            <input
                class="search-input mono"
                placeholder="Search output..."
                bind:value={outputSearch}
                on:keydown={(e) => e.key === "Escape" && clearOutputSearch()}
            />
            {#if outputSearch}
                <span class="search-match-count">
                    {filteredOutput.matchCount}/{filteredOutput.totalLines}
                </span>
                <button class="search-clear" on:click={clearOutputSearch}>&times;</button>
            {/if}
        </div>
        <textarea
            class="console-output mono"
            readonly
            value={outputSearch ? filteredOutput.text : (activeSession ? activeSession.output : consoleOutput)}
            bind:this={consoleRef}
        ></textarea>
    </div>

    <div class="control-bar compact">
        <div class="field-group grow">
            <label for="console-mode">MODE</label>
            <div class="input-wrapper brackets">
                <div class="mode-badge" class:shell={shellMode}>
                    {shellMode ? "SHELL" : "CLI RPC"}
                </div>
            </div>
        </div>
        <div class="field-group grow">
            <label for="cmd-select">COMMAND LIST</label>
            <div class="input-wrapper brackets">
                <div class="cmd-search-row">
                    <input
                        class="input-glass mono cmd-search"
                        placeholder="Search commands..."
                        bind:value={commandSearch}
                        disabled={shellMode}
                    />
                </div>
                {#if commandSearch && filteredCommands.length > 0}
                    <div class="cmd-search-dropdown">
                        {#each filteredCommands.slice(0, 12) as cmd}
                            <button
                                class="cmd-search-item"
                                class:danger={DANGER_COMMANDS.has(cmd.toLowerCase())}
                                on:click={() => handleCommandSearchSelect(cmd)}
                            >
                                {cmd}
                                {#if DANGER_COMMANDS.has(cmd.toLowerCase())}
                                    <span class="danger-badge">!</span>
                                {/if}
                            </button>
                        {/each}
                    </div>
                {:else}
                    <select
                        id="cmd-select"
                        bind:value={selectedCommand}
                        on:change={handleCommandSelect}
                        class="input-glass"
                        disabled={shellMode}
                    >
                        <option value="">Select command...</option>
                        {#each Object.entries(commandGroups) as [groupName, groupCmds]}
                            <optgroup label={groupName}>
                                {#each groupCmds as cmd}
                                    <option value={cmd}>{cmd}{DANGER_COMMANDS.has(cmd.toLowerCase()) ? ' [!]' : ''}</option>
                                {/each}
                            </optgroup>
                        {/each}
                    </select>
                {/if}
            </div>
        </div>
        <div class="field-group grow">
            <label for="cmd-line">ARGUMENT</label>
            <div class="input-wrapper brackets">
                <input
                    id="cmd-line"
                    class="input-glass mono"
                    placeholder={cmdPlaceholder}
                    value={cmdLine}
                    on:input={(e) => {
                        cmdLine = e.target.value;
                        if (activeSession) activeSession.input = e.target.value;
                    }}
                    on:keydown={handlePromptKeydown}
                    bind:this={inputRef}
                />
            </div>
        </div>
        <div class="field-group options-col">
            <label class="option-toggle">
                <input type="checkbox" bind:checked={prettyJson} />
                <span>JSON</span>
            </label>
        </div>
        <div class="action-group">
            <button class="cyber-btn" on:click={runCommand}>[ RUN ]</button>
            <button
                class="cyber-btn ghost"
                on:click={() => {
                    if (activeSession) activeSession.output = "";
                    syncActiveToConsoleOutput();
                }}>CLEAR</button>
            <button class="cyber-btn ghost" on:click={toggleShellMode}>
                {shellMode ? "SHELL" : "CLI"}
            </button>
            <button
                class="cyber-btn ghost"
                on:click={() => showHistoryPanel = !showHistoryPanel}
                title="Command history"
            >
                HIST
            </button>
        </div>
    </div>

    {#if showHistoryPanel}
        <div class="history-panel" transition:fly={{ y: -4, duration: 150 }}>
            <div class="history-header">
                <span>COMMAND HISTORY</span>
                <button class="history-clear-btn" on:click={() => consoleHistory = []}>
                    CLEAR ALL
                </button>
            </div>
            <div class="history-body">
                {#if consoleHistory.length === 0}
                    <div class="history-empty">No history</div>
                {:else}
                    {#each [...consoleHistory].reverse() as line}
                        <button
                            class="history-item"
                            on:click={() => loadHistoryItem(line)}
                            title={line}
                        >
                            <span class="history-text">{line}</span>
                        </button>
                    {/each}
                {/if}
            </div>
        </div>
    {/if}
</div>

<style>
    .console-view {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        height: 100%;
        min-height: 0;
    }
    .full-height {
        height: 100%;
    }

    .tab-row {
        display: flex;
        align-items: center;
        gap: 4px;
        flex-shrink: 0;
    }
    .tabs {
        display: flex;
        gap: 2px;
        flex: 1;
        overflow-x: auto;
    }
    .tab-btn {
        display: flex;
        align-items: center;
        gap: 4px;
        background: rgba(0, 20, 10, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.15);
        color: #888;
        padding: 4px 12px;
        font-size: 0.7rem;
        letter-spacing: 0.5px;
        border-radius: 4px 4px 0 0;
        cursor: pointer;
        white-space: nowrap;
        flex-shrink: 0;
    }
    .tab-btn:hover {
        color: #fff;
        border-color: rgba(0, 255, 65, 0.3);
        background: rgba(0, 30, 10, 0.7);
    }
    .tab-btn.active {
        color: var(--color-primary);
        border-color: var(--color-primary);
        background: rgba(0, 0, 0, 0.7);
        text-shadow: 0 0 6px rgba(0, 255, 65, 0.3);
    }
    .tab-btn.shell.active {
        border-color: #ffa500;
        color: #ffa500;
        text-shadow: 0 0 6px rgba(255, 165, 0, 0.3);
    }
    .tab-label {
        max-width: 120px;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .tab-close {
        background: none;
        border: none;
        font-size: 0.8rem;
        line-height: 1;
        padding: 0 3px;
        color: #555;
        border-radius: 2px;
        cursor: pointer;
        font-family: inherit;
    }
    .tab-close:hover {
        color: #ff5555;
        background: rgba(255, 85, 85, 0.15);
    }
    .tab-rename-input {
        background: #000;
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 1px 4px;
        font-size: 0.7rem;
        outline: none;
        border-radius: 2px;
        font-family: inherit;
    }
    .tab-add-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: var(--color-primary);
        padding: 4px 10px;
        font-size: 0.9rem;
        border-radius: 4px 4px 0 0;
        cursor: pointer;
        line-height: 1;
    }
    .tab-add-btn:hover {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
    }

    .terminal-screen {
        flex: 1;
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        position: relative;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }
    .scanline {
        position: absolute;
        top: 0; left: 0; right: 0;
        height: 2px;
        background: rgba(0, 255, 65, 0.1);
        opacity: 0.5;
        animation: scan 6s linear infinite;
        pointer-events: none;
        z-index: 2;
    }
    @keyframes scan {
        0% { transform: translateY(-100%); }
        100% { transform: translateY(1000%); }
    }

    .output-toolbar {
        display: flex;
        align-items: center;
        padding: 4px 8px;
        background: rgba(0, 0, 0, 0.4);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        z-index: 3;
    }
    .search-input {
        flex: 1;
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.2);
        color: #00ff41;
        padding: 2px 8px;
        font-size: 0.7rem;
        border-radius: 3px;
        outline: none;
        max-width: 200px;
    }
    .search-input:focus {
        border-color: var(--color-primary);
    }
    .search-clear {
        background: none;
        border: none;
        color: #666;
        font-size: 0.9rem;
        cursor: pointer;
        padding: 0 4px;
    }
    .search-clear:hover {
        color: #fff;
    }
    .search-match-count {
        color: #555;
        font-size: 0.6rem;
        white-space: nowrap;
    }

    .console-output {
        flex: 1;
        background: transparent;
        border: none;
        color: #00ff41;
        padding: 0.6rem 1rem;
        font-size: 0.8rem;
        resize: none;
        outline: none;
        white-space: pre-wrap;
        position: relative;
        z-index: 1;
    }

    .control-bar {
        background: rgba(0, 20, 10, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        padding: 0.7rem;
        display: flex;
        gap: 0.6rem;
        align-items: flex-end;
        flex-shrink: 0;
    }
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
    }
    .field-group.grow {
        flex: 1;
        min-width: 0;
    }
    .field-group.options-col {
        justify-content: flex-end;
        padding-bottom: 2px;
    }
    .input-wrapper {
        position: relative;
    }
    .action-group {
        display: flex;
        gap: 0.4rem;
    }
    label {
        font-size: 0.6rem;
        color: #555;
        letter-spacing: 1px;
    }
    .option-toggle {
        display: flex;
        align-items: center;
        gap: 4px;
        font-size: 0.6rem;
        color: #666;
        cursor: pointer;
        white-space: nowrap;
    }
    .option-toggle input {
        accent-color: var(--color-primary);
    }

    .mode-badge {
        padding: 6px 12px;
        border-radius: 4px;
        font-size: 0.7rem;
        letter-spacing: 1px;
        text-align: center;
        background: rgba(0, 255, 65, 0.07);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .mode-badge.shell {
        background: rgba(255, 165, 0, 0.07);
        border-color: rgba(255, 165, 0, 0.3);
        color: #ffa500;
    }

    .cmd-search-row {
        position: relative;
    }
    .cmd-search {
        width: 100%;
    }
    .cmd-search-dropdown {
        position: absolute;
        top: 100%;
        left: 0;
        right: 0;
        background: #111;
        border: 1px solid rgba(0, 255, 65, 0.3);
        border-radius: 0 0 4px 4px;
        max-height: 240px;
        overflow-y: auto;
        z-index: 100;
    }
    .cmd-search-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        width: 100%;
        background: transparent;
        border: none;
        color: #00ff41;
        padding: 5px 10px;
        font-size: 0.75rem;
        cursor: pointer;
        text-align: left;
        font-family: inherit;
    }
    .cmd-search-item:hover {
        background: rgba(0, 255, 65, 0.1);
    }
    .cmd-search-item.danger {
        color: #ffa500;
    }
    .cmd-search-item.danger:hover {
        background: rgba(255, 165, 0, 0.1);
    }
    .danger-badge {
        display: inline-block;
        background: rgba(255, 80, 80, 0.15);
        color: #ff5555;
        border: 1px solid #ff5555;
        font-size: 0.6rem;
        padding: 1px 4px;
        border-radius: 2px;
        margin-left: 4px;
    }

    .input-glass {
        width: 100%;
        background: #111;
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: #00ff41;
        padding: 0.5rem;
        border-radius: 4px;
        font-family: inherit;
        font-size: 0.8rem;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
        outline: none;
    }
    select.input-glass {
        appearance: none;
        -webkit-appearance: none;
        -moz-appearance: none;
        background-color: #111 !important;
        background-image: none;
        color: #00ff41 !important;
        cursor: pointer;
    }
    select.input-glass option {
        background-color: #111;
        color: #00ff41;
        padding: 10px;
    }

    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.5rem 1rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        font-size: 0.7rem;
        white-space: nowrap;
    }
    .cyber-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.3);
    }
    .cyber-btn.ghost {
        border-color: rgba(255, 255, 255, 0.15);
        color: #777;
        background: transparent;
    }
    .cyber-btn.ghost:hover {
        border-color: #fff;
        color: #fff;
        box-shadow: none;
        background: rgba(255, 255, 255, 0.05);
    }

    .history-panel {
        background: rgba(0, 0, 0, 0.85);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 6px;
        max-height: 200px;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        flex-shrink: 0;
    }
    .history-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 6px 10px;
        background: rgba(0, 255, 65, 0.05);
        border-bottom: 1px solid rgba(0, 255, 65, 0.1);
        font-size: 0.6rem;
        color: #555;
        letter-spacing: 1px;
    }
    .history-clear-btn {
        background: none;
        border: 1px solid rgba(255, 85, 85, 0.3);
        color: #ff5555;
        font-size: 0.55rem;
        padding: 2px 6px;
        border-radius: 3px;
        cursor: pointer;
        letter-spacing: 0.5px;
    }
    .history-clear-btn:hover {
        background: rgba(255, 85, 85, 0.1);
    }
    .history-body {
        overflow-y: auto;
        flex: 1;
    }
    .history-empty {
        color: #444;
        padding: 1rem;
        text-align: center;
        font-size: 0.75rem;
    }
    .history-item {
        display: block;
        width: 100%;
        background: transparent;
        border: none;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        color: #0c0;
        padding: 4px 12px;
        font-size: 0.7rem;
        cursor: pointer;
        text-align: left;
        font-family: monospace;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .history-item:hover {
        background: rgba(0, 255, 65, 0.08);
    }
    .history-text {
        pointer-events: none;
    }

    @media (max-width: 800px) {
        .control-bar {
            flex-wrap: wrap;
        }
    }
</style>
