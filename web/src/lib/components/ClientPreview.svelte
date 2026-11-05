<script lang="ts">
	import { onMount } from 'svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import { listPhotos } from '#lib/gallery/photos.js';
	import { toOwnerProofs } from '#lib/gallery/proofs.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { cn } from '#lib/utils.js';
	import ClientGallery from './ClientGallery.svelte';
	import Button from './ui/button/button.svelte';

	/** Read-only, full-screen view of the client page for the signed-in owner; no link is needed. */
	let { gallery, onclose }: { gallery: Gallery; onclose: () => void } = $props();

	const VIEWS = [
		{ id: 'desktop', label: 'Desktop view', icon: '🖥' },
		{ id: 'mobile', label: 'Mobile view', icon: '📱' }
	] as const;

	let view = $state<'desktop' | 'mobile'>('desktop');
	let proofs = $state<Proof[]>([]);
	let favorites = $state<string[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let closeButton = $state<HTMLButtonElement | null>(null);

	onMount(() => {
		closeButton?.focus();
		let active = true;

		listPhotos(gallery.id)
			.then((photos) => {
				if (!active) return;

				proofs = toOwnerProofs(gallery.id, photos);
				favorites = photos.filter(({ favorite }) => favorite).map(({ id }) => id);
			})
			.catch(() => active && (failed = true))
			.finally(() => active && (loaded = true));

		return () => (active = false);
	});

	// Native modals trap focus in browsers; jsdom and the viewer's own dialog need this too.
	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLDialogElement }) {
		const dialog = event.currentTarget;

		if (event.key !== 'Tab' || (event.target as Element).closest('dialog') !== dialog) return;

		const items = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled)')].filter(
			(item) => item.closest('dialog') === dialog
		);
		const first = items[0];
		const last = items[items.length - 1];
		const active = document.activeElement;

		if (event.shiftKey && active === first) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && active === last) {
			event.preventDefault();
			first.focus();
		}
	}
</script>

<Dialog
	{onclose}
	{onkeydown}
	label="Client preview"
	aria-modal="true"
	class="m-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-background p-0 text-foreground open:flex open:flex-col"
>
	{#snippet children(close)}
		<header class="flex flex-wrap items-center justify-between gap-2 border-b border-border p-3">
			<span class="font-medium">Client preview</span>
			<div class="flex gap-2">
				{#each VIEWS as item (item.id)}
					<Button
						variant={view === item.id ? 'default' : 'outline'}
						size="sm"
						aria-label={item.label}
						aria-pressed={view === item.id}
						onclick={() => (view = item.id)}
					>
						<span aria-hidden="true">{item.icon}</span>
					</Button>
				{/each}
			</div>
			<Button
				variant="outline"
				size="sm"
				bind:ref={closeButton}
				aria-label="Close preview"
				onclick={close}>Close</Button
			>
		</header>
		<div class="bg-muted min-h-0 flex-1 overflow-auto">
			<div
				data-frame={view}
				class={cn(
					'relative bg-background',
					view === 'mobile' && 'mx-auto my-4 h-[min(100%,52rem)] w-[390px] max-w-full border'
				)}
			>
				<div class={cn(view === 'mobile' && 'h-full overflow-y-auto')}>
					{#if failed}
						<p role="alert" class="p-4 text-danger">
							Couldn’t load the photos. Close and try again.
						</p>
					{:else if loaded}
						<ClientGallery
							gallery={{ ...gallery, delivered: gallery.states.includes('delivered') }}
							{proofs}
							{favorites}
							preview
							contained={view === 'mobile'}
						/>
					{/if}
				</div>
			</div>
		</div>
	{/snippet}
</Dialog>
