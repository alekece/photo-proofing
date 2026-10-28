<script lang="ts">
	let { expiresAt, now = new Date() }: { expiresAt: string; now?: Date } = $props();

	const expiry = $derived(new Date(expiresAt));
	const days = $derived(Math.ceil((expiry.getTime() - now.getTime()) / 86_400_000));
	const formatted = $derived(
		new Intl.DateTimeFormat('en', { dateStyle: 'long', timeZone: 'UTC' }).format(expiry)
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
