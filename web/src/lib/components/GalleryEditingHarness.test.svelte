<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import { setUploadQueue, type UploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { Gallery } from '#lib/gallery/api.js';
	import GalleryStudio from './GalleryStudio.svelte';
	let { gallery, queue, events }: { gallery: Gallery; queue: UploadQueue; events: LiveEvents } =
		$props();
	// svelte-ignore state_referenced_locally
	setUploadQueue(queue);
	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<GalleryStudio
	{gallery}
	shareToken={gallery.share_token}
	origin="https://pp.test"
	onsave={async () => true}
	schedule={() => {}}
	cancel={() => {}}
	onpreview={() => {}}
	onshare={() => {}}
	onpublish={() => {}}
	onarchive={() => {}}
	onrestore={() => {}}
	ondelete={() => {}}
>
	{#snippet heading()}
		<h1>{gallery.title ?? 'Untitled'}</h1>
	{/snippet}
</GalleryStudio>
