<script lang="ts">
	import Checkbox from './checkbox.svelte';

	let checked = $state(false);
</script>

<Checkbox aria-label="Mark" bind:checked />
<output data-testid="checked">{checked}</output>
