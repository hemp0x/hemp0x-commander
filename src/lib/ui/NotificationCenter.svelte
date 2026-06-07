<script>
    import { fade, fly } from "svelte/transition";
    import { onDestroy } from "svelte";
    import { notifications, unreadCount } from "../stores/notifications.js";

    let panelOpen = false;
    let clearConfirm = false;
    let actionMessage = "";
    /** @type {ReturnType<typeof setTimeout> | undefined} */
    let actionMessageTimer;
    /** @type {Array<Record<string, any>>} */
    let notificationItems = [];

    $: {
        /** @type {Array<Record<string, any>>} */
        const nextNotifications = [];
        for (const item of $notifications || []) {
            if (item) nextNotifications.push(item);
        }
        notificationItems = nextNotifications;
    }

    function togglePanel() {
        panelOpen = !panelOpen;
        clearConfirm = false;
    }

    function closePanel() {
        panelOpen = false;
        clearConfirm = false;
        actionMessage = "";
    }

    /**
     * @param {string} id
     */
    function handleMarkRead(id) {
        notifications.markRead(id);
    }

    function handleMarkAllRead() {
        notifications.markAllRead();
    }

    /**
     * @param {string} id
     */
    function handleClear(id) {
        notifications.clear(id);
    }

    function handleClearAll() {
        if (clearConfirm) {
            notifications.clearAll();
            clearConfirm = false;
            panelOpen = false;
        } else {
            clearConfirm = true;
        }
    }

    function cancelClear() {
        clearConfirm = false;
    }

    /**
     * @param {string} sev
     */
    function severityClass(sev) {
        return sev === "error"
            ? "severity-error"
            : sev === "warning"
                ? "severity-warning"
                : sev === "success"
                    ? "severity-success"
                    : "severity-info";
    }

    /**
     * @param {string} type
     */
    function categoryLabel(type) {
        const map = {
            transaction: "TX",
            asset: "AST",
            message: "MSG",
            ipfs: "IPFS",
            runtime: "RUN",
            tool: "TOOL",
            system: "SYS",
        };
        return map[/** @type {keyof typeof map} */ (type)] || type.substring(0, 3).toUpperCase();
    }

    /**
     * @param {string|number|Date} ts
     */
    function shortTimestamp(ts) {
        const d = new Date(ts);
        /** @param {number} n */
        const pad = (n) => String(n).padStart(2, "0");
        return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
    }

    /** @param {KeyboardEvent} e */
    function handleKeydown(e) {
        if (e.key === "Escape") closePanel();
    }

    /**
     * @param {string} id
     * @param {{ label: string, txid?: string } | undefined} action
     */
    async function handleActionClick(id, action) {
        notifications.markRead(id);
        if (action?.txid) {
            try {
                await navigator.clipboard.writeText(action.txid);
                showActionMessage("TXID copied");
            } catch {
                showActionMessage("Copy failed");
            }
        }
    }

    /**
     * @param {string} msg
     */
    function showActionMessage(msg) {
        clearTimeout(actionMessageTimer);
        actionMessage = msg;
        actionMessageTimer = setTimeout(() => {
            actionMessage = "";
        }, 1800);
    }

    onDestroy(() => {
        clearTimeout(actionMessageTimer);
    });
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="nc-root">
    <button
        class="nc-trigger"
        class:active={panelOpen}
        class:has-unread={$unreadCount > 0}
        on:click={togglePanel}
        type="button"
        title="Notification Center"
        aria-label="Open Notification Center"
    >
        <span class="nc-icon">&#x25C9;</span>
        {#if $unreadCount > 0}
            <span class="nc-badge">{$unreadCount}</span>
        {/if}
    </button>

    {#if panelOpen}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
            class="nc-backdrop"
            on:click={closePanel}
            role="button"
            tabindex="0"
        >
            <div
                class="nc-panel"
                in:fly={{ y: -8, duration: 150, opacity: 0 }}
                out:fade={{ duration: 100 }}
                on:click|stopPropagation
            >
                <div class="nc-header">
                    <span class="nc-title mono">[ NOTIFICATIONS ]</span>
                    <div class="nc-header-actions">
                        {#if $notifications.length > 0}
                            <button
                                class="nc-btn-ghost"
                                on:click={handleMarkAllRead}
                                disabled={$unreadCount === 0}
                                type="button"
                                title="Mark all read"
                            >
                                ALL READ
                            </button>
                            {#if clearConfirm}
                                <span class="nc-clear-confirm">
                                    <button
                                        class="nc-btn-ghost danger"
                                        on:click={handleClearAll}
                                        type="button"
                                    >
                                        CONFIRM
                                    </button>
                                    <button
                                        class="nc-btn-ghost"
                                        on:click={cancelClear}
                                        type="button"
                                    >
                                        NO
                                    </button>
                                </span>
                            {:else}
                                <button
                                    class="nc-btn-ghost"
                                    on:click={handleClearAll}
                                    type="button"
                                    title="Clear all notifications"
                                >
                                    CLEAR
                                </button>
                            {/if}
                        {/if}
                    </div>
                </div>

                <div class="nc-body">
                    {#if notificationItems.length === 0}
                        <div class="nc-empty">
                            <span class="nc-empty-icon">&#x2713;</span>
                            <span class="nc-empty-text"
                                >No notifications</span
                            >
                        </div>
                    {:else}
                        {#each notificationItems as notif (notif.id)}
                            <div
                                class="nc-item"
                                class:unread={!notif.read}
                                class:has-action={!!notif.action}
                            >
                                <div class="nc-item-bar">
                                    <span
                                        class="nc-severity-dot {severityClass(notif.severity)}"
                                    ></span>
                                    <span class="nc-category mono"
                                        >{categoryLabel(notif.type)}</span
                                    >
                                    <span class="nc-time mono"
                                        >{shortTimestamp(notif.timestamp)}</span
                                    >
                                    <div class="nc-item-actions">
                                        {#if !notif.read}
                                            <button
                                                class="nc-btn-sm"
                                                on:click={() =>
                                                    handleMarkRead(notif.id)}
                                                type="button"
                                                title="Mark read"
                                            >
                                                &#x2713;
                                            </button>
                                        {/if}
                                        <button
                                            class="nc-btn-sm"
                                            on:click={() =>
                                                handleClear(notif.id)}
                                            type="button"
                                            title="Dismiss"
                                        >
                                            &#x2715;
                                        </button>
                                    </div>
                                </div>
                                <div class="nc-item-body">
                                    {#if notif.title}
                                        <div class="nc-item-title">
                                            {notif.title}
                                        </div>
                                    {/if}
                                    <div class="nc-item-text">
                                        {notif.body}
                                    </div>
                                    {#if notif.action}
                                        <button
                                            class="nc-action-btn"
                                            on:click={() =>
                                                handleActionClick(
                                                    notif.id,
                                                    notif.action,
                                                )}
                                            type="button"
                                        >
                                            {notif.action.label}
                                        </button>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
                {#if actionMessage}
                    <div class="nc-action-message mono">
                        {actionMessage}
                    </div>
                {/if}
            </div>
        </div>
    {/if}
</div>

<style>
    .nc-root {
        position: relative;
    }

    .nc-trigger {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 6px;
        color: #666;
        cursor: pointer;
        font-size: 0.75rem;
        width: 34px;
        height: 34px;
        display: flex;
        align-items: center;
        justify-content: center;
        position: relative;
        transition: all 0.2s;
    }

    .nc-trigger:hover,
    .nc-trigger.active {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.3);
        box-shadow: 0 0 10px rgba(0, 255, 65, 0.15);
    }

    .nc-trigger.has-unread {
        border-color: rgba(0, 255, 65, 0.4);
        box-shadow: 0 0 8px rgba(0, 255, 65, 0.2);
    }

    .nc-icon {
        font-size: 1rem;
        line-height: 1;
    }

    .nc-badge {
        position: absolute;
        top: -4px;
        right: -4px;
        background: #ff4444;
        color: #fff;
        font-size: 0.55rem;
        font-weight: 700;
        font-family: var(--font-mono);
        min-width: 16px;
        height: 16px;
        border-radius: 8px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0 4px;
        box-shadow: 0 0 6px rgba(255, 68, 68, 0.5);
    }

    .nc-backdrop {
        position: fixed;
        inset: 0;
        z-index: 10000;
        background: transparent;
    }

    .nc-panel {
        position: fixed;
        top: 58px;
        right: 16px;
        width: 380px;
        max-width: calc(100vw - 32px);
        max-height: 480px;
        background: rgba(8, 12, 10, 0.97);
        border: 1px solid rgba(0, 255, 65, 0.2);
        border-radius: 8px;
        box-shadow:
            0 20px 50px rgba(0, 0, 0, 0.8),
            0 0 20px rgba(0, 255, 65, 0.08);
        backdrop-filter: blur(16px);
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }

    .nc-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.65rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        flex-shrink: 0;
    }

    .nc-title {
        font-size: 0.7rem;
        color: var(--color-muted);
        letter-spacing: 1px;
    }

    .nc-header-actions {
        display: flex;
        gap: 0.4rem;
        align-items: center;
    }

    .nc-btn-ghost {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        color: #888;
        font-size: 0.55rem;
        font-family: var(--font-mono);
        letter-spacing: 0.5px;
        cursor: pointer;
        padding: 0.2rem 0.5rem;
        transition: all 0.15s;
    }

    .nc-btn-ghost:hover:not(:disabled) {
        border-color: var(--color-primary);
        color: var(--color-primary);
    }

    .nc-btn-ghost:disabled {
        opacity: 0.3;
        cursor: not-allowed;
    }

    .nc-btn-ghost.danger {
        border-color: rgba(255, 80, 80, 0.3);
        color: #ff5555;
    }

    .nc-btn-ghost.danger:hover {
        border-color: #ff5555;
        background: rgba(255, 80, 80, 0.1);
    }

    .nc-clear-confirm {
        display: flex;
        gap: 0.3rem;
    }

    .nc-body {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
    }

    .nc-body::-webkit-scrollbar {
        width: 6px;
    }

    .nc-body::-webkit-scrollbar-track {
        background: rgba(0, 255, 65, 0.03);
    }

    .nc-body::-webkit-scrollbar-thumb {
        background: rgba(0, 255, 65, 0.2);
        border-radius: 0;
    }

    .nc-empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 2rem 1rem;
        gap: 0.5rem;
    }

    .nc-empty-icon {
        font-size: 1.2rem;
        color: var(--color-primary);
        opacity: 0.5;
    }

    .nc-empty-text {
        font-size: 0.7rem;
        color: #555;
        font-family: var(--font-mono);
    }

    .nc-item {
        padding: 0.5rem 1rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        transition: background 0.15s;
    }

    .nc-item.unread {
        background: rgba(0, 255, 65, 0.03);
        border-left: 2px solid var(--color-primary);
    }

    .nc-item:hover {
        background: rgba(0, 255, 65, 0.04);
    }

    .nc-item-bar {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        margin-bottom: 0.2rem;
    }

    .nc-severity-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        flex-shrink: 0;
    }

    .nc-severity-dot.severity-error {
        background: #ff5555;
        box-shadow: 0 0 4px rgba(255, 85, 85, 0.5);
    }

    .nc-severity-dot.severity-warning {
        background: #ffaa00;
        box-shadow: 0 0 4px rgba(255, 170, 0, 0.5);
    }

    .nc-severity-dot.severity-success {
        background: #00ff41;
        box-shadow: 0 0 4px rgba(0, 255, 65, 0.5);
    }

    .nc-severity-dot.severity-info {
        background: #4488ff;
        box-shadow: 0 0 4px rgba(68, 136, 255, 0.5);
    }

    .nc-category {
        font-size: 0.5rem;
        color: #555;
        letter-spacing: 0.5px;
        background: rgba(255, 255, 255, 0.03);
        padding: 0.1rem 0.35rem;
        border-radius: 3px;
    }

    .nc-time {
        font-size: 0.55rem;
        color: #444;
        margin-left: auto;
    }

    .nc-item-actions {
        display: flex;
        gap: 0.15rem;
        margin-left: 0.3rem;
    }

    .nc-btn-sm {
        background: transparent;
        border: 1px solid rgba(255, 255, 255, 0.06);
        border-radius: 3px;
        color: #555;
        cursor: pointer;
        font-size: 0.55rem;
        width: 18px;
        height: 18px;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        transition: all 0.15s;
    }

    .nc-btn-sm:hover {
        color: var(--color-primary);
        border-color: rgba(0, 255, 65, 0.3);
    }

    .nc-item-body {
        padding-left: 0.4rem;
    }

    .nc-item-title {
        font-size: 0.7rem;
        font-weight: 600;
        color: #ccc;
        margin-bottom: 0.15rem;
    }

    .nc-item-text {
        font-size: 0.65rem;
        color: #999;
        line-height: 1.3;
        word-break: break-word;
    }

    .nc-action-btn {
        background: rgba(0, 255, 65, 0.06);
        border: 1px solid rgba(0, 255, 65, 0.15);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.6rem;
        font-family: var(--font-mono);
        cursor: pointer;
        padding: 0.15rem 0.5rem;
        margin-top: 0.3rem;
        transition: all 0.15s;
    }

    .nc-action-btn:hover {
        background: rgba(0, 255, 65, 0.15);
        box-shadow: 0 0 8px rgba(0, 255, 65, 0.2);
    }

    .nc-action-message {
        position: absolute;
        right: 1rem;
        bottom: 0.75rem;
        background: rgba(0, 255, 65, 0.1);
        border: 1px solid rgba(0, 255, 65, 0.25);
        border-radius: 4px;
        color: var(--color-primary);
        font-size: 0.6rem;
        padding: 0.25rem 0.5rem;
        pointer-events: none;
    }

    @media (max-width: 520px) {
        .nc-panel {
            left: 12px;
            right: 12px;
            width: auto;
            max-width: none;
        }
    }
</style>
