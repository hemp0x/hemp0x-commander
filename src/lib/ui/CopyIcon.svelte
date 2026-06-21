<script>
    export let size = 13;
</script>

<span
    class="copy-icon"
    style={`--copy-icon-size:${size}px`}
    aria-hidden="true"
>
    <span></span>
    <span></span>
</span>

<style>
    .copy-icon {
        position: relative;
        display: inline-block;
        width: var(--copy-icon-size);
        height: var(--copy-icon-size);
        flex: 0 0 auto;
    }

    .copy-icon span {
        position: absolute;
        width: 68%;
        height: 68%;
        border: 1.5px solid currentColor;
        border-radius: 1px;
        box-sizing: border-box;
    }

    .copy-icon span:first-child {
        top: 0;
        left: 0;
    }

    .copy-icon span:last-child {
        right: 0;
        bottom: 0;
        background: inherit;
    }
</style>
