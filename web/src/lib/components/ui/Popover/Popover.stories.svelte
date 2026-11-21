<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import Popover from './Popover.svelte';

	const { Story } = defineMeta({ title: 'UI/Popover', component: Popover });
</script>

{#snippet preview(disabled: boolean)}
	<Popover {disabled} panelClass="min-w-40 p-3">
		{#snippet trigger()}{m.toolbar_filters()}{/snippet}
		{#snippet children({ close })}
			<Button variant="ghost" onclick={close}>{m.action_close()}</Button>
		{/snippet}
	</Popover>
{/snippet}

<Story name="Interactive" asChild>{@render preview(false)}</Story>
<Story name="Disabled" asChild>{@render preview(true)}</Story>
