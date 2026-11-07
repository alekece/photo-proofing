<script lang="ts">
	import { setLiveEvents, type LiveEvents } from '#lib/events.js';
	import { setUploadQueue, type UploadQueue } from '#lib/gallery/uploads.svelte.js';
	import type { Gallery } from '#lib/gallery/api.js';
	import GalleryTabs from './GalleryTabs.svelte';
	let { gallery, queue, events }: { gallery: Gallery; queue: UploadQueue; events: LiveEvents } =
		$props();
	// svelte-ignore state_referenced_locally
	setUploadQueue(queue);
	// svelte-ignore state_referenced_locally
	setLiveEvents(events);
</script>

<GalleryTabs
	galleryId={gallery.id}
	included={gallery.included_photos}
	layout={gallery.layout}
	shared={gallery.share_token !== null}
/>
