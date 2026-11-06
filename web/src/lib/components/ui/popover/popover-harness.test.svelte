<script lang="ts">
	import Popover from './popover.svelte';

	let { disabled = false }: { disabled?: boolean } = $props();
</script>

<p>Outside</p>
<Popover {disabled}>
	{#snippet trigger()}Open{/snippet}
	<p>Inside</p>
	<button type="button">Item</button>
</Popover>

<button type="button">After</button>
