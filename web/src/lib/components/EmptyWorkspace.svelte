<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Images, Plus } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';

	let { oncreate }: { oncreate: () => void } = $props();
</script>

<section class="px-3.5 pb-10 sm:px-8 sm:pb-14">
	<EmptyState>
		<span class="grid size-14 place-items-center rounded-full bg-background"
			><Images size={26} aria-hidden="true" /></span
		>
		<h2 class="font-serif text-[1.625rem] text-foreground sm:text-[1.75rem]">
			{m.dashboard_empty()}
		</h2>
		<Button class="gap-1.5" onclick={oncreate}
			><Plus size={16} aria-hidden="true" />{m.new_gallery_title()}</Button
		>
	</EmptyState>
</section>
