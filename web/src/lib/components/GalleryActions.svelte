<script lang="ts">
	import { goto } from '$app/navigation';
	import Button from '#lib/components/ui/button/button.svelte';
	import {
		archiveGallery,
		deleteGallery,
		GalleryApiError,
		restoreGallery,
		type Gallery
	} from '#lib/gallery/api.js';

	/** `onchange` receives the gallery after it was archived or restored. */
	let { gallery, onchange }: { gallery: Gallery; onchange: (gallery: Gallery) => void } = $props();

	let busy = $state(false);
	let failure = $state('');
	let archived = $derived(gallery.states.includes('archived'));

	async function run(action: () => Promise<void>, message: string) {
		busy = true;
		failure = '';

		try {
			await action();
		} catch (error) {
			if (error instanceof GalleryApiError && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });

				return;
			}

			failure = message;
		} finally {
			busy = false;
		}
	}
	function toggleArchive() {
		return run(
			async () => onchange(await (archived ? restoreGallery : archiveGallery)(gallery.id)),
			archived ? 'Couldn’t restore the gallery.' : 'Couldn’t archive the gallery.'
		);
	}
	function remove() {
		return run(async () => {
			await deleteGallery(gallery.id);
			await goto('/dashboard', { invalidateAll: true });
		}, 'Couldn’t delete the gallery.');
	}
</script>

<div class="grid gap-2">
	<Button variant="outline" disabled={busy} onclick={toggleArchive}>
		{archived ? 'Restore' : 'Archive'}
	</Button>
	<Button variant="outline" disabled={busy} onclick={remove}>Delete</Button>
	{#if failure}<p class="text-sm text-danger" role="alert">{failure}</p>{/if}
</div>
