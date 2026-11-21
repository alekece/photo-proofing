<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Card from './Card.svelte';

	const { Story } = defineMeta({ title: 'UI/Card', component: Card });
</script>

<Story name="With content" asChild>
	<Card as="section" class="max-w-sm p-4">
		<h2 class="font-semibold">{m.favorites_empty()}</h2>
		<p class="text-muted-foreground">{m.favorites_empty_body()}</p>
	</Card>
</Story>
