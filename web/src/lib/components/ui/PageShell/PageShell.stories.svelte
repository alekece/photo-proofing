<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Card from '../Card/Card.svelte';
	import PageShell from './PageShell.svelte';

	const { Story } = defineMeta({
		title: 'UI/PageShell',
		component: PageShell,
		parameters: { layout: 'fullscreen' }
	});
</script>

{#snippet shell(width: 'narrow' | 'default' | 'wide')}
	<PageShell {width}>
		<h1>{m.dashboard_head()}</h1>
		<Card class="p-4">{m.dashboard_empty_body()}</Card>
	</PageShell>
{/snippet}

<Story name="Narrow" asChild>{@render shell('narrow')}</Story>
<Story name="Default" asChild>{@render shell('default')}</Story>
<Story name="Wide" asChild>{@render shell('wide')}</Story>
