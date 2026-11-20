<script lang="ts">
	import ActionButton, { type ActionButtonProps } from './ActionButton.svelte';

	let {
		labels,
		onaction,
		onsettled
	}: Pick<ActionButtonProps, 'labels' | 'onaction' | 'onsettled'> = $props();
	let action: ActionButton;
</script>

<form
	id="edit-contact"
	onsubmit={(event) => {
		event.preventDefault();
		action.activate();
	}}
>
	<label for="name">Name</label>
	<input id="name" name="name" required value="Camille" />
</form>
<ActionButton bind:this={action} {labels} {onaction} {onsettled} form="edit-contact" />
