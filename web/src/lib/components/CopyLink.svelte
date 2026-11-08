<script lang="ts">
	import { onDestroy } from 'svelte';
	import { Check } from '@lucide/svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';

	const COPIED_MS = 3000;

	let { link, label = 'Client link' }: { link: string; label?: string } = $props();

	let copied = $state(false);
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
				<Check size={16} aria-hidden="true" class="animate-copied-pop text-ok-on-dark" />
				Copied
			{:else}
				Copy
			{/if}
		</Button>
	</div>
	<p class="text-xs empty:hidden" role="status">
		{copied ? 'Copied' : failed ? 'Couldn’t copy automatically. Please copy it manually.' : ''}
	</p>
</div>
