<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Tabs from './Tabs.svelte';

	const { Story } = defineMeta({ title: 'UI/Tabs', component: Tabs });
</script>

<Story name="Gallery sections" asChild>
	<Tabs
		label={m.dashboard_head()}
		value="favorites"
		items={[
			{ id: 'favorites', label: m.tab_favorites(), count: 0 },
			{ id: 'comments', label: m.tab_comments(), count: 2, locked: true },
			{ id: 'final', label: m.tab_final(), disabled: true, title: m.tab_after_sharing() }
		]}
	>
		{#snippet panel(id)}
			<p class="p-4">{id === 'favorites' ? m.favorites_empty() : m.tab_comments()}</p>
		{/snippet}
	</Tabs>
</Story>
