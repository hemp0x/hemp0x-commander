<script>
    import { createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";

    export let x = 0;
    export let y = 0;
    export let user = "";
    export let muted = false;
    export let blocked = false;

    const dispatch = createEventDispatcher();

    function view() { dispatch("viewDetails", { rootName: user }); }
    function mute() { dispatch("mute", { rootName: user }); }
    function block() { dispatch("block", { rootName: user }); }
</script>

{#if user}
    <div class="h0xc-ctx-menu" style="left: {x}px; top: {y}px;" transition:fade={{ duration: 80 }} role="menu">
        <div class="ctx-header">[{user.toUpperCase()}]</div>
        <button class="ctx-item" on:click={view}>View Details</button>
        <button class="ctx-item" on:click={mute}>{muted ? "Unmute" : "Mute"}</button>
        <button class="ctx-item danger" on:click={block}>{blocked ? "Unblock" : "Block"}</button>
    </div>
{/if}

<style>
    .h0xc-ctx-menu {
        position: fixed;
        z-index: 1000;
        background: linear-gradient(180deg, #0a0e0b, #101510);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.7);
        padding: 0.2rem 0;
        min-width: 120px;
        font-family: var(--font-mono);
    }
    .ctx-header {
        font-size: 0.58rem;
        font-weight: 700;
        color: var(--color-primary);
        padding: 0.3rem 0.6rem 0.2rem;
        letter-spacing: 0.5px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        margin-bottom: 0.15rem;
    }
    .ctx-item {
        display: block;
        width: 100%;
        text-align: left;
        background: none;
        border: none;
        color: #aaa;
        font-size: 0.58rem;
        padding: 0.3rem 0.6rem;
        cursor: pointer;
        transition: background 0.1s;
        font-family: var(--font-mono);
    }
    .ctx-item:hover {
        background: rgba(0, 255, 65, 0.08);
        color: #fff;
    }
    .ctx-item.danger:hover {
        background: rgba(255, 85, 85, 0.1);
        color: #ff8888;
    }
</style>
