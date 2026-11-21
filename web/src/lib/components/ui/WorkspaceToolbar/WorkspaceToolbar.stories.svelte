<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import WorkspaceToolbar from './WorkspaceToolbar.svelte';

	const { Story } = defineMeta({ title: 'UI/WorkspaceToolbar', component: WorkspaceToolbar });
</script>

<script lang="ts">
	let value = $state('');
</script>

<Story name="Gallery search" asChild>
	<WorkspaceToolbar
		search={{ label: m.dashboard_search(), value, oninput: (next) => (value = next) }}
		onclear={value ? () => (value = '') : undefined}
	>
		{#snippet actions()}
			<Button variant="outline">{m.dashboard_new_gallery()}</Button>
		{/snippet}
	</WorkspaceToolbar>
</Story>
