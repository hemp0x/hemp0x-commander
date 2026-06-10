<script>
    import { onMount, onDestroy, createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";

    export let x = 0;
    export let y = 0;
    export let user = "";
    export let muted = false;
    export let blocked = false;
    export let resolvedAddress = "";
    export let channelAsset = "";
    export let lastSeen = 0;
    export let joinedAt = 0;
    export let messageCount = 0;
    export let isSelf = false;
    export let isLeft = false;

    const dispatch = createEventDispatcher();

    let addTagMode = false;
    let addTagText = "";
    let menuEl = null;
    let winClickHandler = null;
    let menuLeft = 0;
    let menuTop = 0;

    function mute() { dispatch("mute", { rootName: user }); }
    function block() { dispatch("block", { rootName: user }); }
    function blockAndUnsub() { dispatch("blockAndUnsub", { rootName: user }); }
    function manageTags() { dispatch("manageTags", { rootName: user }); }
    function filterByUser() { dispatch("filterByUser", { rootName: user }); }
    function copyChannel() { dispatch("copyChannel", { assetName: channelAsset }); }
    function copyAddress() { dispatch("copyAddress", { address: resolvedAddress }); }

    function submitAddTag() {
        const tag = addTagText.trim();
        if (!tag) return;
        dispatch("addTag", { rootName: user, address: resolvedAddress, tag });
        addTagMode = false;
        addTagText = "";
    }

    function copyText(text) {
        try { navigator.clipboard.writeText(text); } catch {}
    }

    function formatActivity(ts) {
        if (!ts || ts === 0) return "No activity";
        const age = Date.now() - ts;
        if (age < 3600000) return "Active recently";
        if (age < 21600000) return "Few hours ago";
        if (age < 604800000) return "This week";
        if (age < 2592000000) return "Older";
        return "Long ago";
    }

    function handleWindowClick(e) {
        if (!user || !menuEl) return;
        if (menuEl.contains(e.target)) return;
        dispatch("close");
    }

    function handleEscape(e) {
        if (e.key === "Escape" && user) dispatch("close");
    }

    function portal(node) {
        menuEl = node;
        if (typeof document !== "undefined" && node.parentNode !== document.body) {
            document.body.appendChild(node);
        }
        return {
            destroy() {
                if (node.parentNode === document.body) {
                    document.body.removeChild(node);
                }
                if (menuEl === node) {
                    menuEl = null;
                }
            },
        };
    }

    function clampMenu() {
        if (!menuEl || typeof window === "undefined") return;
        const pad = 8;
        const rect = menuEl.getBoundingClientRect();
        menuLeft = Math.max(pad, Math.min(x, window.innerWidth - rect.width - pad));
        menuTop = Math.max(pad, Math.min(y, window.innerHeight - rect.height - pad));
    }

    $: if (user) {
        menuLeft = x;
        menuTop = y;
        requestAnimationFrame(clampMenu);
    }

    onMount(() => {
        setTimeout(() => {
            winClickHandler = handleWindowClick;
            window.addEventListener("click", winClickHandler);
            window.addEventListener("keydown", handleEscape);
        }, 0);
    });

    onDestroy(() => {
        if (winClickHandler) window.removeEventListener("click", winClickHandler);
        window.removeEventListener("keydown", handleEscape);
    });
</script>

{#if user}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_interactive_supports_focus -->
    <div
        class="h0xc-ctx-menu"
        use:portal
        style="left: {menuLeft}px; top: {menuTop}px;"
        transition:fade={{ duration: 80 }}
        role="menu"
        on:click|stopPropagation
        on:keydown|stopPropagation
    >
        <div class="ctx-header">[{user.toUpperCase()}]</div>
        <div class="ctx-info-grid">
            <div class="ctx-info-row">
                <span class="ctx-info-label">Activity</span>
                <span class="ctx-info-val">{formatActivity(lastSeen)}</span>
            </div>
            <div class="ctx-info-row">
                <span class="ctx-info-label">Messages</span>
                <span class="ctx-info-val">{messageCount}</span>
            </div>
            <div class="ctx-info-row">
                <span class="ctx-info-label">Joined</span>
                <span class="ctx-info-val">{joinedAt ? new Date(joinedAt).toLocaleDateString() : "Unknown"}</span>
            </div>
            {#if channelAsset}
                <div class="ctx-info-row">
                    <span class="ctx-info-label">Channel</span>
                    <span class="ctx-info-val mono">{channelAsset}</span>
                </div>
            {/if}
            {#if resolvedAddress}
                <div class="ctx-info-row">
                    <span class="ctx-info-label">Owner</span>
                    <span class="ctx-info-val mono" title={resolvedAddress}>{resolvedAddress.slice(0, 8)}...{resolvedAddress.slice(-6)}</span>
                </div>
            {:else}
                <div class="ctx-info-row">
                    <span class="ctx-info-label">Owner</span>
                    <span class="ctx-info-val dim">Not resolved</span>
                </div>
            {/if}
        </div>
        <div class="ctx-divider"></div>
        <button class="ctx-item" on:click={filterByUser}>Filter Messages</button>
        {#if isSelf}
            <button class="ctx-item danger" on:click={() => dispatch("leave")} disabled={isLeft}>
                {isLeft ? "Already left" : "Leave chat"}
            </button>
        {:else}
            <button class="ctx-item" on:click={mute}>{muted ? "Unmute" : "Mute"}</button>
            <button class="ctx-item danger" on:click={block}>{blocked ? "Unblock" : "Block Locally"}</button>
            <button class="ctx-item danger" on:click={blockAndUnsub}>Block &amp; Unsubscribe</button>
        {/if}
        <div class="ctx-divider"></div>
        {#if channelAsset}
            <button class="ctx-item copy" on:click={() => { copyText(channelAsset); copyChannel(); }}>Copy Channel Asset</button>
        {/if}
        {#if resolvedAddress}
            <button class="ctx-item copy" on:click={() => { copyText(resolvedAddress); copyAddress(); }}>Copy Owner Address</button>
        {/if}
        <div class="ctx-divider"></div>
        {#if !isSelf}
            {#if addTagMode}
                <div class="ctx-tag-input-row">
                    <input
                        class="ctx-tag-input"
                        type="text"
                        bind:value={addTagText}
                        placeholder="#SPAM"
                        on:keydown={(e) => e.key === "Enter" && submitAddTag()}
                    />
                    <button class="ctx-tag-submit" on:click={submitAddTag} disabled={!addTagText.trim()}>Add</button>
                </div>
            {:else}
                <button class="ctx-item tag" on:click={() => { addTagMode = true; addTagText = ""; }}>Add Tag for Address</button>
            {/if}
        {/if}
        {#if resolvedAddress}
            <button class="ctx-item tag" on:click={manageTags}>Manage Tags</button>
        {/if}
    </div>
{/if}

<style>
    .h0xc-ctx-menu {
        position: fixed;
        z-index: 99999;
        background: linear-gradient(180deg, #0a0e0b, #101510);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 6px;
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.7);
        padding: 0.2rem 0;
        min-width: 170px;
        max-width: 240px;
        font-family: var(--font-mono);
    }
    .ctx-header {
        font-size: 0.58rem;
        font-weight: 700;
        color: var(--color-primary);
        padding: 0.3rem 0.6rem 0.2rem;
        letter-spacing: 0.5px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        margin-bottom: 0;
    }
    .ctx-info-grid {
        padding: 0.2rem 0;
    }
    .ctx-info-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 0.4rem;
        padding: 0.15rem 0.6rem;
        font-size: 0.46rem;
    }
    .ctx-info-label {
        color: #555;
        font-weight: 600;
        letter-spacing: 0.3px;
        flex-shrink: 0;
    }
    .ctx-info-val {
        color: #999;
        text-align: right;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        min-width: 0;
    }
    .ctx-info-val.mono {
        font-family: var(--font-mono);
        font-size: 0.42rem;
        color: #777;
    }
    .ctx-info-val.dim {
        color: #555;
        font-style: italic;
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
    .ctx-item.tag {
        color: var(--color-primary);
    }
    .ctx-item.tag:hover {
        background: rgba(0, 255, 65, 0.08);
        color: #fff;
    }
    .ctx-item.copy {
        color: #888;
    }
    .ctx-item.copy:hover {
        background: rgba(68, 136, 255, 0.08);
        color: #aaccff;
    }
    .ctx-divider {
        height: 1px;
        background: rgba(255, 255, 255, 0.06);
        margin: 0.15rem 0;
    }
    .ctx-tag-input-row {
        display: flex;
        gap: 0.25rem;
        padding: 0.25rem 0.6rem;
        align-items: center;
    }
    .ctx-tag-input {
        flex: 1;
        min-width: 0;
        padding: 0.2rem 0.35rem;
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 4px;
        color: #ddd;
        font-size: 0.52rem;
        font-family: var(--font-mono);
        outline: none;
    }
    .ctx-tag-input:focus {
        border-color: var(--color-primary);
    }
    .ctx-tag-input::placeholder { color: #555; }
    .ctx-tag-submit {
        padding: 0.18rem 0.35rem;
        background: rgba(0, 255, 65, 0.08);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.48rem;
        font-weight: 600;
        cursor: pointer;
        font-family: var(--font-mono);
        transition: all 0.15s;
        flex-shrink: 0;
    }
    .ctx-tag-submit:hover:not(:disabled) {
        background: rgba(0, 255, 65, 0.15);
    }
    .ctx-tag-submit:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
</style>
