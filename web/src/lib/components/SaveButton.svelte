<script lang="ts" module>
	export type SaveStatus = 'idle' | 'saving' | 'saved';
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check, LoaderCircle } from '@lucide/svelte';
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
		/** Nothing to save; the saving and saved faces stay readable while disabled. */
		disabled?: boolean;
		onclick: () => void;
		class?: string;
	} = $props();
	const face =
		'col-start-1 row-start-1 inline-flex items-center justify-center gap-2 transition-[opacity,transform] duration-300';
</script>

<Button
	{disabled}
	{onclick}
	class="relative inline-grid {status === 'saved' ? 'disabled:opacity-100' : ''} {className}"
>
	<span
		class="{face} {status === 'idle' ? '' : '-translate-y-2 opacity-0'}"
		aria-hidden={status !== 'idle'}
	>
		{m.studio_save()}
	</span>
	<span
		class="{face} {status === 'saving' ? '' : 'translate-y-2 opacity-0'}"
		aria-hidden={status !== 'saving'}
	>
		<LoaderCircle
			size={16}
			class="animate-spin motion-reduce:animate-none"
			aria-hidden="true"
		/>{m.studio_saving()}
	</span>
	<span
		class="{face} {status === 'saved' ? '' : 'translate-y-2 opacity-0'}"
		aria-hidden={status !== 'saved'}
	>
		<Check
			size={16}
			strokeWidth={2.8}
			class="text-ok-on-dark"
			aria-hidden="true"
		/>{m.studio_saved()}
	</span>
	{#if status === 'saved'}<ClickRipple class="rounded-[inherit] text-ok-on-dark" />{/if}
</Button>
<span role="status" class="sr-only">
	{status === 'saving' ? m.studio_saving() : status === 'saved' ? m.studio_saved() : ''}
</span>
