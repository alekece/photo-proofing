<script lang="ts">
	import { untrack } from 'svelte';
	import Dialog from './dialog.svelte';

	let { onclose, opened = true }: { onclose: () => void; opened?: boolean } = $props();
	let open = $state(untrack(() => opened));
</script>

<button type="button" onclick={() => (open = true)}>Open</button>
<button type="button" onclick={() => (open = false)}>Unmount</button>
{#if open}
	<Dialog {onclose} label="Example">
		{#snippet children(close)}
			<button type="button" onclick={close}>Cancel</button>
		{/snippet}
	</Dialog>
{/if}
