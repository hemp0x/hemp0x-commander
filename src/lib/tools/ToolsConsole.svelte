<script>
    import { tick, createEventDispatcher, onMount } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { ensureNodeSyncedForBroadcast } from "../utils/nodeSync.js";
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
        "transfer", "issue", "reissue", "stop", "rescanblockchain",
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
            "rescanblockchain", "abandontransaction", "gettransaction",
            "listsinceblock",
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
            "getchaintxstats", "getblockstats", "gettxout", "getmempoolentry",
            "getdeploymentinfo",
        ],
        Network: [
            "getinfo", "getpeerinfo", "getconnectioncount", "getnettotals",
            "getnetworkinfo", "ping", "addnode", "disconnectnode", "setban",
            "listbanned", "clearbanned", "setnetworkactive",
            "getnodeaddresses",
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
            "getverifierstring", "getindexinfo",
        ],
    };

    const CONSOLE_DISCLAIMER_KEY = "commander_console_disclaimer_hidden_v1";
    const allCommands = Object.values(commandGroups).flat().sort();

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
            selectedCommand: "",
            shellMode: type !== "CLI",
            historyIndex: -1,
            shellWarningShown: false,
            kind: type,
        };
    }

    sessions = [createSession("CLI")];

    $: activeSession = sessions[activeIndex];
    $: consoleOutput = activeSession ? activeSession.output : "";
    $: cmdPlaceholder = shellMode ? "type a shell command" : "command arguments";

    let consoleRef;
    let inputRef;
    let selectedCommand = "";
    let cmdLine = "";
    let shellMode = false;
    let prettyJson = false;
    let outputSearch = "";
    let editingTabId = null;
    let editTabName = "";
    let advancedShellEnabled = false;
    let showShellEnableConfirm = false;
    let showConsoleDisclaimer = false;
    let hideConsoleDisclaimer = false;
    let consoleDisclaimerEnabled = true;

    let showCommandDropdown = false;
    let commandDropdownSearch = "";
    let commandDropdownRef;

    onMount(async () => {
        try {
            consoleDisclaimerEnabled = localStorage.getItem(CONSOLE_DISCLAIMER_KEY) !== "true";
            showConsoleDisclaimer = consoleDisclaimerEnabled;
        } catch (_) {
            consoleDisclaimerEnabled = true;
            showConsoleDisclaimer = true;
        }

        try {
            advancedShellEnabled = await core.invoke("get_advanced_shell_enabled");
        } catch (_) {
            advancedShellEnabled = false;
        }
    });

    $: filteredDropdownCommands = (() => {
        const term = commandDropdownSearch.trim().toLowerCase();
        if (!term) return commandGroups;
        const result = {};
        for (const [group, cmds] of Object.entries(commandGroups)) {
            const matched = cmds.filter(c => c.toLowerCase().includes(term));
            if (matched.length) result[group] = matched;
        }
        return result;
    })();

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
            selectedCommand = session.selectedCommand;
            shellMode = session.shellMode;
        }
    }

    function switchTab(index) {
        const current = sessions[activeIndex];
        if (current) {
            current.input = cmdLine;
            current.selectedCommand = selectedCommand;
        }
        activeIndex = index;
        syncActiveToConsoleOutput();
        editingTabId = null;
        showCommandDropdown = false;
        tick().then(() => {
            if (consoleRef) consoleRef.scrollTop = consoleRef.scrollHeight;
            if (inputRef) inputRef.focus();
        });
    }

    function addSession() {
        const current = sessions[activeIndex];
        if (current) {
            current.input = cmdLine;
            current.selectedCommand = selectedCommand;
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
            let cmd, cmdArgs, fullLine;
            if (shellMode) {
                fullLine = cmdLine.trim();
                cmd = fullLine;
                cmdArgs = "";
            } else {
                const trimmedArgs = cmdLine.trim();
                if (selectedCommand) {
                    cmd = selectedCommand;
                    cmdArgs = trimmedArgs;
                    fullLine = cmd + (cmdArgs ? " " + cmdArgs : "");
                } else {
                    const line = trimmedArgs;
                    if (!line) {
                        appendOutput("Enter a command to run");
                        showToast("No command entered", "error");
                        return;
                    }
                    const splitAt = line.search(/\s/);
                    cmd = splitAt === -1 ? line : line.slice(0, splitAt);
                    cmdArgs = splitAt === -1 ? "" : line.slice(splitAt + 1);
                    fullLine = line;
                }
            }

            if (!fullLine && !cmd) {
                appendOutput("Enter a command to run");
                showToast("No command entered", "error");
                return;
            }

            if (consoleHistory[consoleHistory.length - 1] !== fullLine) {
                consoleHistory = [...consoleHistory, fullLine];
            }
            activeSession.historyIndex = consoleHistory.length;

            isProcessing = true;
            processingMessage = "Running Command...";

            let res;
            const prompt = shellMode ? "$" : ">";
            appendOutput(`${prompt} ${fullLine}`);

            if (shellMode) {
                res = await core.invoke("run_shell_command", { command: cmd });
            } else {
                if (cmd.toLowerCase() === "sendrawtransaction") {
                    try { await ensureNodeSyncedForBroadcast(); } catch (e) { appendOutput(`Sync check: ${e}`); showToast("Node not synced", "error"); isProcessing = false; processingMessage = ""; return; }
                }
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
            commandDropdownSearch = "";
        } catch (err) {
            isProcessing = false;
            processingMessage = "";
            appendOutput(`Error: ${err}`);
            showToast("Command Failed", "error");
        }
    }

    function handleCommandDropdownSelect(cmd) {
        selectedCommand = cmd;
        cmdLine = "";
        commandDropdownSearch = "";
        showCommandDropdown = false;
        if (DANGER_COMMANDS.has(cmd.toLowerCase())) {
            appendOutput(
                `[!] ${cmd} is a sensitive command. Verify all arguments before executing.`
            );
        }
        tick().then(() => { if (inputRef) inputRef.focus(); });
    }

    async function showCommandHelp(cmd) {
        if (!cmd) return;
        try {
            isProcessing = true;
            processingMessage = "Loading help...";
            const res = await core.invoke("run_cli_command", {
                command: "help",
                args: cmd,
            });
            isProcessing = false;
            processingMessage = "";
            appendOutput(`--- Help for ${cmd} ---\n${res || "(no help available)"}\n---`);
            showToast(`Help loaded for ${cmd}`, "success");
        } catch (err) {
            isProcessing = false;
            processingMessage = "";
            appendOutput(`Error fetching help: ${err}`);
            showToast("Failed to load help", "error");
        }
    }

    function toggleCommandDropdown() {
        if (shellMode) return;
        showCommandDropdown = !showCommandDropdown;
        if (showCommandDropdown) {
            commandDropdownSearch = "";
            tick().then(() => {
                if (commandDropdownRef) commandDropdownRef.focus();
            });
        }
    }

    function closeCommandDropdown() {
        showCommandDropdown = false;
        commandDropdownSearch = "";
    }

    function handleDropdownKeydown(e) {
        if (e.key === "Escape") {
            closeCommandDropdown();
        }
    }

    function toggleShellMode() {
        if (!activeSession) return;
        if (!activeSession.shellMode) {
            if (!advancedShellEnabled) {
                showShellEnableConfirm = true;
                return;
            }
            if (!activeSession.shellWarningShown) {
                activeSession.shellWarningShown = true;
                appendOutput(
                    "[!] SHELL MODE: Unrestricted system commands. " +
                    "Pasted commands can be dangerous. " +
                    "Secrets may remain in output and history."
                );
            }
        }
        activeSession.shellMode = !activeSession.shellMode;
        shellMode = activeSession.shellMode;
    }

    async function confirmEnableShell() {
        try {
            advancedShellEnabled = await core.invoke("set_advanced_shell_enabled", { enabled: true });
            showShellEnableConfirm = false;
            showToast("Shell mode enabled globally", "success");
            appendOutput(
                "[!] SHELL MODE ENABLED: Unrestricted system commands. " +
                "Pasted commands can be dangerous. " +
                "Secrets may remain in output and history."
            );
            activeSession.shellMode = true;
            activeSession.shellWarningShown = true;
            shellMode = true;
        } catch (err) {
            showToast("Failed to enable shell mode: " + err, "error");
        }
    }

    function cancelShellEnable() {
        showShellEnableConfirm = false;
    }

    function closeConsoleDisclaimer() {
        if (hideConsoleDisclaimer) {
            try {
                localStorage.setItem(CONSOLE_DISCLAIMER_KEY, "true");
            } catch (_) {
                // Ignore localStorage failures; the disclaimer can be shown again next visit.
            }
        }
        consoleDisclaimerEnabled = !hideConsoleDisclaimer;
        showConsoleDisclaimer = false;
    }

    function toggleConsoleDisclaimerPreference() {
        try {
            if (consoleDisclaimerEnabled) {
                localStorage.removeItem(CONSOLE_DISCLAIMER_KEY);
            } else {
                localStorage.setItem(CONSOLE_DISCLAIMER_KEY, "true");
            }
        } catch (_) {
            // Ignore localStorage failures; the current checkbox state still reflects the session.
        }
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
        const options = allCommands.filter(c => c.startsWith(token));
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

    function clearOutputSearch() {
        outputSearch = "";
    }
</script>

<div class="console-view full-height">
    <div class="console-header">
        <div class="console-title">
            <span class="console-title-text">CONSOLE</span>
            <span class="console-status" class:online={isNodeOnline}>
                {isNodeOnline ? '● NODE ONLINE' : '● NODE OFFLINE'}
            </span>
        </div>
        <div class="console-meta">
            <label class="console-notice-toggle" title="Show the console notice when opening this page">
                <span class="console-notice-icon">i</span>
                <input
                    type="checkbox"
                    bind:checked={consoleDisclaimerEnabled}
                    on:change={toggleConsoleDisclaimerPreference}
                />
            </label>
            <div class="mode-pill" class:shell={shellMode}>
                <span class="mode-pill-icon">{shellMode ? '⚡' : '>'}</span>
                <span class="mode-pill-label">{shellMode ? 'SHELL MODE' : 'CLI/RPC MODE'}</span>
            </div>
            <div class="network-badge">
                {networkMode || 'mainnet'}
            </div>
        </div>
    </div>

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

    <div class="control-bar">
        <div class="cmd-area">
            {#if !shellMode}
                <div class="cmd-dropdown-wrapper">
                    <button
                        class="cmd-dropdown-trigger"
                        on:click={toggleCommandDropdown}
                        disabled={shellMode}
                        title="Select a Hemp0x command"
                    >
                        <span class="cmd-dropdown-label">{selectedCommand || 'Commands'}</span>
                        <span class="cmd-dropdown-arrow">{showCommandDropdown ? '▲' : '▼'}</span>
                    </button>
                    {#if selectedCommand}
                        <button
                            class="cmd-help-btn"
                            title="Show help for {selectedCommand}"
                            on:click={() => showCommandHelp(selectedCommand)}
                        >?</button>
                    {/if}
                    {#if showCommandDropdown}
                        <div class="cmd-dropdown-panel" transition:fly={{ y: 8, duration: 120 }}>
                            <div class="cmd-dropdown-search">
                                <input
                                    class="cmd-dropdown-input"
                                    placeholder="Filter commands..."
                                    bind:value={commandDropdownSearch}
                                    bind:this={commandDropdownRef}
                                    on:keydown={handleDropdownKeydown}
                                />
                            </div>
                            <div class="cmd-dropdown-list">
                                {#if Object.keys(filteredDropdownCommands).length === 0}
                                    <div class="cmd-dropdown-empty">No commands match</div>
                                {:else}
                                    {#each Object.entries(filteredDropdownCommands) as [groupName, groupCmds]}
                                        <div class="cmd-dropdown-group">{groupName}</div>
                                        {#each groupCmds as cmd}
                                            <div
                                                class="cmd-dropdown-item"
                                                class:danger={DANGER_COMMANDS.has(cmd.toLowerCase())}
                                                on:click={() => handleCommandDropdownSelect(cmd)}
                                                role="button"
                                                tabindex="0"
                                                on:keydown={(e) => e.key === 'Enter' && handleCommandDropdownSelect(cmd)}
                                            >
                                                <span class="cmd-dropdown-item-name">{cmd}</span>
                                                <span class="cmd-dropdown-item-actions">
                                                    {#if DANGER_COMMANDS.has(cmd.toLowerCase())}
                                                        <span class="danger-badge">!</span>
                                                    {/if}
                                                    <button
                                                        type="button"
                                                        class="cmd-help-inline"
                                                        title="Show help for {cmd}"
                                                        on:click|stopPropagation={() => showCommandHelp(cmd)}
                                                    >?</button>
                                                </span>
                                            </div>
                                        {/each}
                                    {/each}
                                {/if}
                            </div>
                        </div>
                    {/if}
                </div>
            {:else}
                <div class="shell-indicator">$</div>
            {/if}

            <div class="input-wrapper grow">
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

        <div class="action-bar">
            <label class="option-toggle" title="Format JSON responses with indentation">
                <input type="checkbox" bind:checked={prettyJson} />
                <span>JSON</span>
            </label>
            <button class="cyber-btn" on:click={runCommand}>[ RUN ]</button>
            <button
                class="cyber-btn ghost"
                on:click={() => {
                    if (activeSession) activeSession.output = "";
                    syncActiveToConsoleOutput();
                }}
            >CLEAR</button>
            <button
                class="cyber-btn"
                class:ghost={!shellMode}
                class:shell-active={shellMode}
                on:click={toggleShellMode}
                title={shellMode ? "Switch to CLI/RPC mode" : "Switch to shell mode"}
            >
                {shellMode ? "SHELL ON" : "SHELL OFF"}
            </button>
        </div>
    </div>

    {#if showShellEnableConfirm}
        <div class="shell-enable-backdrop" transition:fly={{ y: -4, duration: 150 }}>
            <div class="shell-enable-modal">
                <div class="shell-enable-header">
                    <span class="shell-enable-warn-icon">&#9888;</span>
                    <span>ENABLE SHELL MODE</span>
                </div>
                <div class="shell-enable-body">
                    <p class="shell-enable-lead">
                        Shell mode runs <strong>unrestricted system commands</strong> on this computer.
                        This is different from CLI/RPC mode, which only sends commands to Hemp0x Core.
                    </p>
                    <div class="shell-enable-risks">
                        <span class="risk-label">Shell commands can:</span>
                        <ul>
                            <li>Read, modify, or delete any file you have access to</li>
                            <li>Install or remove software</li>
                            <li>Start or stop system services</li>
                            <li>Access network resources</li>
                        </ul>
                    </div>
                    <p class="shell-enable-note">This setting is stored locally and persists across restarts. You can disable it at any time.</p>
                </div>
                <div class="shell-enable-actions">
                    <button class="cyber-btn danger" on:click={confirmEnableShell}>ENABLE SHELL MODE</button>
                    <button class="cyber-btn ghost" on:click={cancelShellEnable}>CANCEL</button>
                </div>
            </div>
        </div>
    {/if}

    {#if showConsoleDisclaimer}
        <div class="console-disclaimer-backdrop" transition:fly={{ y: -4, duration: 150 }}>
            <div class="console-disclaimer-modal">
                <div class="console-disclaimer-header">
                    <span class="console-disclaimer-icon">&#9432;</span>
                    <span>CONSOLE NOTICE</span>
                </div>
                <div class="console-disclaimer-body">
                    <p class="console-disclaimer-lead">
                        The Console is an advanced tool for running Hemp0x Core commands and, if enabled separately, system shell commands.
                    </p>
                    <div class="console-disclaimer-panel">
                        <span class="console-disclaimer-label">Before using it:</span>
                        <ul>
                            <li>Check command help before running unfamiliar commands.</li>
                            <li>Review arguments carefully before broadcasting transactions or changing wallet state.</li>
                            <li>Do not paste commands you do not understand.</li>
                            <li>Shell mode remains disabled until you explicitly enable it.</li>
                        </ul>
                    </div>
                    <label class="console-disclaimer-toggle">
                        <input type="checkbox" bind:checked={hideConsoleDisclaimer} />
                        <span>Do not show this again</span>
                    </label>
                </div>
                <div class="console-disclaimer-actions">
                    <button class="cyber-btn" on:click={closeConsoleDisclaimer}>ENTER CONSOLE</button>
                </div>
            </div>
        </div>
    {/if}
</div>

<style>
    .console-view {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
        height: 100%;
        min-height: 0;
        padding-bottom: 0;
    }
    .full-height {
        height: 100%;
    }

    .console-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.45rem 0.7rem;
        background: rgba(0, 20, 10, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 8px;
        flex-shrink: 0;
    }
    .console-title {
        display: flex;
        align-items: center;
        gap: 0.6rem;
    }
    .console-title-text {
        font-size: 0.75rem;
        letter-spacing: 1.5px;
        color: var(--color-primary);
        font-weight: bold;
    }
    .console-status {
        font-size: 0.6rem;
        letter-spacing: 0.5px;
        color: #555;
    }
    .console-status.online {
        color: var(--color-primary);
    }
    .console-meta {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .console-notice-toggle {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 4px;
        height: 18px;
        padding: 0 5px;
        border: 1px solid rgba(0, 255, 65, 0.22);
        border-radius: 4px;
        background: rgba(0, 255, 65, 0.045);
        cursor: pointer;
    }
    .console-notice-toggle:hover {
        border-color: rgba(0, 255, 65, 0.45);
        background: rgba(0, 255, 65, 0.08);
    }
    .console-notice-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 11px;
        height: 11px;
        border: 1px solid rgba(0, 255, 65, 0.35);
        border-radius: 50%;
        color: var(--color-primary);
        font-size: 0.52rem;
        font-weight: bold;
        line-height: 1;
        font-family: var(--font-mono);
    }
    .console-notice-toggle input {
        width: 12px;
        height: 12px;
        margin: 0;
        accent-color: var(--color-primary);
        cursor: pointer;
    }
    .mode-pill {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 3px 10px;
        border-radius: 4px;
        font-size: 0.65rem;
        letter-spacing: 0.5px;
        background: rgba(0, 255, 65, 0.07);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
    }
    .mode-pill.shell {
        background: rgba(255, 165, 0, 0.07);
        border-color: rgba(255, 165, 0, 0.3);
        color: #ffa500;
    }
    .mode-pill-icon {
        font-size: 0.8rem;
    }
    .network-badge {
        padding: 3px 8px;
        border-radius: 4px;
        font-size: 0.6rem;
        letter-spacing: 0.5px;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: #777;
        text-transform: uppercase;
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
        padding: 3px 10px;
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
    .tab-btn.shell {
        border-color: rgba(255, 165, 0, 0.15);
        color: #a67c00;
    }
    .tab-btn.shell:hover {
        border-color: rgba(255, 165, 0, 0.3);
        color: #d49a00;
        background: rgba(30, 20, 0, 0.6);
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
        padding: 3px 10px;
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
        min-height: 0;
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
        flex-shrink: 0;
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
        padding: 0.4rem 0.7rem;
        font-size: 0.8rem;
        resize: none;
        outline: none;
        white-space: pre-wrap;
        position: relative;
        z-index: 1;
        min-height: 0;
    }

    .control-bar {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.35rem 0.5rem;
        background: rgba(0, 20, 10, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        flex-shrink: 0;
        margin-bottom: 0;
    }
    .cmd-area {
        display: flex;
        align-items: center;
        flex: 1;
        gap: 0.4rem;
        min-width: 0;
    }
    .shell-indicator {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 2rem;
        height: 2rem;
        background: rgba(255, 165, 0, 0.08);
        border: 1px solid rgba(255, 165, 0, 0.3);
        color: #ffa500;
        border-radius: 4px;
        font-weight: bold;
        font-size: 0.85rem;
        flex-shrink: 0;
    }
    .input-wrapper {
        position: relative;
        flex: 1;
        min-width: 0;
    }
    .action-bar {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        flex-shrink: 0;
    }
    .option-toggle {
        display: flex;
        align-items: center;
        gap: 4px;
        font-size: 0.65rem;
        color: #666;
        cursor: pointer;
        white-space: nowrap;
    }
    .option-toggle input {
        accent-color: var(--color-primary);
    }

    .cmd-dropdown-wrapper {
        position: relative;
        flex-shrink: 0;
        display: flex;
        align-items: center;
        gap: 2px;
    }
    .cmd-dropdown-trigger {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 0.4rem 0.6rem;
        background: #111;
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: #00ff41;
        border-radius: 4px;
        font-family: inherit;
        font-size: 0.75rem;
        cursor: pointer;
        white-space: nowrap;
    }
    .cmd-dropdown-trigger:hover {
        border-color: var(--color-primary);
    }
    .cmd-dropdown-trigger:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .cmd-dropdown-label {
        min-width: 80px;
        text-align: left;
    }
    .cmd-dropdown-arrow {
        font-size: 0.6rem;
        color: #888;
    }

    .cmd-dropdown-panel {
        position: absolute;
        bottom: calc(100% + 4px);
        left: 0;
        width: 280px;
        max-height: 260px;
        background: rgba(10, 10, 10, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.35);
        border-radius: 6px;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        z-index: 200;
        box-shadow: 0 -10px 40px rgba(0, 0, 0, 0.8);
    }
    .cmd-dropdown-search {
        padding: 6px 8px;
        border-bottom: 1px solid rgba(0, 255, 65, 0.15);
        flex-shrink: 0;
    }
    .cmd-dropdown-input {
        width: 100%;
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.25);
        color: #00ff41;
        padding: 4px 8px;
        font-size: 0.75rem;
        border-radius: 3px;
        outline: none;
        font-family: inherit;
    }
    .cmd-dropdown-input:focus {
        border-color: var(--color-primary);
    }
    .cmd-dropdown-list {
        overflow-y: auto;
        flex: 1;
        padding: 4px 0;
    }
    .cmd-dropdown-group {
        padding: 5px 10px;
        font-size: 0.65rem;
        color: #888;
        letter-spacing: 1.5px;
        text-transform: uppercase;
        font-weight: bold;
        background: rgba(0, 255, 65, 0.04);
        border-top: 1px solid rgba(0, 255, 65, 0.12);
        border-bottom: 1px solid rgba(0, 255, 65, 0.08);
    }
    .cmd-dropdown-group:first-child {
        border-top: none;
    }
    .cmd-dropdown-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        width: 100%;
        background: transparent;
        border: none;
        color: #00ff41;
        padding: 3px 10px;
        font-size: 0.75rem;
        cursor: pointer;
        text-align: left;
        font-family: inherit;
    }
    .cmd-dropdown-item:hover {
        background: rgba(0, 255, 65, 0.1);
    }
    .cmd-dropdown-item.danger {
        color: #ffa500;
    }
    .cmd-dropdown-item.danger:hover {
        background: rgba(255, 165, 0, 0.1);
    }
    .cmd-dropdown-item-name {
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .cmd-dropdown-item-actions {
        display: flex;
        align-items: center;
        gap: 4px;
        flex-shrink: 0;
    }
    .cmd-help-inline {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 16px;
        height: 16px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #888;
        border-radius: 50%;
        font-size: 0.6rem;
        cursor: pointer;
        line-height: 1;
        padding: 0;
        font-family: inherit;
    }
    .cmd-help-inline:hover {
        background: rgba(0, 255, 65, 0.15);
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .cmd-help-btn {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 24px;
        height: 24px;
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.15);
        color: #888;
        border-radius: 4px;
        font-size: 0.75rem;
        cursor: pointer;
        line-height: 1;
        margin-left: 2px;
    }
    .cmd-help-btn:hover {
        background: rgba(0, 255, 65, 0.1);
        border-color: var(--color-primary);
        color: var(--color-primary);
    }
    .cmd-dropdown-empty {
        padding: 10px;
        text-align: center;
        color: #555;
        font-size: 0.75rem;
    }

    .input-glass {
        width: 100%;
        background: #111;
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: #00ff41;
        padding: 0.45rem 0.6rem;
        border-radius: 4px;
        font-family: inherit;
        font-size: 0.8rem;
        outline: none;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
    }

    .cyber-btn {
        background: rgba(0, 255, 65, 0.05);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 0.45rem 0.8rem;
        letter-spacing: 1px;
        font-weight: bold;
        transition: all 0.2s;
        cursor: pointer;
        text-transform: uppercase;
        font-size: 0.7rem;
        white-space: nowrap;
        border-radius: 4px;
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
    .cyber-btn.danger {
        border-color: rgba(255, 165, 0, 0.5);
        color: #ffa500;
        background: rgba(255, 165, 0, 0.08);
    }
    .cyber-btn.danger:hover {
        background: #ffa500;
        color: #000;
        box-shadow: 0 0 12px rgba(255, 165, 0, 0.3);
    }
    .cyber-btn.shell-active {
        border-color: rgba(255, 165, 0, 0.5);
        color: #ffa500;
        background: rgba(255, 165, 0, 0.08);
    }
    .cyber-btn.shell-active:hover {
        background: rgba(255, 165, 0, 0.15);
        color: #ffa500;
        box-shadow: 0 0 10px rgba(255, 165, 0, 0.2);
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

    .shell-enable-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.85);
        z-index: 1000;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 1rem;
    }
    .shell-enable-modal {
        width: 100%;
        max-width: 520px;
        background: rgba(10, 10, 10, 0.95);
        border: 1px solid rgba(255, 165, 0, 0.35);
        border-radius: 10px;
        padding: 1.5rem;
        display: flex;
        flex-direction: column;
        gap: 1rem;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    }
    .shell-enable-header {
        display: flex;
        align-items: center;
        gap: 10px;
        font-size: 0.9rem;
        color: #ffa500;
        letter-spacing: 1px;
        font-weight: bold;
        border-bottom: 1px solid rgba(255, 165, 0, 0.15);
        padding-bottom: 0.6rem;
    }
    .shell-enable-warn-icon {
        font-size: 1.4rem;
    }
    .shell-enable-body {
        color: #bbb;
        font-size: 0.8rem;
        line-height: 1.6;
    }
    .shell-enable-lead {
        margin: 0 0 0.6rem 0;
    }
    .shell-enable-risks {
        background: rgba(255, 165, 0, 0.06);
        border: 1px solid rgba(255, 165, 0, 0.15);
        border-radius: 6px;
        padding: 0.7rem 1rem;
        margin: 0.4rem 0;
    }
    .risk-label {
        display: block;
        color: #ffa500;
        font-weight: bold;
        font-size: 0.75rem;
        margin-bottom: 0.3rem;
    }
    .shell-enable-risks ul {
        margin: 0;
        padding-left: 1.2rem;
    }
    .shell-enable-risks li {
        margin: 0.2rem 0;
    }
    .shell-enable-note {
        margin: 0.6rem 0 0 0;
        font-size: 0.75rem;
        color: #888;
    }
    .shell-enable-actions {
        display: flex;
        gap: 0.6rem;
        justify-content: flex-end;
        padding-top: 0.4rem;
    }

    .console-disclaimer-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.78);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10000;
        padding: 1rem;
        backdrop-filter: blur(4px);
    }
    .console-disclaimer-modal {
        width: min(560px, 100%);
        background: rgba(3, 12, 8, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.35);
        box-shadow: 0 0 42px rgba(0, 255, 65, 0.12), 0 24px 80px rgba(0, 0, 0, 0.8);
        border-radius: 8px;
        overflow: hidden;
        color: #d8ffe2;
    }
    .console-disclaimer-header {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.9rem 1rem;
        border-bottom: 1px solid rgba(0, 255, 65, 0.18);
        color: var(--color-primary);
        font-weight: bold;
        letter-spacing: 1.2px;
        font-size: 0.82rem;
    }
    .console-disclaimer-icon {
        color: #00ff41;
        font-size: 1rem;
    }
    .console-disclaimer-body {
        padding: 1rem;
        display: flex;
        flex-direction: column;
        gap: 0.8rem;
    }
    .console-disclaimer-lead {
        margin: 0;
        color: #c9f7d2;
        line-height: 1.5;
        font-size: 0.86rem;
    }
    .console-disclaimer-panel {
        border: 1px solid rgba(0, 255, 65, 0.16);
        background: rgba(0, 255, 65, 0.045);
        border-radius: 6px;
        padding: 0.75rem 0.9rem;
    }
    .console-disclaimer-label {
        display: block;
        color: var(--color-primary);
        font-size: 0.72rem;
        font-weight: bold;
        letter-spacing: 1px;
        margin-bottom: 0.45rem;
    }
    .console-disclaimer-panel ul {
        margin: 0;
        padding-left: 1rem;
        color: #b9d8c0;
        line-height: 1.55;
        font-size: 0.8rem;
    }
    .console-disclaimer-toggle {
        display: flex;
        align-items: center;
        gap: 0.45rem;
        color: #9ab8a2;
        font-size: 0.78rem;
        cursor: pointer;
        user-select: none;
    }
    .console-disclaimer-toggle input {
        accent-color: var(--color-primary);
    }
    .console-disclaimer-actions {
        display: flex;
        justify-content: flex-end;
        padding: 0 1rem 1rem;
    }

    @media (max-width: 800px) {
        .control-bar {
            flex-wrap: wrap;
        }
        .cmd-area {
            width: 100%;
        }
    }
</style>
