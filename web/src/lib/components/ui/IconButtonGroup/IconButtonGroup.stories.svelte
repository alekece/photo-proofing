<script module lang="ts">
	import { Columns2, Grid2X2 } from '@lucide/svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import IconButtonGroup from './IconButtonGroup.svelte';

	const { Story } = defineMeta({ title: 'UI/IconButtonGroup', component: IconButtonGroup });
</script>

{#snippet group(disabled: boolean)}
	<IconButtonGroup
		label={m.settings_layout()}
		value="grid"
		items={[
			{ id: 'grid', label: m.settings_layout_grid(), icon: Grid2X2 },
			{ id: 'masonry', label: m.settings_layout_masonry(), icon: Columns2, disabled }
		]}
	/>
{/snippet}

<Story name="Layout picker" asChild>{@render group(false)}</Story>
<Story name="With unavailable option" asChild>{@render group(true)}</Story>
