<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import Radio from './Radio.svelte';

	const { Story } = defineMeta({ title: 'UI/Radio', component: Radio });
</script>

<Story name="Layout options" asChild>
	<fieldset class="grid gap-2">
		<legend>{m.settings_layout()}</legend>
		<Label class="flex items-center gap-2">
			<Radio name="layout" value="grid" checked />
			{m.settings_layout_grid()}
		</Label>
		<Label class="flex items-center gap-2">
			<Radio name="layout" value="masonry" />
			{m.settings_layout_masonry()}
		</Label>
	</fieldset>
</Story>

<Story name="Disabled" asChild>
	<Label class="flex items-center gap-2">
		<Radio checked disabled />
		{m.settings_layout_grid()}
	</Label>
</Story>
