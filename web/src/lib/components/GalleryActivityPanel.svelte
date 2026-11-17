<script lang="ts">
	import { onMount } from 'svelte';
	import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import ActivityFilters from '#lib/components/ActivityFilters.svelte';
	import ActivityTimeline from '#lib/components/ActivityTimeline.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listGalleryContacts } from '#lib/gallery/contacts.js';

	let { galleryId }: { galleryId: string } = $props();

	const events = getLiveEvents();
	// The studio shows one gallery for the panel's whole life; a different gallery remounts the page.
	// svelte-ignore state_referenced_locally
	const feed = new ActivityFeed({ galleryId });

	let contacts = $state<{ id: string; name: string }[]>([]);

	const filtered = $derived(Object.keys(feed.filters).length > 0);

	onMount(() => {
		const stop = feed.start(events);

		void feed.load();
		// The visitor choices are a convenience, so a failure just leaves "All visitors".
		void listGalleryContacts(galleryId)
			.then((linked) => (contacts = linked))
			.catch(() => {});

		return stop;
	});
</script>

<ActivityFilters {feed} {contacts} />
<ActivityTimeline {feed} {filtered} showGallery={false} />
