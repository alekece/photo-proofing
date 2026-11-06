<script lang="ts">
	import type { Signature } from '#lib/settings/settings.js';

	/** The studio's mark and name as printed on a cover, per the Settings signature choice. */
	let {
		studio = null,
		logo = null,
		signature = 'name',
		stacked = false,
		class: className = ''
	}: {
		studio?: string | null;
		/** Image URL of the studio logo. */
		logo?: string | null;
		signature?: Signature;
		/** Logo above the name, as on the empty "photos on their way" page. */
		stacked?: boolean;
		class?: string;
	} = $props();

	const name = $derived(studio?.trim() || null);
	const showLogo = $derived(logo !== null && (signature === 'logo' || signature === 'both'));
	// A logo-only signature falls back to the name when there is no logo.
	const showName = $derived(
		name !== null &&
			(signature === 'name' || signature === 'both' || (signature === 'logo' && logo === null))
	);
</script>

{#if showLogo || showName}
	<div
		data-testid="studio-signature"
		class="flex items-center justify-center gap-2.5 {stacked ? 'flex-col gap-2' : ''} {className}"
	>
		{#if showLogo}
			<img src={logo} alt="" class={stacked ? 'size-11' : 'size-[2.2em]'} />
		{/if}
		{#if showName}
			<span
				class="text-[0.62em] font-semibold tracking-[0.22em] whitespace-nowrap uppercase not-italic"
				>{name}</span
			>
		{/if}
	</div>
{/if}
