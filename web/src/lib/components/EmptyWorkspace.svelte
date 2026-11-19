<script lang="ts">
	import { Images } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
</script>

<section class="px-3.5 pb-10 sm:px-8 sm:pb-14">
	<EmptyState variant="plain">
		<Images size={32} aria-hidden="true" class="text-muted-foreground" />
		<h2 class="font-serif text-[1.625rem] text-foreground sm:text-[1.75rem]">
			{m.dashboard_empty()}
		</h2>
		<p class="text-sm">{m.dashboard_empty_body()}</p>
	</EmptyState>
</section>
