<script module lang="ts">
	import { ArrowDownUp } from '@lucide/svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import ToolbarMenu from './ToolbarMenu.svelte';

	const { Story } = defineMeta({ title: 'UI/ToolbarMenu', component: ToolbarMenu });
</script>

<script lang="ts">
	let value = $state('updated');

	const options = $derived([
		{ value: 'updated', label: m.sort_updated() },
		{ value: 'created', label: m.sort_created() },
		{ value: 'title', label: m.sort_title() }
	]);
</script>

{#snippet menu(disabled: boolean)}
	<ToolbarMenu
		{disabled}
		{value}
		{options}
		icon={ArrowDownUp}
		label={m.sort_by()}
		summary={options.find((option) => option.value === value)?.label ?? m.sort_by()}
		onchange={(next) => (value = next)}
	/>
{/snippet}

<Story name="Sort menu" asChild>{@render menu(false)}</Story>
<Story name="Disabled" asChild>{@render menu(true)}</Story>
