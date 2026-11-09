<script lang="ts">
	import { DEFAULT_SIZE, signatureScale, type Signature } from '#lib/settings/settings.js';

	/** The studio's mark and name as printed on a cover, per the Settings signature choice. */
	let {
		studio = null,
		logo = null,
		signature = 'name',
		size = DEFAULT_SIZE,
		stacked = false,
		class: className = ''
	}: {
		studio?: string | null;
		/** Image URL of the studio logo. */
		logo?: string | null;
		signature?: Signature;
		/** Size step from Settings; the default step is the original size. */
		size?: number;
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
		style="--signature-scale:{signatureScale(size)}"
		class="flex items-center justify-center gap-2.5 {stacked || signature === 'both'
			? 'flex-col gap-2'
			: ''} {className}"
	>
		{#if showLogo}
			<img
				src={logo}
				alt=""
				class={stacked
					? 'size-[calc(2.75rem*var(--signature-scale))]'
					: 'size-[calc(2.2em*var(--signature-scale))]'}
			/>
		{/if}
		{#if showName}
			<span
				class="text-[calc(0.62em*var(--signature-scale))] font-semibold tracking-[0.22em] whitespace-nowrap uppercase not-italic"
				>{name}</span
			>
		{/if}
	</div>
{/if}
