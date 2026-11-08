<script lang="ts">
	import { untrack } from 'svelte';
	import Dialog from './Dialog.svelte';

	let {
		onclose,
		opened = true,
		modal = true
	}: { onclose: () => void; opened?: boolean; modal?: boolean } = $props();
	let open = $state(untrack(() => opened));
</script>

<button type="button" onclick={() => (open = true)}>Open</button>
<button type="button" onclick={() => (open = false)}>Unmount</button>
{#if open}
	<Dialog {onclose} {modal} label="Example">
		{#snippet children(close)}
			<button type="button" onclick={close}>Cancel</button>
		{/snippet}
	</Dialog>
{/if}
