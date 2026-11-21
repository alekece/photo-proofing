<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import ActionButton from './ActionButton.svelte';

	const { Story } = defineMeta({ title: 'UI/ActionButton', component: ActionButton });
</script>

{#snippet action(disabled: boolean)}
	<ActionButton
		{disabled}
		labels={{ idle: m.studio_save(), pending: m.studio_saving(), succeeded: m.studio_saved() }}
		onaction={async () => {
			await new Promise((resolve) => setTimeout(resolve, 600));
			return true;
		}}
	/>
{/snippet}

<Story name="Save" asChild>{@render action(false)}</Story>
<Story name="Disabled" asChild>{@render action(true)}</Story>
