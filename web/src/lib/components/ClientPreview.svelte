<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Eye, Monitor, Smartphone } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import { listClientPreviewPhotos, listPhotos, previewUrl } from '#lib/gallery/photos.js';
	import { toClientPreviewProofs } from '#lib/gallery/proofs.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { parseSettings, type WorkspaceSettings } from '#lib/settings/settings.js';
	import { cn } from '#lib/utils.js';
	import ClientGallery from './ClientGallery.svelte';

	/** Read-only, full-screen view of the client page for the signed-in owner; no link is needed. */
	let { gallery, onclose }: { gallery: Gallery; onclose: () => void } = $props();

	const VIEWS = [
		{ id: 'desktop', label: m.preview_desktop(), icon: Monitor },
		{ id: 'phone', label: m.preview_phone(), icon: Smartphone }
	] as const;

	let view = $state<string>('desktop');
	let proofs = $state<Proof[]>([]);
	let favorites = $state<string[]>([]);
	let settings = $state<WorkspaceSettings | null>(null);
	let loaded = $state(false);
	let failed = $state(false);
	let closeButton = $state<HTMLButtonElement | null>(null);

	onMount(() => {
		closeButton?.focus();
	});

	$effect(() => {
		const id = gallery.id;
		const watermarked = gallery.watermark_enabled;
		let active = true;
		loaded = false;
		failed = false;

		Promise.all([
			listPhotos(id),
			listClientPreviewPhotos(id, watermarked),
			fetch('/api/v1/settings').then(async (response) => {
				if (!response.ok) throw new Error('Couldn’t load studio settings.');
				const parsed = parseSettings(await response.json());
				if (!parsed) throw new Error('Unexpected studio settings.');
				return parsed;
			})
		])
			.then(([photos, visible, studio]) => {
				if (!active) return;

				proofs = toClientPreviewProofs(id, visible, watermarked);
				settings = studio;
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
	label={m.preview_label()}
	aria-modal="true"
	class="m-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-[#1d1815] p-0 text-foreground open:flex open:flex-col"
>
	{#snippet children(close)}
		<header
			class="grid grid-cols-[1fr_auto_1fr] items-center gap-3.5 border-b border-[#3b322b] bg-foreground px-4 py-2.5 text-sm font-medium text-[#efe6da] max-[700px]:grid-cols-[auto_1fr_auto] max-[700px]:gap-2 max-[700px]:px-2.5 max-[700px]:py-2"
		>
			<span class="flex items-center gap-2 font-semibold">
				<Eye size={18} aria-hidden="true" /><span class="max-[700px]:hidden"
					>{m.action_preview()}</span
				>
			</span>
			<div
				role="group"
				aria-label={m.preview_size()}
				class="flex justify-self-center rounded-full bg-[#3a302a] p-[3px]"
			>
				{#each VIEWS as { id, label, icon: Icon } (id)}
					<button
						type="button"
						aria-label={label}
						aria-pressed={view === id}
						onclick={() => (view = id)}
						class="flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[0.8125rem] font-semibold focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus {view ===
						id
							? 'bg-panel text-foreground'
							: 'text-[#cdbfae]'}"
					>
						<Icon size={16} aria-hidden="true" /><span class="max-[700px]:hidden">{label}</span>
					</button>
				{/each}
			</div>
			<button
				type="button"
				bind:this={closeButton}
				aria-label={m.preview_close()}
				onclick={close}
				class="cursor-pointer justify-self-end rounded-full border border-[#4a3f37] px-3.5 py-[7px] text-[0.8125rem] font-semibold focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
			>
				<span aria-hidden="true">✕</span>
				{m.action_close()}
			</button>
		</header>
		<div
			class={cn(
				'min-h-0 flex-1',
				view === 'phone'
					? 'grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_50%_40%,#3a302a,#1d1815_70%)] p-6'
					: 'overflow-auto bg-background'
			)}
		>
			<div
				data-frame={view}
				class={cn(
					view === 'phone' &&
						'h-[min(680px,calc(100dvh-110px))] w-[340px] max-w-full rounded-[40px] bg-black p-2.5 shadow-[0_30px_80px_rgb(0_0_0/0.5)]'
				)}
			>
				<div
					class={cn(
						'relative bg-background',
						view === 'phone' &&
							'h-full translate-z-0 [scrollbar-width:none] overflow-auto rounded-[30px]'
					)}
				>
					{#if failed}
						<p role="alert" class="p-4 text-danger">
							{m.preview_load_failed()}
						</p>
					{:else if loaded}
						<ClientGallery
							gallery={{ ...gallery, delivered: gallery.states.includes('delivered') }}
							coverUrl={gallery.hero_id ? previewUrl(gallery.id, gallery.hero_id) : undefined}
							{proofs}
							{favorites}
							studio={settings?.studioName}
							studioLogo={settings?.logo}
							signature={settings?.signature}
							preview
							framed={view === 'phone'}
						/>
					{/if}
				</div>
			</div>
		</div>
	{/snippet}
</Dialog>
