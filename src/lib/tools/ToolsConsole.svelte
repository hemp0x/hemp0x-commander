<script>
    import { tick, createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";
    import { nodeStatus, networkInfo } from "../../stores.js"; // Import Stores

    export let consoleOutput = "";
    export let consoleHistory = [];
    // export let isNodeOnline = false; // DEPRECTAED
    // export let networkMode = "mainnet"; // DEPRECATED

    $: isNodeOnline = $nodeStatus.online;
    $: networkMode = $networkInfo.chain;

    export let isProcessing = false;
    export let processingMessage = "";

    const dispatch = createEventDispatcher();

    // Console State
    let selectedCommand = "";
    let cmdLine = "";
    let shellMode = false;
    let shellWarningShown = false;
    let historyIndex = -1;
    let consoleRef;

    $: cmdPlaceholder = shellMode ? "type a shell command" : "command args";

    // Commands grouped for better UX
    const commandGroups = {
        "Wallet & Payments": [
            "getbalance",
            "getunconfirmedbalance",
            "getwalletinfo",
            "listtransactions",
            "listunspent",
            "listlockunspent",
            "lockunspent",
            "listaccounts",
            "listwallets",
            "listaddressgroupings",
            "listreceivedbyaccount",
            "listreceivedbyaddress",
            "getnewaddress",
            "getaccountaddress",
            "getaccount",
            "setaccount",
            "getreceivedbyaccount",
            "getreceivedbyaddress",
            "getaddressesbyaccount",
            "sendtoaddress",
            "sendfrom",
            "sendmany",
            "settxfee",
            "validateaddress",
            "signmessage",
            "verifymessage",
            "backupwallet",
            "encryptwallet",
            "walletpassphrase",
            "walletlock",
            "dumpwallet",
            "importwallet",
        ],
        Assets: [
            "listassets",
            "listmyassets",
            "getassetdata",
            "issue",
            "issueunique",
            "issuerestrictedasset",
            "issuequalifierasset",
            "reissue",
            "reissuerestrictedasset",
            "transfer",
            "transferfromaddress",
            "transferfromaddresses",
            "transferqualifier",
            "listaddressrestrictions",
            "listglobalrestrictions",
            "checkaddressrestriction",
            "checkglobalrestriction",
            "freezeaddress",
            "unfreezeaddress",
            "freezerestrictedasset",
            "unfreezerestrictedasset",
            "viewmyrestrictedaddresses",
            "viewmytaggedaddresses",
            "addtagtoaddress",
            "removetagfromaddress",
            "listtagsforaddress",
            "distributereward",
            "getdistributestatus",
        ],
        "Mining & Chain": [
            "getblockcount",
            "getbestblockhash",
            "getdifficulty",
            "getnetworkhashps",
            "getmininginfo",
            "getblock",
            "getblockhash",
            "getblockheader",
            "getchaintips",
            "getblockchaininfo",
            "getmempoolinfo",
            "getrawmempool",
            "gettxoutsetinfo",
            "generate",
            "generatetoaddress",
            "getgenerate",
            "setgenerate",
            "submitblock",
        ],
        Network: [
            "getinfo",
            "getpeerinfo",
            "getconnectioncount",
            "getnettotals",
            "getnetworkinfo",
            "ping",
            "addnode",
            "disconnectnode",
            "setban",
            "listbanned",
            "clearbanned",
            "setnetworkactive",
        ],
        "Advanced / Raw": [
            "createrawtransaction",
            "decoderawtransaction",
            "signrawtransaction",
            "sendrawtransaction",
            "getrawtransaction",
            "fundrawtransaction",
            "combinerawtransaction",
            "dumpprivkey",
            "importprivkey",
            "importaddress",
            "importpubkey",
            "importmulti",
            "importprunedfunds",
            "removeprunedfunds",
            "addmultisigaddress",
            "createmultisig",
            "signmessagewithprivkey",
        ],
        "System & Debug": [
            "help",
            "stop",
            "uptime",
            "getmemoryinfo",
            "getrpcinfo",
            "getcacheinfo",
            "checkaddresstag",
            "isvalidverifierstring",
            "getverifierstring",
        ],
    };

    // Flatten for autocomplete
    const commands = Object.values(commandGroups).flat().sort();

    function showToast(msg, type = "info") {
        dispatch("toast", { msg, type });
    }

    async function appendOutput(text) {
        if (!text) return;
        consoleOutput = consoleOutput ? `${consoleOutput}\n${text}` : text;
        await tick();
        if (consoleRef) {
            consoleRef.scrollTop = consoleRef.scrollHeight;
        }
    }

    async function runCommand() {
        // if (!isNodeOnline && !shellMode) {
        //   showToast("Node is offline", "error");
        // }

        try {
            const line = cmdLine.trim();
            if (!line) {
                appendOutput("Enter a command to run");
                showToast("No command entered", "error");
                return;
            }

            // SAFETY CHECK REMOVED per user request for fluent workflow.

            if (consoleHistory[consoleHistory.length - 1] !== line) {
                consoleHistory = [...consoleHistory, line];
            }
            historyIndex = consoleHistory.length;

            // Show processing overlay for long-running commands
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

            isProcessing = false;
            processingMessage = "";
            appendOutput(res || "(no output)");
            showToast("Command Executed", "success");
            cmdLine = "";
            selectedCommand = ""; // Reset dropdown
        } catch (err) {
            isProcessing = false;
            processingMessage = "";
            appendOutput(`Error: ${err}`);
            showToast("Command Failed", "error");
        }
    }

    function handleCommandSelect(event) {
        selectedCommand = event.target.value;
        cmdLine = selectedCommand ? selectedCommand : "";
        if (selectedCommand) {
            setTimeout(() => {
                selectedCommand = "";
            }, 100);
        }
    }

    function toggleShellMode() {
        if (!shellMode && !shellWarningShown) {
            shellWarningShown = true;
            appendOutput(
                "Shell Mode runs commands on this computer. Use it only when you understand the command and trust the input. Wallet secrets typed here may appear in history or output.",
            );
        }
        shellMode = !shellMode;
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
        if (shellMode) {
            try {
                const matches = await core.invoke("shell_autocomplete", {
                    line: cmdLine,
                });
                if (!matches || matches.length === 0) {
                    return;
                }
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
        const options = commands.filter((cmd) => cmd.startsWith(token));
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
            historyIndex = Math.max(0, historyIndex - 1);
            cmdLine = consoleHistory[historyIndex] || "";
            return;
        }
        if (event.key === "ArrowDown") {
            if (consoleHistory.length === 0) return;
            event.preventDefault();
            historyIndex = Math.min(consoleHistory.length, historyIndex + 1);
            cmdLine =
                historyIndex >= consoleHistory.length
                    ? ""
                    : consoleHistory[historyIndex];
            return;
        }
        if (event.key === "Tab") {
            event.preventDefault();
            handleAutocomplete();
        }
    }
</script>

<div class="tool-grid console-view full-height">
    <div class="terminal-screen">
        <div class="scanline"></div>
        <textarea
            class="console-output mono"
            readonly
            bind:value={consoleOutput}
            bind:this={consoleRef}
        ></textarea>
    </div>

    <div class="control-bar compact">
        <div class="field-group grow">
            <label for="cmd-select">COMMAND LIST</label>
            <div class="input-wrapper brackets">
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
                                <option value={cmd}>{cmd}</option>
                            {/each}
                        </optgroup>
                    {/each}
                </select>
            </div>
        </div>
        <div class="field-group grow">
            <label for="cmd-line">ARGUMENT</label>
            <div class="input-wrapper brackets">
                <input
                    id="cmd-line"
                    class="input-glass mono"
                    placeholder={cmdPlaceholder}
                    bind:value={cmdLine}
                    on:keydown={handlePromptKeydown}
                />
            </div>
        </div>
        <div class="action-group">
            <button class="cyber-btn" on:click={runCommand}>[ RUN ]</button>
            <button
                class="cyber-btn ghost"
                on:click={() => (consoleOutput = "")}>CLEAR</button
            >
            <button class="cyber-btn ghost" on:click={toggleShellMode}>
                {shellMode ? "SHELL MODE" : "CLI MODE"}
            </button>
        </div>
    </div>
</div>

<style>
    /* Styles are inherited from parent or global css, but some specific ones might need moving if scoped in ViewTools - checking ViewTools styles */
    .tool-grid {
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }
    .full-height {
        height: 100%;
    }
    .terminal-screen {
        flex: 1;
        background: rgba(0, 0, 0, 0.6);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        position: relative;
        overflow: hidden;
        display: flex;
    }
    .scanline {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        height: 2px;
        background: rgba(0, 255, 65, 0.1);
        opacity: 0.5;
        animation: scan 6s linear infinite;
        pointer-events: none;
    }
    @keyframes scan {
        0% {
            transform: translateY(-100%);
        }
        100% {
            transform: translateY(1000%);
        }
    }
    .console-output {
        flex: 1;
        background: transparent;
        border: none;
        color: #00ff41;
        padding: 1rem;
        font-size: 0.8rem;
        resize: none;
        outline: none;
        white-space: pre-wrap;
    }
    .control-bar {
        background: rgba(0, 20, 10, 0.6);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        padding: 1rem;
        display: flex;
        gap: 1rem;
        align-items: flex-end;
    }
    .control-bar.compact {
        padding: 0.8rem;
    }
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .field-group.grow {
        flex: 1;
    }
    .input-wrapper {
        position: relative;
    }
    .action-group {
        display: flex;
        gap: 0.5rem;
    }

    label {
        font-size: 0.6rem;
        color: #666;
        letter-spacing: 1px;
    }

    /* Input styles presumed global or needing import - keeping minimal here */
    .input-glass {
        width: 100%;
        background: #111; /* Force solid dark background (fixes Linux transparency issues) */
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: #00ff41; /* Matrix green text */
        padding: 0.6rem;
        border-radius: 4px;
        font-family: inherit;
    }
    .input-glass:focus {
        border-color: var(--color-primary);
        outline: none;
    }

    /* LINUX FIX: Deep OS Override */
    select.input-glass {
        appearance: none; /* Remove native OS styling */
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
</style>
