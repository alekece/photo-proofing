<script lang="ts">
	import { buildVersion, displayVersion } from '#lib/build.js';
	import { legalNotices as defaultNotices, type LegalNotice } from '#lib/footer.js';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	let {
		version = buildVersion,
		legalNotices = defaultNotices,
		compact = false,
		class: className
	}: {
		version?: string;
		legalNotices?: LegalNotice[];
		compact?: boolean;
		class?: string;
	} = $props();
</script>

<footer
	class={cn(
		'grid justify-items-start gap-2 border-t border-border workspace-inset py-6 text-xs text-muted-foreground',
		compact && 'py-3',
		className
	)}
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
		<span class={cn(compact && 'sm:hidden')}>
			{#if version}
				<span class="font-mono" title={version}
					>{m.footer_build({ version: displayVersion(version) })}</span
				>
			{:else}
				{m.footer_development_build()}
			{/if}
		</span>
	</div>
</footer>
