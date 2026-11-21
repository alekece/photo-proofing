<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from '../Button/Button.svelte';
	import Toaster from './Toaster.svelte';
	import { toasts } from './toasts.svelte.js';

	const { Story } = defineMeta({ title: 'UI/Toast', component: Toaster });
</script>

<script lang="ts">
	import { onDestroy } from 'svelte';

	const toastIds = new Set<number>();
	const showSuccess = () => toastIds.add(toasts.success(m.studio_saved()));

	onDestroy(() => {
		for (const id of toastIds) toasts.dismiss(id);
	});
</script>

<Story name="Feedback" asChild>
	<div class="flex flex-wrap gap-2">
		<Button onclick={showSuccess}>{m.studio_saved()}</Button>
		<Button
			variant="outline"
			onclick={() => toastIds.add(toasts.error(m.gallery_not_saved(), showSuccess))}
		>
			{m.gallery_not_saved()}
		</Button>
	</div>
	<Toaster />
</Story>
