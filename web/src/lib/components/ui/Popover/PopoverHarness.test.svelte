<script lang="ts">
	import Popover from './Popover.svelte';

	let {
		disabled = false,
		placement = 'below'
	}: {
		disabled?: boolean;
		placement?: 'below' | 'above' | 'right' | 'responsive';
	} = $props();
</script>

<p>Outside</p>
<Popover {disabled} {placement}>
	{#snippet trigger()}Open{/snippet}
	<p>Inside</p>
	<button type="button">Item</button>
</Popover>

<button type="button">After</button>
