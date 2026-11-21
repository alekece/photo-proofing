<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import SelectActions from './SelectActions.svelte';

	const { Story } = defineMeta({ title: 'UI/SelectActions', component: SelectActions });
</script>

<script lang="ts">
	const total = 5;
	let count = $state(2);
	let active = $state(true);
</script>

<Story name="Selection mode" asChild>
	<Button disabled={active} onclick={() => (active = true)}>{m.action_select()}</Button>
	{#if active}
		<SelectActions
			{count}
			{total}
			rail="dashboard"
			onselectall={() => (count = total)}
			onunselectall={() => (count = 0)}
			onexit={() => (active = false)}
			canEscape={() => true}
		>
			{#snippet actions()}
				<Button variant="outline" disabled={!count} onclick={() => (count = 0)}>
					{m.action_archive()}
				</Button>
			{/snippet}
		</SelectActions>
	{/if}
</Story>
