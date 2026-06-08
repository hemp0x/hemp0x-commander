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
    export let tagBlockedChannels = new Set();
    /** @type {Record<string, string>} */
    export let resolvedAddresses = {};

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
        contextUser = rootName;
        contextX = e.clientX;
        contextY = e.clientY;
    }

    /** @param {MouseEvent} e */
    function openContextClick(e, rootName) {
        e.preventDefault();
        e.stopPropagation();
        contextUser = rootName;
        contextX = e.clientX;
        contextY = e.clientY;
    }

    function closeContext() {
        contextUser = null;
    }

    /**
     * @param {number} lastSeen
     * @returns {{ label: string, tier: number }}
     */
    function activityBucket(lastSeen) {
        const age = Date.now() - lastSeen;
        if (age < 0 || lastSeen === 0) return { label: "No activity", tier: 4 };
        if (age < 3600000) return { label: "Active recently", tier: 0 };
        if (age < 21600000) return { label: "Few hours ago", tier: 1 };
        if (age < 604800000) return { label: "This week", tier: 2 };
        if (age < 2592000000) return { label: "Older", tier: 3 };
        return { label: "Long ago", tier: 4 };
    }

    $: filteredParticipants = participants.filter((p) => !blockedUsers.includes(p.rootName) && !tagBlockedChannels.has(p.assetName));
    $: sortedParticipants = [...filteredParticipants].sort((a, b) => {
        const aBucket = activityBucket(a.lastSeen).tier;
        const bBucket = activityBucket(b.lastSeen).tier;
        if (aBucket !== bBucket) return aBucket - bBucket;
        return a.rootName.localeCompare(b.rootName);
    });
    $: activeCount = sortedParticipants.filter((p) => activityBucket(p.lastSeen).tier === 0).length;
</script>

<div class="h0xc-user-list" transition:fade={{ duration: 100 }}>
    <div class="user-list-header">
        <span class="ul-title">USERS</span>
        <span class="ul-count">{filteredParticipants.length}</span>
    </div>
    <div class="user-list-body">
        {#if filteredParticipants.length === 0}
            <div class="ul-empty">No participants discovered</div>
        {:else}
            {#each sortedParticipants as p}
                {@const bucket = activityBucket(p.lastSeen)}
                <button
                    class="ul-user"
                    class:me={isMe(p.rootName)}
                    class:muted={mutedUsers.includes(p.rootName)}
                    on:click={(e) => openContextClick(e, p.rootName)}
                    on:contextmenu={(e) => openContext(e, p.rootName)}
                    title={`${p.rootName.toUpperCase()} — ${p.messageCount} msgs — ${bucket.label}`}
                >
                    <span class="ul-dot dot-tier-{bucket.tier}"></span>
                    <span class="ul-name">[{p.rootName.toUpperCase()}]</span>
                    {#if mutedUsers.includes(p.rootName)}
                        <span class="ul-badge muted">M</span>
                    {:else if blockedUsers.includes(p.rootName)}
                        <span class="ul-badge blocked">B</span>
                    {/if}
                </button>
            {/each}
        {/if}
    </div>

    <H0xCUserContextMenu
        x={contextX}
        y={contextY}
        user={contextUser || ""}
        muted={contextUser ? mutedUsers.includes(contextUser) : false}
        blocked={contextUser ? blockedUsers.includes(contextUser) : false}
        resolvedAddress={contextUser ? resolvedAddresses[contextUser] || "" : ""}
        channelAsset={contextUser ? (participants.find((p) => p.rootName === contextUser)?.assetName || "") : ""}
        lastSeen={contextUser ? (participants.find((p) => p.rootName === contextUser)?.lastSeen || 0) : 0}
        messageCount={contextUser ? (participants.find((p) => p.rootName === contextUser)?.messageCount || 0) : 0}
        isSelf={contextUser ? isMe(contextUser) : false}
        on:viewDetails={(e) => { dispatch("viewDetails", e.detail); closeContext(); }}
        on:mute={(e) => { dispatch("mute", e.detail); closeContext(); }}
        on:block={(e) => { dispatch("block", e.detail); closeContext(); }}
        on:blockAndUnsub={(e) => { dispatch("blockAndUnsub", e.detail); closeContext(); }}
        on:manageTags={(e) => { dispatch("manageTags", e.detail); closeContext(); }}
        on:filterByUser={(e) => { dispatch("filterByUser", e.detail); closeContext(); }}
        on:copyChannel={() => { closeContext(); }}
        on:copyAddress={() => { closeContext(); }}
        on:addTag={(e) => { dispatch("addTag", e.detail); closeContext(); }}
        on:close={() => { closeContext(); }}
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
    .ul-user.muted .ul-name {
        color: #777;
        text-decoration: line-through;
        opacity: 0.7;
    }
    .ul-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        flex-shrink: 0;
        transition: all 0.3s;
    }
    .dot-tier-0 {
        background: var(--color-primary);
        box-shadow: 0 0 4px rgba(0, 255, 65, 0.5);
    }
    .dot-tier-1 {
        background: #88cc44;
        opacity: 0.7;
    }
    .dot-tier-2 {
        background: #ccaa00;
        opacity: 0.5;
    }
    .dot-tier-3 {
        background: #886633;
        opacity: 0.4;
    }
    .dot-tier-4 {
        background: #555;
        opacity: 0.3;
    }
    .ul-name {
        font-size: 0.52rem;
        color: #aaa;
        font-family: var(--font-mono);
        letter-spacing: 0.3px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        flex: 1;
        min-width: 0;
    }
    .ul-badge {
        font-size: 0.38rem;
        font-weight: 700;
        letter-spacing: 0.5px;
        padding: 0.05rem 0.18rem;
        border-radius: 3px;
        flex-shrink: 0;
        line-height: 1;
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
