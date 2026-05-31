<script>
    import { fly, fade } from "svelte/transition";

    export let title = "";
    let open = false;
    let anchorEl;
    let popoverStyle = "";
    let arrowClass = "arrow-left";

    function updatePosition() {
        if (!anchorEl || !open) return;
        const rect = anchorEl.getBoundingClientRect();
        const popoverWidth = 280;
        const popoverHeight = 200; // approximate max, actual will vary
        const padding = 12;
        const gap = 8;

        let left, top;

        // Try positioning to the right of the button first
        left = rect.right + gap;
        top = rect.top + rect.height / 2 - 40; // roughly center vertically

        // Clamp top so it doesn't go above viewport
        if (top < padding) top = padding;

        // If it would extend past the right edge, flip to the left side of the button
        if (left + popoverWidth > window.innerWidth - padding) {
            left = rect.left - popoverWidth - gap;
            arrowClass = "arrow-right";
        } else {
            arrowClass = "arrow-left";
        }

        // Clamp left so it doesn't go past the left edge
        if (left < padding) {
            left = padding;
            arrowClass = "arrow-left";
        }

        // Also clamp bottom if the approximate height exceeds viewport
        const estimatedHeight = 180; // conservative estimate
        if (top + estimatedHeight > window.innerHeight - padding) {
            top = window.innerHeight - estimatedHeight - padding;
            if (top < padding) top = padding;
        }

        popoverStyle = `top: ${top}px; left: ${left}px;`;
    }

    function toggle(e) {
        e.stopPropagation();
        open = !open;
        if (open) {
            requestAnimationFrame(updatePosition);
        }
    }

    function close() {
        open = false;
    }

    function handleKeydown(e) {
        if (e.key === "Escape") close();
    }
</script>

<svelte:window on:click={open ? close : undefined} on:keydown={handleKeydown} on:resize={open ? updatePosition : undefined} />

<div class="help-hitbox" bind:this={anchorEl}>
    <button class="help-btn" on:click={toggle} type="button" aria-label={title || "Help"}>
        ?
    </button>

    {#if open}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
            class="help-popover {arrowClass}"
            style="{popoverStyle}"
            transition:fly={{ x: 8, duration: 150 }}
            on:click|stopPropagation
        >
            {#if title}
                <div class="help-title">{title}</div>
            {/if}
            <div class="help-body">
                <slot />
            </div>
        </div>
    {/if}
</div>

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
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.4);
    }

    .help-popover {
        position: fixed;
        z-index: 10000;
        width: 280px;
        max-width: 90vw;
        background: rgba(8, 14, 10, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        padding: 0.7rem 0.95rem;
        box-shadow:
            0 0 40px rgba(0, 0, 0, 0.7),
            0 0 20px rgba(0, 255, 65, 0.08);
        backdrop-filter: blur(6px);
    }

    /* Arrow pointing left (popover is to the right of button) */
    .help-popover.arrow-left::before {
        content: "";
        position: absolute;
        top: 24px;
        left: -5px;
        transform: translateY(-50%);
        border-top: 5px solid transparent;
        border-bottom: 5px solid transparent;
        border-right: 5px solid rgba(0, 255, 65, 0.25);
    }

    /* Arrow pointing right (popover is to the left of button) */
    .help-popover.arrow-right::before {
        content: "";
        position: absolute;
        top: 24px;
        right: -5px;
        transform: translateY(-50%);
        border-top: 5px solid transparent;
        border-bottom: 5px solid transparent;
        border-left: 5px solid rgba(0, 255, 65, 0.25);
    }

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
