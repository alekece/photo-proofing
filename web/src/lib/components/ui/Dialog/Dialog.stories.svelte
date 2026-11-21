<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import Dialog from './Dialog.svelte';

	const { Story } = defineMeta({ title: 'UI/Dialog', component: Dialog });
</script>

<script lang="ts">
	let open = $state(false);
</script>

{#snippet preview(modal: boolean)}
	<Button onclick={() => (open = true)}>{m.action_preview()}</Button>
	{#if open}
		<Dialog {modal} label={m.action_preview()} onclose={() => (open = false)}>
			{#snippet children(close)}
				<div class="grid gap-4 p-4">
					<h2>{m.gallery_untitled()}</h2>
					<Button onclick={close}>{m.action_close()}</Button>
				</div>
			{/snippet}
		</Dialog>
	{/if}
{/snippet}

<Story name="Modal" asChild>{@render preview(true)}</Story>
<Story name="Non-modal" asChild>{@render preview(false)}</Story>
