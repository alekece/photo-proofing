<script lang="ts">
	import { MessageSquare } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	let { count }: { count: number } = $props();
</script>

{#if count > 0}
	<span
		role="img"
		aria-label={m.comments_count({ count })}
		class="pointer-events-none absolute bottom-2 left-2 z-[3] inline-flex items-center gap-1 rounded-full bg-dark/80 px-2 py-1 text-xs font-semibold text-panel"
	>
		<MessageSquare size={13} aria-hidden="true" />{count}
	</span>
{/if}
