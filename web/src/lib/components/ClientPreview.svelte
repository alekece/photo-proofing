<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Eye, Maximize2 } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { loadClientPreviewData, type PreviewViewport } from '#lib/gallery/preview.js';
	import type { Proof } from '#lib/gallery/types.js';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';
	import ClientGallery from './ClientGallery.svelte';
	import PreviewFrame from './PreviewFrame.svelte';

	/**
	 * Read-only view of the client page for the signed-in owner; no link is needed.
	 *
	 * `modal` (the default) is a full-screen dialog with background scroll lock and focus return,
	 * for small screens or an explicit "Open full preview" action. `inline` embeds the same stage
	 * without a dialog, fixed height or its own scrollbar, so the host page owns the only scroll;
	 * it offers "Open full preview" instead of Close, through `onexpand`.
	 */
	let {
		gallery,
		settings = DEFAULT_SETTINGS,
		heroRevision,
		variant = 'modal',
		onclose,
		onexpand
	}: {
		gallery: Gallery;
		settings?: WorkspaceSettings;
		heroRevision?: number;
		variant?: 'modal' | 'inline';
		onclose?: () => void;
		onexpand?: () => void;
	} = $props();

	let view = $state<PreviewViewport>('desktop');
	let proofs = $state<Proof[]>([]);
	let favorites = $state<string[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let closeButton = $state<HTMLButtonElement | null>(null);

	onMount(() => {
		if (variant === 'modal') closeButton?.focus();
	});

	const delivered = $derived(gallery.states.includes('delivered'));

	// Once delivered the client sees the finals, without the favourites made on the proofs.
	$effect(() => {
		const id = gallery.id;
		const watermarked = gallery.watermark_enabled;
		const finals = delivered;
		let active = true;
		loaded = false;
		failed = false;

		loadClientPreviewData(id, { delivered: finals, watermarked })
			.then((data) => {
				if (!active) return;

				proofs = data.proofs;
				favorites = data.favorites;
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

{#snippet stage()}
	{#if failed}
		<p role="alert" class="p-4 text-danger">
			{m.preview_load_failed()}
		</p>
	{:else if loaded}
		<ClientGallery
			gallery={{ ...gallery, delivered }}
			coverUrl={gallery.hero_id
				? previewUrl(gallery.id, gallery.hero_id, 'photos', heroRevision)
				: undefined}
			{proofs}
			{favorites}
			studio={settings.studioName}
			studioLogo={settings.logo}
			signature={settings.signature}
			signatureSize={settings.signatureSize}
			signatureTextColor={settings.signatureTextColor}
			preview
			embedded={variant === 'inline'}
			framed={view === 'phone'}
		/>
	{/if}
{/snippet}

{#if variant === 'inline'}
	<section
		aria-label={m.preview_label()}
		class="flex min-w-0 flex-col overflow-hidden rounded-[0.875rem] border border-border bg-[#1d1815] text-foreground shadow-[0_18px_40px_-24px_rgb(43_36_32/0.45)]"
	>
		<PreviewFrame
			bind:mode={view}
			stageClass="flex-1"
			scrollable={false}
			compactToggle
			toggleClass="justify-self-center"
		>
			{#snippet header(toggle)}
				<header
					class="flex flex-wrap items-center justify-between gap-2.5 border-b border-[#3b322b] bg-foreground px-4 py-2.5 text-sm font-medium text-[#efe6da]"
				>
					<span class="flex items-center gap-2 font-semibold">
						<Eye size={18} aria-hidden="true" /><span class="max-[700px]:hidden">
							{m.action_preview()}
						</span>
					</span>
					{@render toggle()}
					<button
						type="button"
						aria-label={m.preview_open_full()}
						onclick={() => onexpand?.()}
						class="ml-auto flex cursor-pointer items-center gap-1.5 rounded-full border border-[#4a3f37] px-3.5 py-[7px] text-[0.8125rem] font-semibold focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
					>
						<Maximize2 size={14} aria-hidden="true" />
						<span class="max-[700px]:hidden">{m.preview_open_full()}</span>
					</button>
				</header>
			{/snippet}

			{@render stage()}
		</PreviewFrame>
	</section>
{:else}
	<Dialog
		onclose={() => onclose?.()}
		{onkeydown}
		label={m.preview_label()}
		aria-modal="true"
		class="m-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-[#1d1815] p-0 text-foreground open:flex open:flex-col"
	>
		{#snippet children(close)}
			<PreviewFrame bind:mode={view} compactToggle toggleClass="justify-self-center">
				{#snippet header(toggle)}
					<header
						class="grid grid-cols-[1fr_auto_1fr] items-center gap-3.5 border-b border-[#3b322b] bg-foreground px-4 py-2.5 text-sm font-medium text-[#efe6da] max-[700px]:grid-cols-[auto_1fr_auto] max-[700px]:gap-2 max-[700px]:px-2.5 max-[700px]:py-2"
					>
						<span class="flex items-center gap-2 font-semibold">
							<Eye size={18} aria-hidden="true" /><span class="max-[700px]:hidden">
								{m.action_preview()}
							</span>
						</span>
						{@render toggle()}
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
				{/snippet}

				{@render stage()}
			</PreviewFrame>
		{/snippet}
	</Dialog>
{/if}
