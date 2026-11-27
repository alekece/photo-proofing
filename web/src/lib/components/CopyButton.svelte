<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { Check, Copy } from '@lucide/svelte';
	import ClickPop from '#lib/components/ClickPop.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import { cn } from '#lib/utils.js';

	const COPIED_MS = 1000;

	let {
		value,
		disabled = false,
		fallbackInput = null,
		class: className = 'min-w-28',
		statusClass = 'text-xs',
		icon = false
	}: {
		value: string;
		disabled?: boolean;
		fallbackInput?: HTMLInputElement | null;
		class?: string;
		statusClass?: string;
		/** A bare icon that turns into a check, for sitting inside a field. */
		icon?: boolean;
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

{#if icon}
	<button
		type="button"
		class={cn(
			'icon-action relative inline-flex size-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-focus disabled:pointer-events-none disabled:opacity-50',
			copied && 'text-ok',
			className
		)}
		aria-label={copied ? m.copy_copied() : m.copy_copy()}
		title={copied ? m.copy_copied() : m.copy_copy()}
		{disabled}
		onclick={copy}
	>
		{#if copied}
			<ClickPop active={popping} onend={() => (popping = false)}>
				<Check size={16} aria-hidden="true" />
			</ClickPop>
		{:else}
			<Copy size={16} aria-hidden="true" />
		{/if}
	</button>
{:else}
	<Button
		size="sm"
		class={cn('min-h-9! w-28 min-w-28 justify-center gap-1.5', className)}
		aria-label={copied ? m.copy_copied() : m.copy_copy()}
		{disabled}
		onclick={copy}
	>
		{#if copied}
			<span class="text-ok-on-dark">
				<ClickPop active={popping} onend={() => (popping = false)}>
					<Check size={16} aria-hidden="true" />
				</ClickPop>
			</span>
		{:else}
			<Copy size={16} aria-hidden="true" />
			{m.copy_copy()}
		{/if}
	</Button>
{/if}
<p class={failed ? statusClass : 'sr-only'} role="status">
	{copied ? m.copy_copied() : failed ? m.copy_failed() : ''}
</p>
