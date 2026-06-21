<script>
    import { fly, fade } from "svelte/transition";

    export let title = "";
    export let right = false;
    let open = false;
    let btnEl = null;
    let popLeft = 0;
    let popTop = 0;

    function toggle(e) {
        e.stopPropagation();
        open = !open;
        if (open) {
            positionPopover();
        }
    }

    function close() {
        open = false;
    }

    /** @param {KeyboardEvent} e */
    function handleKeydown(e) {
        if (e.key === "Escape") close();
    }

    function positionPopover() {
        if (!btnEl || typeof window === "undefined") return;
        const rect = btnEl.getBoundingClientRect();
        const pad = 8;
        const width = 280;
        const maxHeight = Math.min(420, window.innerHeight * 0.7);
        const gap = 48;
        let left = rect.left;
        let top = rect.bottom + gap;
        if (right) {
            left = rect.right - width - 24;
        }
        if (left + width > window.innerWidth - pad) {
            left = window.innerWidth - width - pad;
        }
        if (left < pad) {
            left = pad;
        }
        if (top + maxHeight > window.innerHeight - pad) {
            top = Math.max(pad, rect.top - maxHeight - gap);
        }
        if (top < 120) {
            top = 120;
        }
        popLeft = left;
        popTop = top;
    }

    function portal(node) {
        if (typeof document !== "undefined" && node.parentNode !== document.body) {
            document.body.appendChild(node);
        }
        return {
            destroy() {
                if (node.parentNode === document.body) {
                    document.body.removeChild(node);
                }
            },
        };
    }
</script>

<svelte:window on:click={open ? close : undefined} on:keydown={handleKeydown} />

<div class="help-hitbox">
    <button class="help-btn" bind:this={btnEl} on:click={toggle} type="button" aria-label={title || "Help"}>
        ?
    </button>
</div>

{#if open}
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
        class="help-popover"
        class:right={right}
        use:portal
        transition:fly={{ y: 4, duration: 150 }}
        on:click|stopPropagation
        style="left: {popLeft}px; top: {popTop}px;"
    >
        {#if title}
            <div class="help-title">{title}</div>
        {/if}
        <div class="help-body">
            <slot />
        </div>
    </div>
{/if}

<style>
    .help-hitbox {
        position: relative;
        display: inline-flex;
        align-items: center;
        justify-content: center;
    }

    .help-btn {
        width: 18px;
        height: 18px;
        border-radius: 50%;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.3);
        color: var(--color-primary);
        font-size: 0.6rem;
        font-weight: 700;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        line-height: 1;
        transition: all 0.15s;
        flex-shrink: 0;
    }

    .help-btn:hover {
        background: var(--color-primary);
        color: #000;
        box-shadow: 0 0 8px rgba(0, 255, 65, 0.3);
    }

    .help-popover {
        position: fixed;
        width: 280px;
        max-width: 90vw;
        max-height: 70vh;
        overflow-y: auto;
        background: rgba(2, 4, 3, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        padding: 0.7rem 0.95rem;
        box-shadow: 0 12px 32px rgba(0, 0, 0, 0.75);
        z-index: 1000;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.35) transparent;
    }
    .help-popover::-webkit-scrollbar { width: 5px; }
    .help-popover::-webkit-scrollbar-track { background: transparent; }
    .help-popover::-webkit-scrollbar-thumb { background: rgba(0, 255, 65, 0.35); border-radius: 3px; }

    .help-title {
        font-size: 0.7rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
        margin-bottom: 0.4rem;
        text-transform: uppercase;
    }

    .help-body {
        font-size: 0.65rem;
        line-height: 1.55;
        color: #aaa;
    }

    .help-body :global(p) {
        margin: 0 0 0.4rem 0;
    }

    .help-body :global(p:last-child) {
        margin-bottom: 0;
    }

    .help-body :global(ul) {
        margin: 0.3rem 0;
        padding-left: 1rem;
    }

    .help-body :global(li) {
        margin-bottom: 0.2rem;
    }

    .help-body :global(code) {
        font-family: var(--font-mono);
        background: rgba(0, 255, 65, 0.08);
        padding: 0.05rem 0.25rem;
        border-radius: 3px;
        color: #ccc;
    }
</style>
