<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import ConfirmDialog from './ConfirmDialog.svelte';

	const { Story } = defineMeta({ title: 'UI/ConfirmDialog', component: ConfirmDialog });
</script>

<script lang="ts">
	let open = $state(false);
</script>

<Story name="Destructive confirmation" asChild>
	<Button variant="destructive" onclick={() => (open = true)}>{m.action_delete()}</Button>
	{#if open}
		<ConfirmDialog
			title={m.delete_title()}
			confirmLabel={m.action_delete()}
			destructive
			onconfirm={() => {}}
			onclose={() => (open = false)}
		>
			{#snippet description()}
				<p>{m.delete_body()}</p>
			{/snippet}
		</ConfirmDialog>
	{/if}
</Story>
