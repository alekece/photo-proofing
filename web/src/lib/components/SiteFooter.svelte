<script lang="ts">
	import { buildVersion, displayVersion } from '#lib/build.js';
	import { legalNotices as defaultNotices, type LegalNotice } from '#lib/footer.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		version = buildVersion,
		legalNotices = defaultNotices
	}: {
		version?: string;
		legalNotices?: LegalNotice[];
	} = $props();
</script>

<footer
	class="grid justify-items-start gap-2 border-t border-border px-3.5 py-6 text-xs text-muted-foreground sm:px-8"
>
	<div class="flex flex-wrap items-center gap-x-4 gap-y-1">
		<span class="font-serif text-sm text-foreground italic">PhotoProofing</span>
		<span>© {new Date().getFullYear()} PhotoProofing</span>
		{#if legalNotices.length}
			<nav aria-label={m.footer_legal()} class="flex gap-4">
				{#each legalNotices as notice (notice.href)}
					<a href={notice.href} class="underline-offset-2 hover:underline">{notice.label}</a>
				{/each}
			</nav>
		{/if}
		{#if version}
			<span class="font-mono" title={version}
				>{m.footer_build({ version: displayVersion(version) })}</span
			>
		{:else}
			<span>{m.footer_development_build()}</span>
		{/if}
	</div>
</footer>
