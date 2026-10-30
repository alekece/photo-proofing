<script lang="ts">
	import { onMount } from 'svelte';

	let { expiresAt, now = new Date() }: { expiresAt: string; now?: Date } = $props();

	// Expiry is the end of a day in the photographer's time zone. The server cannot know the
	// viewer's zone, so it renders UTC and the browser switches to its own after mount.
	let timeZone = $state<string | undefined>('UTC');
	onMount(() => (timeZone = undefined));

	const expiry = $derived(new Date(expiresAt));
	const days = $derived(Math.ceil((expiry.getTime() - now.getTime()) / 86_400_000));
	const formatted = $derived(
		new Intl.DateTimeFormat('en', { dateStyle: 'long', timeZone }).format(expiry)
	);
	const remaining = $derived(
		days <= 0 ? 'expires today' : days === 1 ? '1 day left' : `${days} days left`
	);
</script>

<p class="expiry" class:soon={days <= 7}>
	Access until <time datetime={expiresAt}>{formatted}</time> ({remaining})
</p>

<style>
	.expiry {
		margin: 0;
		font-size: 0.9rem;
		opacity: 0.85;
	}
	.soon {
		font-weight: 600;
		opacity: 1;
	}
</style>
