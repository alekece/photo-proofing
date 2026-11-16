<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { formatFullDate } from '#lib/gallery/format.js';

	let { expiresAt }: { expiresAt: string } = $props();

	const formatted = $derived(formatFullDate(expiresAt));
</script>

<p class="text-[0.9rem] opacity-85">
	{m.expiry_open_until()} <time datetime={expiresAt}>{formatted}</time>
</p>
