<script lang="ts">
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
	title="Delete this gallery?"
	confirmLabel="Delete"
	cancelLabel="Keep it"
	destructive
	{onconfirm}
	{onclose}
>
	{#snippet visual()}
		<PhotoStack {galleryId} photoIds={ids} />
	{/snippet}
	{#snippet description()}
		<p class="text-sm text-muted-foreground">Its photos, favorites and client link go for good.</p>
	{/snippet}
</ConfirmDialog>
