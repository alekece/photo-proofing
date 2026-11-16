<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { Check } from '@lucide/svelte';
	import ClickPop from '#lib/components/ClickPop.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import { cn } from '#lib/utils.js';

	const COPIED_MS = 3000;

	let {
		value,
		disabled = false,
		fallbackInput = null,
		class: className = 'min-w-28',
		statusClass = 'text-xs'
	}: {
		value: string;
		disabled?: boolean;
		fallbackInput?: HTMLInputElement | null;
		class?: string;
		statusClass?: string;
	} = $props();

	let copied = $state(false);
	let popping = $state(false);
	let failed = $state(false);
	// svelte-ignore state_referenced_locally
	let lastValue = value;
	let timer: ReturnType<typeof setTimeout> | undefined;

	onDestroy(() => clearTimeout(timer));

	$effect.pre(() => {
		if (value !== lastValue) {
			lastValue = value;
			clearTimeout(timer);
			copied = false;
			popping = false;
			failed = false;
		}
	});

	async function copy() {
		if (disabled) return;
		clearTimeout(timer);
		try {
			await navigator.clipboard.writeText(value);
			failed = false;
			copied = true;
			popping = true;
			timer = setTimeout(() => (copied = false), COPIED_MS);
		} catch {
			copied = false;
			failed = true;
			fallbackInput?.focus();
			fallbackInput?.select();
		}
	}
</script>

<Button size="sm" class={cn('min-h-9! gap-1.5', className)} {disabled} onclick={copy}>
	{#if copied}
		<span class="text-ok-on-dark">
			<ClickPop active={popping} onend={() => (popping = false)}>
				<Check size={16} aria-hidden="true" />
			</ClickPop>
		</span>
		{m.copy_copied()}
	{:else}
		{m.copy_copy()}
	{/if}
</Button>
<!-- The button already shows "Copied"; only the announcement is hidden visually. -->
<p class={failed ? statusClass : 'sr-only'} role="status">
	{copied ? m.copy_copied() : failed ? m.copy_failed() : ''}
</p>
