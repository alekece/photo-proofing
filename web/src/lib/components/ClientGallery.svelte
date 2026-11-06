<script lang="ts">
	import { onMount } from 'svelte';
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import { Selection } from '#lib/gallery/selection.svelte.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';

	let {
		token = '',
		gallery,
		proofs = [],
		favorites = [],
		preview = false,
		contained = false
	}: {
		/** The private link's token; the owner's preview has none. */
		token?: string;
		gallery: SharedGallery;
		proofs?: Proof[];
		favorites?: string[];
		/** The owner's read-only preview: marks the page and gates every write action. */
		preview?: boolean;
		/** Opens the photo viewer inside the nearest positioned ancestor, e.g. a preview frame. */
		contained?: boolean;
	} = $props();
	const SEPARATOR = ' · ';
	let viewerIndex = $state<number | null>(null);

	// The page is loaded once per link, so the initial values are read once on purpose.
	// svelte-ignore state_referenced_locally
	const selection = new Selection({ token, initial: favorites });
	const select = $derived(
		preview || gallery.delivered ? undefined : (id: string) => void selection.toggle(id)
	);
	const extra = $derived(
		gallery.included_photos === null ? 0 : Math.max(0, selection.count - gallery.included_photos)
	);

	const summary = $derived(
		`${selection.count} selected` +
			(gallery.included_photos === null ? '' : `${SEPARATOR}${gallery.included_photos} included`)
	);

	// Another browser on the same link changes the selection; a resync means events were missed.
	onMount(() => {
		if (preview || gallery.delivered) return;

		const source = new EventSource(`/api/v1/galleries/shared/${encodeURIComponent(token)}/events`);
		const refresh = () => void selection.refresh();

		source.addEventListener('favorites', refresh);
		source.addEventListener('resync', refresh);

		return () => source.close();
	});
	// Proofs are the ready photos, so the cover can only ever be a ready photo's preview.
	let cover = $derived(proofs.find(({ id }) => id === gallery.hero_id));
</script>

<main class="mx-auto grid max-w-7xl gap-4 px-4 py-6">
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
		<p class="text-center">
			{summary}{#if extra > 0}{SEPARATOR}<span class="font-bold text-warning">{extra} extra</span
				>{/if}
		</p>
		{#if selection.error}
			<p role="alert" class="text-center font-bold text-warning">{selection.error}</p>
		{/if}
		<ProofGrid
			{proofs}
			layout={gallery.layout}
			isSelected={(id) => selection.has(id)}
			readonly={preview}
			onopen={(i) => (viewerIndex = i)}
			onselect={select}
		/>
	{/if}
</main>
{#if viewerIndex !== null}
	<ProofViewer
		{proofs}
		bind:index={viewerIndex}
		{contained}
		readonly={preview}
		isSelected={(id) => selection.has(id)}
		onclose={() => (viewerIndex = null)}
		onselect={select}
	/>
{/if}
