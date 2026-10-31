<script lang="ts">
	import Tabs from './tabs.svelte';

	const items = [
		{ id: 'a', label: 'Alpha' },
		{ id: 'b', label: 'Beta' },
		{ id: 'c', label: 'Gamma' }
	];
	let value = $state('a');
</script>

<Tabs bind:value {items} label="Sections">
	{#snippet panel(id)}
		<p>Content {id}</p>
	{/snippet}
</Tabs>
<output data-testid="value">{value}</output>
