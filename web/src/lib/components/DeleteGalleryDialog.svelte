<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import PhotoStack from '#lib/components/ui/PhotoStack/PhotoStack.svelte';
	import { listPhotos } from '#lib/gallery/photos.js';

	let {
		galleryId,
		load = listPhotos,
		onconfirm,
		onclose
	}: {
		galleryId: string;
		load?: typeof listPhotos;
		onconfirm: () => void | Promise<void>;
		onclose: () => void;
	} = $props();

	let ids = $state<string[]>([]);

	onMount(() => {
		// The picture is decoration: a failed load just leaves the empty frame.
		load(galleryId)
			.then((list) => (ids = list.filter(({ status }) => status === 'ready').map(({ id }) => id)))
			.catch(() => {});
	});
</script>

<ConfirmDialog
	title={m.delete_title()}
	confirmLabel={m.action_delete()}
	destructive
	{onconfirm}
	{onclose}
>
	{#snippet visual()}
		<PhotoStack {galleryId} photoIds={ids} />
	{/snippet}
	{#snippet description()}
		<p class="text-sm text-muted-foreground">{m.delete_body()}</p>
	{/snippet}
</ConfirmDialog>
