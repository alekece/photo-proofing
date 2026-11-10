<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import CommentsPanel from '#lib/components/CommentsPanel.svelte';
	import FavoritesPanel from '#lib/components/FavoritesPanel.svelte';
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import type { GalleryLayout } from '#lib/gallery/api.js';
	import Tabs from '#lib/components/ui/Tabs/Tabs.svelte';

	let {
		galleryId,
		included,
		layout = 'masonry',
		heroId = $bindable(null),
		heroRevision = $bindable(),
		delivered = false,
		shared,
		photoCount = $bindable(null),
		favoriteCount = $bindable(null)
	}: {
		galleryId: string;
		included: number | null;
		layout?: GalleryLayout;
		heroId?: string | null;
		heroRevision?: number;
		/** After delivery the client's selection is locked. */
		delivered?: boolean;
		/** Favorites and finals become available after the first share. */
		shared: boolean;
		/** Reported upwards so the gallery header can show them. */
		photoCount?: number | null;
		favoriteCount?: number | null;
	} = $props();

	let tab = $state('photos');
	let commentCount = $state<number | null>(null);
	let finalCount = $state<number | null>(null);
	// Saving replaces the parent gallery object; equal IDs must not reset loaded panels.
	const id = $derived(galleryId);

	const items = $derived([
		{ id: 'photos', label: m.common_photos(), count: photoCount },
		{
			id: 'favorites',
			label: m.tab_favorites(),
			count: favoriteCount,
			accent: included !== null && favoriteCount !== null && favoriteCount > included,
			locked: delivered,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		},
		{
			id: 'comments',
			label: m.tab_comments(),
			count: commentCount,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		},
		{
			id: 'final',
			label: m.tab_final(),
			count: finalCount,
			disabled: !shared,
			title: shared ? undefined : m.tab_after_sharing()
		}
	]);
</script>

<section class="min-w-0 px-3 pt-3 pb-8 sm:px-7 sm:pt-[1.125rem] sm:pb-10">
	<Tabs bind:value={tab} {items} label={m.tabs_label()}>
		{#snippet panel(active)}
			<!-- All panels stay mounted so every tab can show its count; only the active one is visible. -->
			<div hidden={active !== 'photos'}>
				<PhotosPanel
					galleryId={id}
					bind:heroId
					bind:heroRevision
					{layout}
					oncount={(n) => (photoCount = n)}
				/>
			</div>
			<div hidden={active !== 'favorites'} class="grid gap-4">
				<FavoritesPanel galleryId={id} {layout} oncount={(n) => (favoriteCount = n)} />
			</div>
			<div hidden={active !== 'comments'} class="grid gap-4">
				<CommentsPanel galleryId={id} {layout} oncount={(n) => (commentCount = n)} />
			</div>
			<div hidden={active !== 'final'}>
				<PhotosPanel set="finals" galleryId={id} {layout} oncount={(n) => (finalCount = n)} />
			</div>
		{/snippet}
	</Tabs>
</section>
