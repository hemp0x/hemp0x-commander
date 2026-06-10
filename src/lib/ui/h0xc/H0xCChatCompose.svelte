<script>
    import { onMount, createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import { insertSuggestion } from "../../utils.js";
    import ShortMessageAutocomplete from "../ShortMessageAutocomplete.svelte";

    export let isGuest = false;
    export let busy = false;
    export let error = "";

    const dispatch = createEventDispatcher();

    function requestIdentity() {
        dispatch("requestIdentity");
    }

    let composeText = "";
    let composeEncoding = false;
    /** @type {any} */
    let composeResult = null;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let composeDebounce = null;
    /** @type {HTMLTextAreaElement | null} */
    let textareaEl = null;
    let textareaFocused = false;

    let autocompleteEnabled = true;
    let packPreparing = true;
    /** @type {string[]} */
    let emojiList = [];
    let emojiOpen = false;
    /** @type {HTMLDivElement | null} */
    let emojiWrap = null;

    onMount(() => {
        preparePack();
    });

    async function preparePack() {
        packPreparing = true;
        try {
            await core.invoke("short_message_prepare_built_in_table_pack");
        } catch {}
        packPreparing = false;
    }

    function queueEncode() {
        if (composeDebounce) clearTimeout(composeDebounce);
        composeResult = null;
        composeDebounce = setTimeout(encodeShort, 250);
    }

    async function encodeShort() {
        const text = composeText.trim();
        if (!text) {
            composeResult = null;
            return;
        }
        composeEncoding = true;
        try {
            composeResult = await core.invoke("short_message_encode_chat_built_in", { text });
        } catch {
            composeResult = null;
        } finally {
            composeEncoding = false;
        }
    }

    async function ensureEmojis() {
        if (emojiList.length > 0) return;
        try {
            const result = await core.invoke("short_message_emojis_built_in");
            emojiList = Array.isArray(result) ? result : [];
        } catch {
            emojiList = [];
        }
    }

    function toggleEmojiPicker() {
        emojiOpen = !emojiOpen;
        if (emojiOpen) ensureEmojis();
    }

    function insertEmoji(/** @type {string} */ emoji) {
        if (!emoji || busy) return;
        const ta = textareaEl;
        if (!ta) {
            composeText = `${composeText}${emoji}`;
            queueEncode();
            return;
        }
        const start = ta.selectionStart ?? composeText.length;
        const end = ta.selectionEnd ?? composeText.length;
        composeText = composeText.slice(0, start) + emoji + composeText.slice(end);
        queueEncode();
        requestAnimationFrame(() => {
            ta.focus();
            const next = start + emoji.length;
            ta.setSelectionRange(next, next);
        });
    }

    function handleSuggestion(/** @type {string} */ suggestion) {
        const next = insertSuggestion(composeText, suggestion);
        composeText = next;
        queueEncode();
        return next;
    }

    /** @param {FocusEvent} e */
    function handleFocusOut(e) {
        const related = e.relatedTarget;
        if (emojiWrap && related instanceof Node && emojiWrap.contains(related)) return;
        textareaFocused = false;
        emojiOpen = false;
    }

    /** @param {MouseEvent} e */
    function handleWindowClick(e) {
        const target = e.target instanceof Node ? e.target : null;
        if (emojiOpen && emojiWrap && (!target || !emojiWrap.contains(target))) {
            emojiOpen = false;
        }
    }

    /** @param {KeyboardEvent} e */
    function handleKeydown(e) {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            send();
        }
    }

    function send() {
        if (busy) return;
        const text = composeText.trim();
        if (!text) return;
        if (!composeResult?.fits) return;
        dispatch("send", { hex: composeResult.hex, text, mode: "short" });
    }

    export function clearCompose() {
        composeText = "";
        composeResult = null;
    }

    $: canSend = !busy && composeText.trim().length > 0 && !!composeResult?.fits;
</script>

<svelte:window on:click={handleWindowClick} />

