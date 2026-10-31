<script lang="ts">
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';

	let { gallery, proofs = [] }: { gallery: SharedGallery; proofs?: Proof[] } = $props();
	let viewerIndex = $state<number | null>(null);
</script>

<main class="mx-auto grid max-w-7xl gap-4 px-4 py-6">
	<h1 class="text-center text-[2rem] wrap-anywhere">{gallery.title ?? UNTITLED_GALLERY}</h1>
	{#if gallery.expires_at}
		<ExpiryBadge expiresAt={gallery.expires_at} />
	{/if}
	{#if proofs.length === 0}
		<p class="text-center text-muted-foreground">The photographer hasn't added photos yet. Check back soon.</p>
	{:else}
		<ProofGrid {proofs} layout={gallery.layout} onopen={(i) => (viewerIndex = i)} />
	{/if}
</main>
{#if viewerIndex !== null}
	<ProofViewer {proofs} bind:index={viewerIndex} onclose={() => (viewerIndex = null)} />
{/if}
