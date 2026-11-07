<script lang="ts">
	import { tick } from 'svelte';
	import { Copy } from '@lucide/svelte';
	import Button from '#lib/components/ui/button/button.svelte';

	let { sha = import.meta.env.PP_BUILD_SHA }: { sha?: string } = $props();
	let status = $state('');
	let manual = $state(false);
	let input = $state<HTMLInputElement | null>(null);

	async function copy() {
		try {
			await navigator.clipboard.writeText(sha);
			manual = false;
			status = 'Copied';
		} catch {
			manual = true;
			status = 'Couldn’t copy automatically. Please copy it manually.';
			await tick();
			input?.focus();
			input?.select();
		}
	}
</script>

<footer
	class="mt-10 grid justify-items-start gap-2 border-t border-border pt-4 text-xs text-muted-foreground"
>
	{#if sha}
		<div class="flex items-center gap-2">
			<span class="font-mono" title={sha}>Build {sha.slice(0, 7)}</span>
			<Button variant="ghost" size="sm" aria-label="Copy build SHA" onclick={copy}>
				<Copy size={14} aria-hidden="true" />
			</Button>
		</div>
		{#if manual}
			<input
				bind:this={input}
				readonly
				value={sha}
				aria-label="Full build SHA"
				class="w-full max-w-sm rounded border border-border bg-input p-2 font-mono"
			/>
		{/if}
		<p role="status" class="empty:hidden">{status}</p>
	{:else}
		<span>Development build</span>
	{/if}
</footer>
