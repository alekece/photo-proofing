<script lang="ts">
	import FavoritesPanel from '#lib/components/FavoritesPanel.svelte';
	import FinalPanel from '#lib/components/FinalPanel.svelte';
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import Tabs from '#lib/components/ui/tabs/tabs.svelte';

	let {
		galleryId,
		included,
		heroId = $bindable(null),
		watermarked = false,
		delivered = false,
		photoCount = $bindable(null),
		favoriteCount = $bindable(null)
	}: {
		galleryId: string;
		included: number | null;
		heroId?: string | null;
		watermarked?: boolean;
		/** After delivery the client's selection is locked. */
		delivered?: boolean;
		/** Reported upwards so the gallery header can show them. */
		photoCount?: number | null;
		favoriteCount?: number | null;
	} = $props();

	let tab = $state('photos');
	let finalCount = $state<number | null>(null);

	const items = $derived([
		{ id: 'photos', label: 'Photos', count: photoCount },
		{
			id: 'favorites',
			label: 'Favorites',
			count: favoriteCount,
			accent: included !== null && favoriteCount !== null && favoriteCount > included,
			locked: delivered
		},
		{ id: 'final', label: 'Final', count: finalCount }
	]);
</script>

<section class="min-w-0">
	<Tabs bind:value={tab} {items} label="Gallery content">
		{#snippet panel(active)}
			<!-- All panels stay mounted so every tab can show its count; only the active one is visible. -->
			<div hidden={active !== 'photos'}>
				<PhotosPanel {galleryId} bind:heroId oncount={(n) => (photoCount = n)} {watermarked} />
			</div>
			<div hidden={active !== 'favorites'} class="grid gap-4">
				<FavoritesPanel {galleryId} {included} oncount={(n) => (favoriteCount = n)} />
			</div>
			<div hidden={active !== 'final'}>
				<FinalPanel oncount={(n) => (finalCount = n)} />
			</div>
		{/snippet}
	</Tabs>
</section>
