<script lang="ts">
	import Tabs from './tabs.svelte';

	const defaults = [
		{ id: 'a', label: 'Alpha' },
		{ id: 'b', label: 'Beta' },
		{ id: 'c', label: 'Gamma' }
	];
	let {
		items = defaults
	}: {
		items?: { id: string; label: string; count?: number; accent?: boolean; locked?: boolean }[];
	} = $props();
	let value = $state('a');
</script>

<Tabs bind:value {items} label="Sections">
	{#snippet panel(id)}
		<p>Content {id}</p>
	{/snippet}
</Tabs>
<output data-testid="value">{value}</output>
