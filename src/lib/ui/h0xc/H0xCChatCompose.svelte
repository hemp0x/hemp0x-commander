<script>
    import { createEventDispatcher } from "svelte";
    import { core } from "@tauri-apps/api";
    import { fade } from "svelte/transition";
    import { deriveRootNameFn } from "../../stores/h0xc.js";

    export let identity = "";
    export let isGuest = false;
    export let busy = false;
    export let error = "";

    const dispatch = createEventDispatcher();

    let composeText = "";
    let composeEncoding = false;
    /** @type {any} */
    let composeResult = null;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let composeDebounce = null;
    /** @type {HTMLTextAreaElement | null} */
    let textareaEl = null;

    function getChannel() {
        return identity.endsWith("!") ? identity : `${identity}!`;
    }

    function getRootLabel() {
        if (!identity) return "";
        return deriveRootNameFn(identity).toUpperCase();
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
            composeResult = await core.invoke("short_message_encode", { text });
        } catch {
            composeResult = null;
        } finally {
            composeEncoding = false;
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
        if (!composeResult?.fits || busy) return;
        dispatch("send", { hex: composeResult.hex, text: composeText.trim() });
    }

    export function clearCompose() {
        composeText = "";
        composeResult = null;
    }
</script>

<div class="h0xc-compose" transition:fade={{ duration: 100 }}>
    {#if error}
        <div class="compose-error">{error}</div>
    {/if}
    {#if isGuest}
        <div class="compose-row guest-row">
            <span class="guest-cta">Create a .H0XC identity to send messages</span>
        </div>
    {:else}
        <div class="compose-row">
        <span class="compose-channel" title={getChannel()}>{getRootLabel()}</span>
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
        {#if composeResult}
            {@const used = composeResult.encoded_payload_len || 0}
            {@const max = 27}
            <div class="compose-status"
                class:ok={composeResult.fits && used < 20}
                class:warn={composeResult.fits && used >= 20}
                class:over={!composeResult.fits}
            >
                <span class="status-text">{used}/{max}</span>
            </div>
        {/if}
        <button
            class="compose-send"
            disabled={busy || !composeResult?.fits}
            on:click={send}
        >
            {#if busy}
                ...
            {:else if composeEncoding}
                ...
            {:else}
                ▶
            {/if}
        </button>
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
    .compose-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }
    .guest-row {
        justify-content: center;
        padding: 0.3rem 0;
    }
    .guest-cta {
        font-size: 0.56rem;
        color: #666;
        font-style: italic;
    }
    .compose-channel {
        font-size: 0.5rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 0.5px;
        flex-shrink: 0;
        min-width: 2.5rem;
        text-align: center;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 999px;
        padding: 0.15rem 0.4rem;
    }
    .compose-input {
        flex: 1;
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
    .compose-input:focus {
        border-color: var(--color-primary);
    }
    .compose-input::placeholder {
        color: #444;
    }
    .compose-input:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .compose-status {
        font-size: 0.5rem;
        font-weight: 600;
        padding: 0.15rem 0.3rem;
        border-radius: 4px;
        flex-shrink: 0;
        font-family: var(--font-mono);
    }
    .compose-status.ok {
        color: var(--color-primary);
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.2);
    }
    .compose-status.warn {
        color: #ffcc00;
        background: rgba(255, 204, 0, 0.1);
        border: 1px solid rgba(255, 204, 0, 0.2);
    }
    .compose-status.over {
        color: #ff5555;
        background: rgba(255, 85, 85, 0.1);
        border: 1px solid rgba(255, 85, 85, 0.2);
    }
    .status-text { font-family: var(--font-mono); }
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
    .compose-send:hover:not(:disabled) {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 16px rgba(0, 255, 65, 0.3);
    }
    .compose-send:disabled {
        opacity: 0.35;
        cursor: not-allowed;
    }
</style>
