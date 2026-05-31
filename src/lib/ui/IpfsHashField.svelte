<script>
    import ContentLibraryPicker from "./ContentLibraryPicker.svelte";

    export let value = "";
    export let id = "ipfs-hash";
    export let disabled = false;
</script>

<ContentLibraryPicker {id} bind:value placeholder="Qm..." {disabled} />
