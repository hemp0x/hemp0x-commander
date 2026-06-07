<script>
    import { createEventDispatcher } from "svelte";
    import { fade } from "svelte/transition";
    import { deriveRootNameFn } from "../../stores/h0xc.js";
    import H0xCUserContextMenu from "./H0xCUserContextMenu.svelte";

    /**
     * @typedef {{ rootName: string, assetName: string, lastSeen: number, messageCount: number }} Participant
     */

    /** @type {Participant[]} */
    export let participants = [];
    /** @type {string[]} */
    export let mutedUsers = [];
    /** @type {string[]} */
    export let blockedUsers = [];
    export let selectedIdentity = "";

    const dispatch = createEventDispatcher();

    /** @type {string | null} */
    let contextUser = null;
    let contextX = 0;
    let contextY = 0;

    /** @param {string} rootName */
    function isMe(rootName) {
        const myRoot = deriveRootNameFn(selectedIdentity);
        return rootName.toUpperCase() === myRoot.toUpperCase();
    }

    /** @param {MouseEvent} e */
    function openContext(e, rootName) {
        e.preventDefault();
        e.stopPropagation();
        if (isMe(rootName)) return;
        contextUser = rootName;
        contextX = e.clientX;
        contextY = e.clientY;
    }

    function closeContext() {
        contextUser = null;
    }

    $: filteredParticipants = participants.filter((p) => !blockedUsers.includes(p.rootName));
    $: online = filteredParticipants.filter((p) => {
        const hourMs = 3600000;
        return (Date.now() - p.lastSeen) < hourMs;
    });
    $: offline = filteredParticipants.filter((p) => {
        const hourMs = 3600000;
        return (Date.now() - p.lastSeen) >= hourMs;
    });
    $: sortedOnline = [...online].sort((a, b) => a.rootName.localeCompare(b.rootName));
    $: sortedOffline = [...offline].sort((a, b) => a.rootName.localeCompare(b.rootName));

    function handleWindowClick() {
        if (contextUser) closeContext();
    }
</script>

<svelte:window on:click={handleWindowClick} />

<div class="h0xc-user-list" transition:fade={{ duration: 100 }}>
    <div class="user-list-header">
        <span class="ul-title">USERS</span>
        <span class="ul-count">{filteredParticipants.length}</span>
    </div>
    <div class="user-list-body">
        {#if filteredParticipants.length === 0}
            <div class="ul-empty">No participants discovered</div>
        {:else}
            {#if sortedOnline.length > 0}
                <div class="ul-section-label">ONLINE</div>
                {#each sortedOnline as p}
                    <button
                        class="ul-user"
                        class:me={isMe(p.rootName)}
                        class:muted={mutedUsers.includes(p.rootName)}
                        on:contextmenu={(e) => openContext(e, p.rootName)}
                        title={`${p.assetName} - ${p.messageCount} msgs`}
                    >
                        <span class="ul-dot"></span>
                        <span class="ul-name">[{p.rootName.toUpperCase()}]</span>
                        {#if mutedUsers.includes(p.rootName)}
                            <span class="ul-badge muted">MUTED</span>
                        {:else if blockedUsers.includes(p.rootName)}
                            <span class="ul-badge blocked">BLOCKED</span>
                        {/if}
                    </button>
                {/each}
            {/if}
            {#if sortedOffline.length > 0}
                <div class="ul-section-label">OFFLINE</div>
                {#each sortedOffline as p}
                    <button
                        class="ul-user offline"
                        class:me={isMe(p.rootName)}
                        class:muted={mutedUsers.includes(p.rootName)}
                        on:contextmenu={(e) => openContext(e, p.rootName)}
                        title={`${p.assetName} - ${p.messageCount} msgs`}
                    >
                        <span class="ul-dot offline"></span>
                        <span class="ul-name">[{p.rootName.toUpperCase()}]</span>
                        {#if mutedUsers.includes(p.rootName)}
                            <span class="ul-badge muted">MUTED</span>
                        {:else if blockedUsers.includes(p.rootName)}
                            <span class="ul-badge blocked">BLOCKED</span>
                        {/if}
                    </button>
                {/each}
            {/if}
        {/if}
    </div>

    <H0xCUserContextMenu
        x={contextX}
        y={contextY}
        user={contextUser || ""}
        muted={contextUser ? mutedUsers.includes(contextUser) : false}
        blocked={contextUser ? blockedUsers.includes(contextUser) : false}
        on:viewDetails={(e) => { dispatch("viewDetails", e.detail); closeContext(); }}
        on:mute={(e) => { dispatch("mute", e.detail); closeContext(); }}
        on:block={(e) => { dispatch("block", e.detail); closeContext(); }}
    />
</div>

<style>
    .h0xc-user-list {
        width: 130px;
        min-width: 110px;
        border-left: 1px solid rgba(255, 255, 255, 0.06);
        display: flex;
        flex-direction: column;
        background: rgba(0, 0, 0, 0.2);
        position: relative;
    }
    .user-list-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.35rem 0.5rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        background: rgba(0, 0, 0, 0.2);
    }
    .ul-title {
        font-size: 0.5rem;
        font-weight: 700;
        color: #666;
        letter-spacing: 1px;
    }
    .ul-count {
        font-size: 0.45rem;
        color: #555;
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.05);
        border-radius: 999px;
        padding: 0.1rem 0.3rem;
    }
    .user-list-body {
        flex: 1;
        overflow-y: auto;
        padding: 0.3rem 0;
        scrollbar-width: thin;
        scrollbar-color: rgba(0, 255, 65, 0.2) transparent;
    }
    .ul-empty {
        font-size: 0.5rem;
        color: #555;
        text-align: center;
        padding: 1rem 0.5rem;
    }
    .ul-section-label {
        font-size: 0.45rem;
        font-weight: 600;
        color: #555;
        letter-spacing: 0.5px;
        padding: 0.2rem 0.5rem;
        margin-top: 0.1rem;
    }
    .ul-user {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        padding: 0.2rem 0.5rem;
        cursor: pointer;
        background: none;
        border: none;
        width: 100%;
        text-align: left;
        transition: background 0.1s;
    }
    .ul-user:hover {
        background: rgba(0, 255, 65, 0.04);
    }
    .ul-user.me .ul-name {
        color: var(--color-primary);
        font-weight: 700;
    }
    .ul-user.muted .ul-name {
        color: #777;
        text-decoration: line-through;
        opacity: 0.7;
    }
    .ul-dot {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: var(--color-primary);
        flex-shrink: 0;
    }
    .ul-dot.offline {
        opacity: 0.25;
    }
    .ul-name {
        font-size: 0.52rem;
        color: #aaa;
        font-family: var(--font-mono);
        letter-spacing: 0.3px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .ul-badge {
        font-size: 0.36rem;
        font-weight: 700;
        letter-spacing: 0.5px;
        padding: 0.05rem 0.2rem;
        border-radius: 3px;
    }
    .ul-badge.muted {
        color: #ffaa00;
        background: rgba(255, 170, 0, 0.1);
        border: 1px solid rgba(255, 170, 0, 0.2);
    }
    .ul-badge.blocked {
        color: #ff5555;
        background: rgba(255, 85, 85, 0.1);
        border: 1px solid rgba(255, 85, 85, 0.2);
    }
</style>
