<script lang="ts">
	import { onMount, type ComponentProps } from 'svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { ActivityFeed } from '#lib/activityLog/activityFeed.svelte.js';
	import ActivityFilters from '#lib/components/ActivityFilters.svelte';
	import ActivityTimeline from '#lib/components/ActivityTimeline.svelte';
	import { getLiveEvents } from '#lib/events.js';

	let {
		galleryId,
		onopen
	}: { galleryId: string; onopen?: ComponentProps<typeof ActivityTimeline>['onopen'] } = $props();

	const events = getLiveEvents();
	// The studio shows one gallery for the panel's whole life; a different gallery remounts the page.
	// svelte-ignore state_referenced_locally
	const feed = new ActivityFeed({ galleryId });

	let resultsHeight = $state(0);

	const filtered = $derived(Object.keys(feed.filters).length > 0);

	onMount(() => {
		const stop = feed.start(events);

		void feed.load();
		return stop;
	});
</script>

<ActivityFilters {feed} searchLabel={m.activity_search_visitor()} {resultsHeight} />
<ActivityTimeline
	{feed}
	{filtered}
	showGallery={false}
	bind:resultsHeight
	{onopen}
	onclear={() => feed.clearFilters()}
/>
