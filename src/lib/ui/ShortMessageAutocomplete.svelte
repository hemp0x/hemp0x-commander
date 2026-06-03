<script>
    import { core } from "@tauri-apps/api";
    import { fly } from "svelte/transition";

    export let text = "";
    export let disabled = false;
    /** @type {HTMLElement|null} */
    export let targetElement = null;
    /** @type {(suggestion: string) => void} */
    export let onAccept = () => {};
    export let focused = false;

    let open = false;
    let activeIndex = -1;
    /** @type {string[]} */
    let suggestions = [];
    let loading = false;
    /** @type {ReturnType<typeof setTimeout> | null} */
    let debounceTimer = null;
    let lastFetchedPrefix = "";
    let suppressForText = "";
    /** @type {HTMLDivElement | null} */
    let panelRef = null;
    let requestId = 0;

    const MAX_SUGGESTIONS = 6;
    const DEBOUNCE_MS = 150;

    function close() {
        open = false;
        activeIndex = -1;
    }

    /**
     * @param {string} prefix
     * @param {number} currentRequestId
     */
    async function fetchSuggestions(prefix, currentRequestId) {
        if (!prefix || prefix.trim().length === 0) {
            close();
            suggestions = [];
            return;
        }
        loading = true;
        try {
            const result = await core.invoke("short_message_suggestions", {
                prefix,
                context: null,
            });
            if (currentRequestId !== requestId) {
                return;
            }
            if (Array.isArray(result) && result.length > 0) {
                suggestions = result.slice(0, MAX_SUGGESTIONS);
                if (focused) {
                    open = true;
                }
                activeIndex = -1;
            } else {
                close();
                suggestions = [];
            }
        } catch (e) {
            close();
            suggestions = [];
        } finally {
            loading = false;
        }
    }

    $: if (disabled || !focused) {
        close();
        if (debounceTimer) clearTimeout(debounceTimer);
        requestId += 1;
        suppressForText = "";
    } else if (text !== undefined && focused) {
        if (debounceTimer) clearTimeout(debounceTimer);
        const prefix = text;
        if (!prefix || prefix.trim().length === 0) {
            close();
            suggestions = [];
            lastFetchedPrefix = "";
            suppressForText = "";
        } else if (prefix === suppressForText) {
            close();
            suggestions = [];
        } else if (prefix !== lastFetchedPrefix || !open) {
            debounceTimer = setTimeout(() => {
                lastFetchedPrefix = prefix;
                const currentRequestId = ++requestId;
                fetchSuggestions(prefix, currentRequestId);
            }, DEBOUNCE_MS);
        }
    }

    /**
     * @param {string} suggestion
     */
    function accept(suggestion) {
        if (!suggestion) return;
        const acceptedText = onAccept(suggestion);
        if (typeof acceptedText === "string") {
            suppressForText = acceptedText;
            lastFetchedPrefix = acceptedText;
            requestId += 1;
        }
        close();
    }

    export function suppressUntilText(nextText) {
        suppressForText = nextText || "";
        lastFetchedPrefix = suppressForText;
        if (debounceTimer) clearTimeout(debounceTimer);
        requestId += 1;
        close();
        suggestions = [];
    }

    /** @param {KeyboardEvent} event */
    function handleKeydown(event) {
        if (!open || suggestions.length === 0) return;

        switch (event.key) {
            case "ArrowDown":
                event.preventDefault();
                activeIndex = (activeIndex + 1) % suggestions.length;
                break;
            case "ArrowUp":
                event.preventDefault();
                activeIndex =
                    activeIndex <= 0
                        ? suggestions.length - 1
                        : activeIndex - 1;
                break;
            case "Tab":
                event.preventDefault();
                if (activeIndex >= 0) {
                    accept(suggestions[activeIndex]);
                } else if (suggestions[0]) {
                    accept(suggestions[0]);
                }
                break;
            case "Enter":
                if (activeIndex >= 0) {
                    event.preventDefault();
                    accept(suggestions[activeIndex]);
                }
                break;
            case "Escape":
                event.preventDefault();
                close();
                break;
        }
    }

    /** @param {MouseEvent} event */
    function onDocumentClick(event) {
        const clickTarget = event.target instanceof Node ? event.target : null;
        if (open && panelRef && clickTarget && !panelRef.contains(clickTarget) && targetElement && !targetElement.contains(clickTarget)) {
            close();
        }
    }
</script>

<svelte:window on:keydown={handleKeydown} on:click={onDocumentClick} />

{#if open && suggestions.length > 0}
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
        bind:this={panelRef}
        class="suggestion-popup"
                on:mouseleave={() => activeIndex = -1}
                transition:fly={{ y: 4, duration: 120 }}
    >
        {#each suggestions as suggestion, i (suggestion)}
            <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
            <div
                class="suggestion-row"
                class:active={i === activeIndex}
                on:mousedown|preventDefault={() => accept(suggestion)}
                on:mouseenter={() => activeIndex = i}
            >
                <span class="suggestion-text">{suggestion}</span>
                {#if i === activeIndex}
                    <span class="suggestion-hint">Tab</span>
                {:else if i === 0 && activeIndex < 0}
                    <span class="suggestion-hint">Tab</span>
                {/if}
            </div>
        {/each}
    </div>
{/if}

<style>
    .suggestion-popup {
        position: absolute;
        left: 0;
        right: 0;
        top: 100%;
        margin-top: 4px;
        background: rgba(10, 15, 12, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        overflow: hidden;
        box-shadow:
            0 8px 32px rgba(0, 0, 0, 0.7),
            0 0 0 1px rgba(0, 255, 65, 0.1);
        z-index: 10;
        pointer-events: auto;
    }
    .suggestion-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.45rem 0.6rem;
        cursor: pointer;
        font-size: 0.7rem;
        color: #ccc;
        transition: background 0.12s, color 0.12s;
        border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }
    .suggestion-row:last-child {
        border-bottom: none;
    }
    .suggestion-row:hover,
    .suggestion-row.active {
        background: rgba(0, 255, 65, 0.12);
        color: #fff;
    }
    .suggestion-text {
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .suggestion-hint {
        font-size: 0.5rem;
        color: #555;
        margin-left: 0.5rem;
        letter-spacing: 0.5px;
        transition: color 0.12s;
    }
    .suggestion-row.active .suggestion-hint,
    .suggestion-row:hover .suggestion-hint {
        color: var(--color-primary);
    }
</style>
