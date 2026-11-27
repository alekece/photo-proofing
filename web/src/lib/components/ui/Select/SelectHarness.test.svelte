<script lang="ts">
	import Select from './Select.svelte';

	let value = $state('a');
	const options = [
		{ value: 'a', label: 'Alpha' },
		{ value: 'b', label: 'Beta' }
	];
</script>

<Select label="Pick" bind:value {options} />
<output data-testid="value">{value}</output>
