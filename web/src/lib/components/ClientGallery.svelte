<script lang="ts">
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';

	let { gallery, proofs = [] }: { gallery: SharedGallery; proofs?: Proof[] } = $props();
	let viewerIndex = $state<number | null>(null);
</script>

<main class="gallery">
	<h1 style="overflow-wrap: anywhere">{gallery.title ?? UNTITLED_GALLERY}</h1>
	{#if gallery.expires_at}
		<ExpiryBadge expiresAt={gallery.expires_at} />
	{/if}
	{#if proofs.length === 0}
		<p>The photographer hasn't added photos yet. Check back soon.</p>
	{:else}
		<ProofGrid {proofs} layout={gallery.layout} onopen={(i) => (viewerIndex = i)} />
	{/if}
</main>
{#if viewerIndex !== null}
	<ProofViewer {proofs} bind:index={viewerIndex} onclose={() => (viewerIndex = null)} />
{/if}

<style>
	.gallery {
		display: grid;
		gap: 1rem;
		max-width: 80rem;
		margin: 0 auto;
		padding: 1.5rem 1rem;
	}
	h1 {
		margin: 0;
		font-family: var(--font-serif);
		font-weight: 400;
		font-size: 2rem;
		text-align: center;
	}
	p {
		margin: 0;
		text-align: center;
		color: var(--muted-foreground);
	}
</style>
