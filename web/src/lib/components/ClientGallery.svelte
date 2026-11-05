<script lang="ts">
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';

	let {
		gallery,
		proofs = [],
		preview = false,
		contained = false
	}: {
		gallery: SharedGallery;
		proofs?: Proof[];
		/** The owner's read-only preview: marks the page and gates every write action. */
		preview?: boolean;
		/** Opens the photo viewer inside the nearest positioned ancestor, e.g. a preview frame. */
		contained?: boolean;
	} = $props();
	let viewerIndex = $state<number | null>(null);
	// Proofs are the ready photos, so the cover can only ever be a ready photo's preview.
	let cover = $derived(proofs.find(({ id }) => id === gallery.hero_id));
</script>

<main class="mx-auto grid max-w-7xl gap-4 px-4 py-6">
	{#if preview}
		<p class="justify-self-center rounded-sm border border-border-strong px-2 text-sm">Preview</p>
	{/if}
	{#if cover}
		<img
			src={cover.previewUrl}
			alt="Gallery cover"
			class="mx-auto max-h-[70vh] w-full rounded-card object-cover"
		/>
	{/if}
	<h1 class="text-center text-[2rem] wrap-anywhere">{gallery.title ?? UNTITLED_GALLERY}</h1>
	{#if gallery.expires_at}
		<ExpiryBadge expiresAt={gallery.expires_at} />
	{/if}
	{#if proofs.length === 0}
		<p class="text-center text-muted-foreground">
			The photographer hasn't added photos yet. Check back soon.
		</p>
	{:else}
		<ProofGrid {proofs} layout={gallery.layout} onopen={(i) => (viewerIndex = i)} />
	{/if}
</main>
{#if viewerIndex !== null}
	<ProofViewer {proofs} bind:index={viewerIndex} {contained} onclose={() => (viewerIndex = null)} />
{/if}
