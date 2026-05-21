<script>
    import { tick, onDestroy } from "svelte";
    export let text = "";
    export let delay = 200;

    let visible = false;
    let timer;
    let container;
    let x = 0;
    let y = 0;
    let popupElement;

    // Portal action to move element to body
    function portal(node) {
        document.body.appendChild(node);
        return {
            destroy() {
                if (node.parentNode) {
                    node.parentNode.removeChild(node);
                }
            },
        };
    }

    function updatePosition() {
        if (container) {
            const rect = container.getBoundingClientRect();
            // Center horizontally, place above top
            x = rect.left + rect.width / 2;
            y = rect.top - 8; // 8px gap
        }
    }

    function handleMouseEnter() {
        timer = setTimeout(async () => {
            updatePosition();
            visible = true;
            await tick();
        }, delay);
        // Track Scroll/Resize to update position if needed (optional optimization)
        window.addEventListener("scroll", updatePosition, true);
        window.addEventListener("resize", updatePosition);
    }

    function handleMouseLeave() {
        clearTimeout(timer);
        visible = false;
        window.removeEventListener("scroll", updatePosition, true);
        window.removeEventListener("resize", updatePosition);
    }

    onDestroy(() => {
        if (typeof window !== "undefined") {
            window.removeEventListener("scroll", updatePosition, true);
            window.removeEventListener("resize", updatePosition);
        }
    });
</script>

<div
    class="tooltip-container"
    bind:this={container}
    on:mouseenter={handleMouseEnter}
    on:mouseleave={handleMouseLeave}
    role="tooltip"
>
    <slot />
    {#if visible && text}
        <div class="tooltip-popup" use:portal style="top: {y}px; left: {x}px;">
            {text}
            <div class="arrow"></div>
        </div>
    {/if}
</div>

<style>
    .tooltip-container {
        display: inline-flex;
    }

    .tooltip-popup {
        position: fixed;
        transform: translateX(-50%) translateY(-100%);
        background: rgba(10, 20, 15, 0.98);
        border: 1px solid var(--color-primary);
        color: var(--color-primary);
        padding: 5px 10px;
        font-size: 0.75rem;
        border-radius: 4px;
        white-space: nowrap;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.8);
        z-index: 99999;
        pointer-events: none;
        letter-spacing: 0.5px;
        font-weight: 600;
    }

    .arrow {
        position: absolute;
        top: 100%;
        left: 50%;
        margin-left: -5px;
        border-width: 5px;
        border-style: solid;
        border-color: var(--color-primary) transparent transparent transparent;
    }

    @keyframes tooltipFadeIn {
        from {
            opacity: 0;
            transform: translateX(-50%) translateY(4px);
        }
        to {
            opacity: 1;
            transform: translateX(-50%) translateY(0);
        }
    }
</style>
