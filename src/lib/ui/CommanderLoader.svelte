<script>
    export let label = "Loading";
    export let detail = "";
    export let compact = false;
    export let overlay = false;
</script>

<div class="loader-wrap" class:overlay={overlay} class:compact={compact} role="status" aria-live="polite">
    <div class="loader-core">
        <div class="ring"></div>
        <div class="dot"></div>
    </div>
    <div class="loader-text">
        <div class="loader-label">{label}</div>
        {#if detail}
            <div class="loader-detail mono">{detail}</div>
        {/if}
    </div>
</div>

<style>
    .loader-wrap {
        display: inline-flex;
        align-items: center;
        gap: 0.55rem;
        padding: 0.45rem 0.65rem;
        border: 1px solid rgba(0, 255, 65, 0.18);
        border-radius: 8px;
        background: rgba(2, 4, 3, 0.9);
        color: #cfd8d2;
    }

    .loader-wrap.compact {
        padding: 0.35rem 0.5rem;
        gap: 0.45rem;
    }

    .loader-wrap.overlay {
        position: absolute;
        inset: 0;
        margin: auto;
        width: fit-content;
        height: fit-content;
        z-index: 20;
    }

    .loader-core {
        position: relative;
        width: 20px;
        height: 20px;
        flex-shrink: 0;
    }

    .loader-wrap.compact .loader-core {
        width: 16px;
        height: 16px;
    }

    .ring {
        width: 100%;
        height: 100%;
        border-radius: 50%;
        border: 2px solid rgba(0, 255, 65, 0.18);
        border-top-color: var(--color-primary);
        animation: spin 1s linear infinite;
        box-sizing: border-box;
    }

    .dot {
        position: absolute;
        width: 4px;
        height: 4px;
        border-radius: 50%;
        background: var(--color-primary);
        left: 50%;
        top: 50%;
        transform: translate(-50%, -50%);
    }

    .loader-label {
        font-size: 0.72rem;
        letter-spacing: 0.4px;
        color: #cfd8d2;
        font-weight: 600;
        line-height: 1.1;
    }

    .loader-detail {
        margin-top: 0.12rem;
        font-size: 0.62rem;
        color: #8a9a90;
        line-height: 1.2;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }
</style>
