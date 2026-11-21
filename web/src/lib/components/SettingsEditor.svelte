<script lang="ts">
	import type { Snippet } from 'svelte';
	import SamplePhoto from './SamplePhoto.svelte';

	let {
		label,
		toolbar,
		preview,
		overlay
	}: {
		label: string;
		toolbar: Snippet;
		preview: Snippet;
		overlay?: Snippet;
	} = $props();
</script>

<div class="grid w-full max-w-[48rem] min-w-0 gap-3">
	<div
		class="flex flex-wrap items-start gap-x-4 gap-y-3 rounded-card border border-border bg-panel p-3"
	>
		{@render toolbar()}
	</div>
	<div class="relative">
		<SamplePhoto {label}>
			{@render preview()}
		</SamplePhoto>
		{@render overlay?.()}
	</div>
</div>
