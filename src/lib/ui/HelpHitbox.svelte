<script>
    import { fly, fade } from "svelte/transition";

    export let title = "";
    let open = false;
    let anchorEl;

    function toggle(e) {
        e.stopPropagation();
        open = !open;
    }

    function close() {
        open = false;
    }

    function handleKeydown(e) {
        if (e.key === "Escape") close();
    }
</script>

<svelte:window on:click={open ? close : undefined} on:keydown={handleKeydown} />

<div class="help-hitbox" bind:this={anchorEl}>
    <button class="help-btn" on:click={toggle} type="button" aria-label={title || "Help"}>
        ?
    </button>

    {#if open}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
            class="help-popover"
            transition:fly={{ y: 4, duration: 150 }}
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
        position: absolute;
        z-index: 100;
        top: calc(100% + 6px);
        left: 50%;
        transform: translateX(-50%);
        width: 280px;
        max-width: 90vw;
        background: rgba(8, 14, 10, 0.98);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 8px;
        padding: 0.75rem 1rem;
        box-shadow:
            0 0 40px rgba(0, 0, 0, 0.7),
            0 0 20px rgba(0, 255, 65, 0.08);
        backdrop-filter: blur(6px);
    }

    .help-popover::before {
        content: "";
        position: absolute;
        top: -5px;
        left: 50%;
        transform: translateX(-50%);
        border-left: 5px solid transparent;
        border-right: 5px solid transparent;
        border-bottom: 5px solid rgba(0, 255, 65, 0.25);
    }

    .help-title {
        font-size: 0.65rem;
        font-weight: 700;
        color: var(--color-primary);
        letter-spacing: 1px;
        margin-bottom: 0.4rem;
        text-transform: uppercase;
    }

    .help-body {
        font-size: 0.6rem;
        line-height: 1.5;
        color: #aaa;
    }

    .help-body :global(p) {
        margin: 0 0 0.4rem 0;
    }

    .help-body :global(p:last-child) {
        margin-bottom: 0;
    }

    .help-body :global(ul) {
        margin: 0.25rem 0;
        padding-left: 1rem;
    }

    .help-body :global(li) {
        margin-bottom: 0.15rem;
    }
</style>
