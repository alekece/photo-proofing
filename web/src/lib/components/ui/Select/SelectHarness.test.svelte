<script lang="ts">
	import Select from './Select.svelte';

	let value = $state('a');
</script>

<Select aria-label="Pick" bind:value>
	<option value="a">Alpha</option>
	<option value="b">Beta</option>
</Select>
<output data-testid="value">{value}</output>
