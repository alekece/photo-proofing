<script lang="ts">
	import AccessUnavailable from '#lib/components/AccessUnavailable.svelte';
	import ExpiryBadge from '#lib/components/ExpiryBadge.svelte';
	import GalleryStateNotice from '#lib/components/GalleryStateNotice.svelte';
	import ProofGrid from '#lib/components/ProofGrid.svelte';
	import ProofViewer from '#lib/components/ProofViewer.svelte';
	import type { ClientGalleryView } from '#lib/gallery/types.js';

	let { data }: { data: { view: ClientGalleryView } } = $props();
	let viewerIndex = $state<number | null>(null);
</script>

<svelte:head>
	<title>{data.view.access === 'granted' ? data.view.title : 'Gallery unavailable'}</title>
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex" />
</svelte:head>

{#if data.view.access === 'granted'}
	<main class="gallery">
		<h1>{data.view.title}</h1>
		<ExpiryBadge expiresAt={data.view.expiresAt} />
		<GalleryStateNotice state={data.view.state} />
		<ProofGrid proofs={data.view.proofs} onopen={(i) => (viewerIndex = i)} />
	</main>
	{#if viewerIndex !== null}
		<ProofViewer
			proofs={data.view.proofs}
			bind:index={viewerIndex}
			onclose={() => (viewerIndex = null)}
		/>
	{/if}
{:else}
	<AccessUnavailable />
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
	}
</style>
