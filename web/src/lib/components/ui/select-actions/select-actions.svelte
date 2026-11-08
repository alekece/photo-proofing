<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';

	let {
		count,
		onunselectall,
		onleave
	}: {
		count: number;
		onunselectall: () => void;
		onleave: () => void;
	} = $props();
</script>

<div data-slot="select-actions" class="mr-auto flex flex-wrap items-center gap-2">
	<strong class="mr-1 text-sm font-semibold">{count} selected</strong>
	<!-- The one added control; with nothing left to unselect it leaves select mode instead. -->
	<Button
		variant="outline"
		size="sm"
		class="min-h-9!"
		onclick={count === 0 ? onleave : onunselectall}
		>{count === 0 ? 'Cancel' : 'Unselect all'}</Button
	>
</div>
