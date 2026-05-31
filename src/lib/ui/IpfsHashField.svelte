<script>
    import ContentLibraryPicker from "./ContentLibraryPicker.svelte";

    export let value = "";
    export let id = "ipfs-hash";
    export let disabled = false;

    function openContentLibrary() {
        window.dispatchEvent(new CustomEvent("commander-open-content-library"));
    }
</script>

<ContentLibraryPicker {id} bind:value placeholder="Qm..." {disabled} on:openLibrary={openContentLibrary} />
