<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import Button from '#lib/components/ui/button/button.svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import { createGallery, GalleryApiError, updateGallery } from '#lib/gallery/api.js';
	import { TITLE_MAX } from '#lib/gallery/format.js';

	let {
		onclose,
		create = createGallery,
		update = updateGallery
	}: {
		onclose: () => void;
		create?: typeof createGallery;
		update?: typeof updateGallery;
	} = $props();

	let field = $state<HTMLInputElement | null>(null);
	let title = $state('');
	let error = $state('');
	let creating = $state(false);

	onMount(() => field?.focus());

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (creating) return;

		const trimmed = title.trim();

		if (trimmed === '') {
			error = 'Enter a title for the gallery.';

			return;
		}
		if (trimmed.length > TITLE_MAX) {
			error = `Titles can be up to ${TITLE_MAX} characters.`;

			return;
		}

		creating = true;
		error = '';

		try {
			const gallery = await create();

			// The gallery exists even if naming it fails; the title can be set on its own page.
			await update(gallery.id, { title: trimmed }).catch(() => undefined);
			await goto(`/dashboard/${gallery.id}`);
		} catch (cause) {
			if (cause instanceof GalleryApiError && cause.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });

				return;
			}

			error = 'Couldn’t create the gallery. Please try again.';
		} finally {
			creating = false;
		}
	}
</script>

<Dialog
	{onclose}
	labelledby="new-gallery-title"
	class="m-auto w-[min(24rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
>
	{#snippet children(close)}
		<form onsubmit={submit} class="grid gap-4" aria-busy={creating}>
			<div class="grid gap-1">
				<h2 id="new-gallery-title" class="text-xl">New gallery</h2>
				<p class="text-sm text-muted-foreground">
					Give it a title. You can change it any time, and nothing is shared until you say so.
				</p>
			</div>
			<div class="grid gap-2">
				<Label for="new-gallery-field" class="font-normal">Title</Label>
				<Input
					bind:ref={field}
					id="new-gallery-field"
					bind:value={title}
					placeholder="e.g. Martin wedding"
					aria-describedby={error ? 'new-gallery-error' : undefined}
					aria-invalid={error ? true : undefined}
					disabled={creating}
				/>
				{#if error}
					<p id="new-gallery-error" role="alert" class="text-sm text-danger">{error}</p>
				{/if}
			</div>
			<div class="flex justify-end gap-2">
				<Button variant="ghost" onclick={close} disabled={creating}>Cancel</Button>
				<Button type="submit" disabled={creating}
					>{creating ? 'Creating…' : 'Create gallery'}</Button
				>
			</div>
		</form>
	{/snippet}
</Dialog>
