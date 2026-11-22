<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import Checkbox from './Checkbox.svelte';

	const { Story } = defineMeta({ title: 'UI/Checkbox', component: Checkbox });
</script>

{#snippet field(checked: boolean, disabled: boolean)}
	<div class="flex items-center gap-2">
		<Checkbox id="watermark" {checked} {disabled} />
		<Label for="watermark">{m.settings_watermark()}</Label>
	</div>
{/snippet}

<Story name="Unchecked" asChild>{@render field(false, false)}</Story>
<Story name="Checked" asChild>{@render field(true, false)}</Story>
<Story name="Disabled" asChild>{@render field(false, true)}</Story>
