<script lang="ts" module>
	export type SaveStatus = 'idle' | 'saving';
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { LoaderCircle } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';

	let {
		status,
		disabled = false,
		onclick,
		class: className = ''
	}: {
		/** The owner drives the pending state. */
		status: SaveStatus;
		/** Nothing to save. While saving the button ignores clicks without greying out. */
		disabled?: boolean;
		onclick: () => void;
		class?: string;
	} = $props();

	const busy = $derived(status !== 'idle');
</script>

<Button
	{disabled}
	size="sm"
	aria-disabled={busy || undefined}
	aria-label={busy ? m.studio_saving() : undefined}
	onclick={() => !busy && onclick()}
	class="relative inline-grid aria-disabled:cursor-default {className}"
>
	<span class:invisible={busy}>{m.settings_save()}</span>
	{#if busy}
		<span class="absolute inset-0 grid place-items-center" aria-hidden="true">
			<LoaderCircle size={16} class="animate-spin motion-reduce:animate-none" />
		</span>
	{/if}
</Button>
<span role="status" class="sr-only">
	{status === 'saving' ? m.studio_saving() : ''}
</span>
