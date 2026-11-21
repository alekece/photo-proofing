<script lang="ts">
	import { untrack } from 'svelte';
	import Dialog from './Dialog.svelte';

	let {
		onclose,
		opened = true,
		modal = true,
		formMotion = false
	}: { onclose: () => void; opened?: boolean; modal?: boolean; formMotion?: boolean } = $props();
	let open = $state(untrack(() => opened));
</script>

<button type="button" onclick={() => (open = true)}>Open</button>
<button type="button" onclick={() => (open = false)}>Unmount</button>
{#if open}
	<Dialog {onclose} {modal} {formMotion} label="Example">
		{#snippet children(close, complete)}
			<button type="button" onclick={close}>Cancel</button>
			{#if formMotion}
				<button type="button" onclick={complete}>Save</button>
			{/if}
		{/snippet}
	</Dialog>
{/if}
