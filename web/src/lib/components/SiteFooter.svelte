<script lang="ts">
	import { tick } from 'svelte';
	import { Copy } from '@lucide/svelte';
	import { buildVersion, displayVersion } from '#lib/build.js';
	import { legalNotices as defaultNotices, type LegalNotice } from '#lib/footer.js';
	import Button from '#lib/components/ui/button/button.svelte';

	let {
		version = buildVersion,
		copyable = true,
		legalNotices = defaultNotices
	}: {
		version?: string;
		/** Photographer pages offer copying the full version; the client gallery only shows it. */
		copyable?: boolean;
		legalNotices?: LegalNotice[];
	} = $props();
	let status = $state('');
	let manual = $state(false);
	let input = $state<HTMLInputElement | null>(null);

	async function copy() {
		try {
			await navigator.clipboard.writeText(version);
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
	class="grid justify-items-start gap-2 border-t border-border px-3.5 py-6 text-xs text-muted-foreground sm:px-8"
>
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<span class="font-serif text-sm text-foreground italic">PhotoProofing</span>
		<span>© {new Date().getFullYear()} PhotoProofing</span>
		{#if legalNotices.length}
			<nav aria-label="Legal" class="flex gap-4">
				{#each legalNotices as notice (notice.href)}
					<a href={notice.href} class="underline-offset-2 hover:underline">{notice.label}</a>
				{/each}
			</nav>
		{/if}
		{#if version}
			<span class="flex items-center gap-1">
				<span class="font-mono" title={version}>Build {displayVersion(version)}</span>
				{#if copyable}
					<Button variant="ghost" size="sm" aria-label="Copy build version" onclick={copy}>
						<Copy size={14} aria-hidden="true" />
					</Button>
				{/if}
			</span>
		{:else}
			<span>Development build</span>
		{/if}
	</div>
	{#if manual}
		<input
			bind:this={input}
			readonly
			value={version}
			aria-label="Full build version"
			class="w-full max-w-sm rounded border border-border bg-input p-2 font-mono"
		/>
	{/if}
	<p role="status" class="empty:hidden">{status}</p>
</footer>
