<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { formatFullDate } from '#lib/gallery/format.js';

	let { expiresAt }: { expiresAt: string } = $props();

	// Expiry is the end of a day in the photographer's time zone. The server cannot know the
	// viewer's zone, so it renders UTC and the browser switches to its own after mount.
	let timeZone = $state<string | undefined>('UTC');
	onMount(() => (timeZone = undefined));

	const formatted = $derived(formatFullDate(expiresAt, timeZone));
</script>

<p class="text-[0.9rem] opacity-85">
	{m.expiry_open_until()} <time datetime={expiresAt}>{formatted}</time>
</p>