<div class="h0xc-compose" transition:fade={{ duration: 100 }}>
    {#if error}
        <div class="compose-error">{error}</div>
    {/if}
    {#if isGuest}
        <div class="compose-row guest-row">
            <span class="guest-cta">Read-only guest mode</span>
            <button class="guest-action" on:click={requestIdentity}>Choose or Create Identity</button>
        </div>
    {:else}
        <div class="compose-main">
            <div class="textarea-wrap"
                on:focusin={() => { textareaFocused = true; ensureEmojis(); }}
                on:focusout={handleFocusOut}
            >
                <textarea
                    bind:this={textareaEl}
                    bind:value={composeText}
                    on:input={queueEncode}
                    on:keydown={handleKeydown}
                    placeholder="Type a message..."
                    rows="2"
                    disabled={busy}
                    class="compose-input"
                ></textarea>
                {#if autocompleteEnabled && !busy && !packPreparing}
                    <ShortMessageAutocomplete
                        text={composeText}
                        disabled={busy}
                        targetElement={textareaEl}
                        onAccept={handleSuggestion}
                        focused={textareaFocused}
                        preferredDict={composeResult?.dictionary_index ?? null}
                        suggestionCommand="short_message_suggestions_built_in"
                    />
                {/if}
            </div>
            <button class="compose-send" disabled={!canSend} on:click={send}>
                {#if busy || composeEncoding}...{:else}▶{/if}
            </button>
        </div>
        <div class="tools-row">
            {#if packPreparing}
                <span class="pack-loading">Loading predictions...</span>
            {:else}
                <label class="pred-toggle">
                    <input type="checkbox" bind:checked={autocompleteEnabled} />
                    <span>Predict</span>
                </label>
            {/if}
            <div class="emoji-picker-wrap" bind:this={emojiWrap}>
                <button class="emoji-toggle" type="button" title="Emoji" on:click|stopPropagation={toggleEmojiPicker} disabled={busy}>☺</button>
                {#if emojiOpen && emojiList.length > 0}
                    <div class="emoji-grid">
                        {#each emojiList as emoji}
                            <button type="button" class="emoji-btn" on:click={() => insertEmoji(emoji)} disabled={busy}>{emoji}</button>
                        {/each}
                    </div>
                {/if}
            </div>
            {#if composeResult}
                {@const used = composeResult.encoded_payload_len || 0}
                <span class="compose-status"
                    class:ok={composeResult.fits && used < 20}
                    class:warn={composeResult.fits && used >= 20}
                    class:over={!composeResult.fits}
                >
                    {used}/27
                </span>
            {/if}
            {#if composeResult?.dictionary_name}
                <span class="dict-chip">DICT {composeResult.dictionary_name}</span>
            {/if}
        </div>
    {/if}
</div>

<style>
    .h0xc-compose {
        border-top: 1px solid rgba(255, 255, 255, 0.06);
        padding: 0.45rem 0.6rem;
        background: rgba(0, 0, 0, 0.3);
    }
    .compose-error {
        font-size: 0.55rem;
        color: #ff5555;
        margin-bottom: 0.3rem;
        padding: 0.15rem 0;
    }
    .compose-main {
        display: flex;
        align-items: stretch;
        gap: 0.4rem;
    }

    .guest-row { justify-content: center; padding: 0.3rem 0; }
    .guest-cta { font-size: 0.56rem; color: #666; font-style: italic; }
    .guest-action {
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 5px;
        padding: 0.3rem 0.6rem;
        color: var(--color-primary);
        font-size: 0.58rem;
        font-weight: 600;
        letter-spacing: 0.4px;
        cursor: pointer;
        transition: all 0.15s;
    }
    .guest-action:hover { background: var(--color-primary); color: #000; }

    .textarea-wrap {
        flex: 1;
        position: relative;
    }
    .compose-input {
        width: 100%;
        background: rgba(0, 0, 0, 0.5);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 6px;
        padding: 0.35rem 0.5rem;
        color: #e0e0e0;
        font-size: 0.68rem;
        font-family: var(--font-mono);
        resize: none;
        outline: none;
        transition: border-color 0.15s;
        min-height: 2rem;
    }
    .compose-input:focus { border-color: var(--color-primary); }
    .compose-input::placeholder { color: #444; }
    .compose-input:disabled { opacity: 0.5; cursor: not-allowed; }

    .compose-status {
        font-size: 0.5rem;
        font-weight: 600;
        padding: 0.15rem 0.3rem;
        border-radius: 4px;
        font-family: var(--font-mono);
    }
    .compose-status.ok { color: var(--color-primary); background: rgba(0, 255, 65, 0.1); border: 1px solid rgba(0, 255, 65, 0.2); }
    .compose-status.warn { color: #ffcc00; background: rgba(255, 204, 0, 0.1); border: 1px solid rgba(255, 204, 0, 0.2); }
    .compose-status.over { color: #ff5555; background: rgba(255, 85, 85, 0.1); border: 1px solid rgba(255, 85, 85, 0.2); }

    .compose-send {
        width: 2rem;
        height: 2rem;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid var(--color-primary);
        border-radius: 6px;
        color: var(--color-primary);
        font-size: 0.8rem;
        cursor: pointer;
        flex-shrink: 0;
        transition: all 0.15s;
    }
    .compose-send:hover:not(:disabled) { background: var(--color-primary); color: #000; box-shadow: 0 0 16px rgba(0, 255, 65, 0.3); }
    .compose-send:disabled { opacity: 0.35; cursor: not-allowed; }

    .tools-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-top: 0.25rem;
    }
    .pred-toggle {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        font-size: 0.5rem;
        color: #555;
        cursor: pointer;
    }
    .pred-toggle input[type="checkbox"] {
        width: 10px;
        height: 10px;
        accent-color: var(--color-primary);
        cursor: pointer;
    }

    .emoji-picker-wrap {
        position: relative;
    }
    .emoji-toggle {
        background: none;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 4px;
        color: #555;
        font-size: 0.7rem;
        cursor: pointer;
        padding: 0.1rem 0.3rem;
        transition: all 0.15s;
    }
    .emoji-toggle:hover { border-color: rgba(255, 170, 0, 0.3); color: #aaa; }
    .emoji-toggle:disabled { opacity: 0.35; cursor: not-allowed; }

    .emoji-grid {
        position: absolute;
        bottom: 100%;
        left: 0;
        margin-bottom: 4px;
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 6px;
        padding: 0.3rem;
        display: grid;
        grid-template-columns: repeat(8, 1fr);
        gap: 0.15rem;
        z-index: 10;
        box-shadow: 0 -4px 16px rgba(0, 0, 0, 0.6);
        max-height: 8rem;
        overflow-y: auto;
    }
    .emoji-btn {
        width: 1.5rem;
        height: 1.5rem;
        display: flex;
        align-items: center;
        justify-content: center;
        background: none;
        border: 1px solid transparent;
        border-radius: 4px;
        cursor: pointer;
        font-size: 0.8rem;
        transition: all 0.1s;
    }
    .emoji-btn:hover { background: rgba(0, 255, 65, 0.1); border-color: rgba(0, 255, 65, 0.2); }
    .emoji-btn:disabled { opacity: 0.35; cursor: not-allowed; }

    .dict-chip {
        font-size: 0.45rem;
        font-weight: 600;
        color: #666;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 3px;
        padding: 0.1rem 0.35rem;
        letter-spacing: 0.5px;
    }
    .pack-loading {
        font-size: 0.48rem;
        color: var(--color-primary);
        opacity: 0.6;
        animation: pulse-loading 1.2s ease-in-out infinite;
    }
    @keyframes pulse-loading {
        0%, 100% { opacity: 0.4; }
        50% { opacity: 0.8; }
    }
</style>
