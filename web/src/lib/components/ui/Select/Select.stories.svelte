<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import Select from './Select.svelte';

	const { Story } = defineMeta({ title: 'UI/Select', component: Select });
</script>

{#snippet field(disabled: boolean)}
	<div class="grid max-w-xs gap-2">
		<Label for="sort">{m.sort_by()}</Label>
		<Select id="sort" value="updated" {disabled}>
			<option value="updated">{m.sort_updated()}</option>
			<option value="created">{m.sort_created()}</option>
			<option value="title">{m.sort_title()}</option>
		</Select>
	</div>
{/snippet}

<Story name="Sort options" asChild>{@render field(false)}</Story>
<Story name="Disabled" asChild>{@render field(true)}</Story>
