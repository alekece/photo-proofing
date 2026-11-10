<script lang="ts" module>
	export type SaveStatus = 'idle' | 'saving' | 'saved';
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, LoaderCircle } from '@lucide/svelte';
	import ButtonFace from '#lib/components/ButtonFace.svelte';
	import ClickRipple from '#lib/components/ClickRipple.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';

	let {
		status,
		disabled = false,
		onclick,
		class: className = ''
	}: {
		/** The owner drives the lifecycle: idle, then saving, then saved, then idle again. */
		status: SaveStatus;
		/** Nothing to save. While saving or saved the button ignores clicks without greying out. */
		disabled?: boolean;
		onclick: () => void;
		class?: string;
	} = $props();

	const busy = $derived(status !== 'idle');
</script>

<Button
	{disabled}
	aria-disabled={busy || undefined}
	onclick={() => !busy && onclick()}
	class="relative inline-grid aria-disabled:cursor-default {className}"
>
	<ButtonFace shown={status === 'idle'}>{m.studio_save()}</ButtonFace>
	<ButtonFace shown={status === 'saving'}>
		{#snippet icon()}
			<LoaderCircle size={16} class="animate-spin motion-reduce:animate-none" aria-hidden="true" />
		{/snippet}
		{m.studio_saving()}
	</ButtonFace>
	<ButtonFace shown={status === 'saved'}>
		{#snippet icon()}
			<span class="relative inline-grid">
				<Check size={16} strokeWidth={2.8} class="text-ok-on-dark" aria-hidden="true" />
				{#if status === 'saved'}<ClickRipple class="text-ok-on-dark" />{/if}
			</span>
		{/snippet}
		{m.studio_saved()}
	</ButtonFace>
</Button>
<span role="status" class="sr-only">
	{status === 'saving' ? m.studio_saving() : status === 'saved' ? m.studio_saved() : ''}
</span>
