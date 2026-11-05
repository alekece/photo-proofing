<script lang="ts">
	import PhotosPanel from '#lib/components/PhotosPanel.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import Tabs from '#lib/components/ui/tabs/tabs.svelte';

	let { galleryId }: { galleryId: string } = $props();

	const TABS = [
		{ id: 'photos', label: 'Photos' },
		{
			id: 'favorites',
			label: 'Favorites',
			action: {
				label: 'Download',
				title: 'Downloads arrive with a later release',
				hint: 'Client selection'
			},
			empty: 'No favorites yet. The client’s selection will appear here.'
		},
		{
			id: 'finals',
			label: 'Finals',
			action: {
				label: '+ Add finished photos',
				title: 'Uploading finished photos arrives with a later release'
			},
			empty: 'No finished photos yet. Drop the edited JPEGs here.'
		}
	] as const;

	let tab = $state('photos');
</script>

<section class="min-w-0">
	<Tabs bind:value={tab} items={TABS} label="Gallery content">
		{#snippet panel(active)}
			{#each TABS as item (item.id)}
				{#if active === item.id && !('action' in item)}
					<PhotosPanel {galleryId} />
				{:else if active === item.id && 'action' in item}
					{@const { action, empty } = item}
					{#if 'hint' in action}
						<div class="flex flex-wrap items-center justify-between gap-2">
							<span class="text-sm text-muted-foreground">{action.hint}</span>
							<Button size="sm" disabled title={action.title}>{action.label}</Button>
						</div>
					{:else}
						<div class="flex justify-end">
							<Button variant="outline" size="sm" disabled title={action.title}>
								{action.label}
							</Button>
						</div>
					{/if}
					<EmptyState class="text-muted-foreground">{empty}</EmptyState>
				{/if}
			{/each}
		{/snippet}
	</Tabs>
</section>
