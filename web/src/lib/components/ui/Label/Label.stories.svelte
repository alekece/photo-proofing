<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Input from '../Input/Input.svelte';
	import Label from './Label.svelte';

	const { Story } = defineMeta({ title: 'UI/Label', component: Label });
</script>

{#snippet field(srOnly: boolean)}
	<div class="grid max-w-xs gap-2">
		<Label for="search-galleries" {srOnly}>{m.dashboard_search()}</Label>
		<Input id="search-galleries" type="search" placeholder={m.dashboard_search()} />
	</div>
{/snippet}

<Story name="Visible" asChild>{@render field(false)}</Story>
<Story name="Screen reader only" asChild>{@render field(true)}</Story>
