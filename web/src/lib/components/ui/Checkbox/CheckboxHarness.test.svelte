<script lang="ts">
	import Checkbox from './Checkbox.svelte';

	let checked = $state(false);
</script>

<Checkbox aria-label="Mark" bind:checked />
<output data-testid="checked">{checked}</output>
