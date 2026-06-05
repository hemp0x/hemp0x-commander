<script>
    import { fly, fade } from "svelte/transition";

    export let title = "";
    let open = false;

    function toggle(e) {
        e.stopPropagation();
        open = !open;
    }

    function close() {
        open = false;
    }

    /** @param {KeyboardEvent} e */
    function handleKeydown(e) {
        if (e.key === "Escape") close();
    }
</script>

<svelte:window on:click={open ? close : undefined} on:keydown={handleKeydown} />

<div class="help-hitbox">
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
        top: 100%;
        left: 0;
        margin-top: 6px;
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
        z-index: 40;
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
