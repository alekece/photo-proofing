<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import EmptyState from './EmptyState.svelte';

	const { Story } = defineMeta({ title: 'UI/EmptyState', component: EmptyState });
</script>

<Story name="Plain" asChild>
	<EmptyState variant="plain">
		<h2>{m.favorites_empty()}</h2>
		<p>{m.favorites_empty_body()}</p>
	</EmptyState>
</Story>

<Story name="Drop target" asChild>
	<EmptyState>{m.photos_drop()}</EmptyState>
</Story>
