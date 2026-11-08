<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onDestroy } from 'svelte';
	import { Check } from '@lucide/svelte';
	import ClickPop from '#lib/components/ClickPop.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';

	const COPIED_MS = 3000;

	let { link, label = m.copy_link_label() }: { link: string; label?: string } = $props();

	let copied = $state(false);
	let popping = $state(false);
	let failed = $state(false);
	let input = $state<HTMLInputElement | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	onDestroy(() => clearTimeout(timer));

	async function copyLink() {
		clearTimeout(timer);
		try {
			await navigator.clipboard.writeText(link);
			failed = false;
			copied = true;
			popping = true;
			timer = setTimeout(() => (copied = false), COPIED_MS);
		} catch {
			copied = false;
			failed = true;
			input?.focus();
			input?.select();
		}
	}
</script>

<div class="grid gap-2.5">
	<div class="flex gap-2">
		<Input
			bind:ref={input}
			readonly
			value={link}
			aria-label={label}
			class="bg-muted/40 min-h-0 min-w-0 flex-1 cursor-default truncate rounded-lg border-border px-2.5 py-[0.5625rem] font-mono text-sm text-muted-foreground focus-visible:outline-none"
		/>
		<Button size="sm" class="min-h-9! min-w-28 gap-1.5" onclick={copyLink}>
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
	</div>
	<!-- The button already shows "Copied"; only the announcement is hidden visually. -->
	<p class={failed ? 'text-xs' : 'sr-only'} role="status">
		{copied ? m.copy_copied() : failed ? m.copy_failed() : ''}
	</p>
</div>
