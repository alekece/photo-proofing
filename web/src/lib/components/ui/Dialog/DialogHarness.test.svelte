<script lang="ts">
	import { untrack } from 'svelte';
	import Dialog from './Dialog.svelte';

	let {
		onclose,
		opened = true,
		modal = true,
		dismissible = true,
		formMotion = false,
		structured = false
	}: {
		onclose: () => void;
		opened?: boolean;
		modal?: boolean;
		dismissible?: boolean;
		formMotion?: boolean;
		structured?: boolean;
	} = $props();
	let open = $state(untrack(() => opened));
</script>

{#snippet legacyChildren(close: () => void, complete: () => void)}
	<button type="button" onclick={close}>Cancel</button>
	{#if formMotion}
		<button type="button" onclick={complete}>Save</button>
	{/if}
{/snippet}

{#snippet header()}<h1>Structured title</h1>{/snippet}
{#snippet content()}<div data-testid="structured-content">Structured body</div>{/snippet}
{#snippet actions(close: () => void)}
	<button type="button" onclick={close}>Cancel</button>
	<button type="button">Save</button>
{/snippet}

<button type="button" onclick={() => (open = true)}>Open</button>
<button type="button" onclick={() => (open = false)}>Unmount</button>
{#if open}
	<Dialog
		onclose={() => {
			open = false;
			onclose();
		}}
		{modal}
		{dismissible}
		{formMotion}
		label="Example"
		header={structured ? header : undefined}
		content={structured ? content : undefined}
		actions={structured ? actions : undefined}
		children={structured ? undefined : legacyChildren}
	/>
{/if}
