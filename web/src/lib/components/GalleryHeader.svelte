<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Eye, ArchiveRestore, Send } from '@lucide/svelte';
	import JourneySteps from '#lib/components/JourneySteps.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import StatusChip from '#lib/components/ui/status-chip/status-chip.svelte';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { galleryInitials, galleryStatus } from '#lib/gallery/format.js';
	import type { Gallery } from '#lib/gallery/api.js';

	/**
	 * The banner over a gallery: hero photo (or a warm stand-in), status, the editable `heading`
	 * field, one meta line, the next main action and the journey steps. Photo and favorite counts
	 * come from the tabs; they stay out of the text until known.
	 */
	let {
		gallery,
		photoCount = null,
		favoriteCount = null,
		heading,
		onpreview,
		onshare,
		ondeliver,
		onrestore
	}: {
		gallery: Gallery;
		photoCount?: number | null;
		favoriteCount?: number | null;
		heading: Snippet;
		onpreview: () => void;
		onshare: () => void;
		ondeliver: () => void;
		onrestore: () => void;
	} = $props();

	const status = $derived(galleryStatus(gallery.states));
	const shared = $derived(gallery.share_token !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');
	const created = $derived(
		new Date(gallery.created_at).toLocaleDateString('en', { day: 'numeric', month: 'short' })
	);
	const createdLong = $derived(
		new Date(gallery.created_at).toLocaleDateString('en', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		})
	);
	const meta = $derived(
		photoCount === 0 && !shared
			? `Created ${created} · no photos yet`
			: [
					photoCount === null ? '' : `${photoCount} ${photoCount === 1 ? 'photo' : 'photos'}`,
					delivered ? 'delivered' : shared ? 'shared' : `created ${created}`
				]
					.filter(Boolean)
					.join(' · ')
	);
	const photos = $derived(photoCount ?? 0);
	const favorites = $derived(favoriteCount ?? 0);
	const steps = $derived([
		{
			id: 'upload',
			label: 'Upload',
			done: photos > 0,
			fact: photos > 0 ? `${photos} ${photos === 1 ? 'photo' : 'photos'}` : 'Add your proofs'
		},
		{ id: 'share', label: 'Share', done: shared, fact: shared ? 'Shared' : 'Copy the link' },
		{
			id: 'collect',
			label: 'Collect favorites',
			done: delivered,
			fact: shared ? `${favorites} selected` : 'Client picks',
			accent: gallery.included_photos !== null && favorites > gallery.included_photos
		},
		{
			id: 'deliver',
			label: 'Deliver',
			done: delivered,
			fact: delivered ? 'Delivered' : 'Upload finals, then deliver'
		}
	]);
</script>

<div
	data-slot="gallery-header"
	class="relative isolate flex h-[20.625rem] flex-col justify-end gap-0 overflow-hidden bg-dark px-[1.125rem] pb-[4.625rem] text-panel sm:h-[25rem] sm:flex-row sm:items-end sm:gap-4 sm:px-8 sm:pt-[1.625rem] sm:pb-[6.125rem]"
>
	{#if gallery.hero_id}
		<img
			src={previewUrl(gallery.id, gallery.hero_id)}
			alt=""
			class="absolute inset-0 -z-20 size-full animate-hero-in object-cover {archived
				? 'brightness-[0.85] grayscale-[0.7]'
				: ''}"
		/>
	{:else}
		<div
			data-testid="no-hero"
			class="absolute inset-0 -z-20 bg-[radial-gradient(55%_80%_at_82%_10%,rgb(214_105_63/0.22),transparent_70%),radial-gradient(80%_120%_at_20%_100%,#3a2a20,transparent_70%),linear-gradient(160deg,#2b211b,#16120f_70%)]"
		></div>
		<div aria-hidden="true" class="absolute inset-0 -z-10 overflow-hidden">
			<span
				class="absolute top-1/2 right-[4%] -translate-y-1/2 font-serif text-[clamp(9.375rem,24vw,20.625rem)] leading-none tracking-tight whitespace-nowrap text-panel/[0.07] italic select-none"
				>{galleryInitials(gallery.title)}</span
			>
			<span
				class="absolute inset-x-[1.125rem] top-[4.125rem] truncate font-mono text-[0.5625rem] tracking-[0.2em] text-[#d9792e]/80 uppercase sm:inset-x-8 sm:top-[4.875rem] sm:text-[0.625rem]"
			>
				<span
					class="mb-2 block h-1.5 w-[min(26rem,100%)] bg-[repeating-linear-gradient(90deg,rgb(251_248_243/0.14)_0_8px,transparent_8px_18px)]"
				></span>
				▸&nbsp;&nbsp;{createdLong}
			</span>
			<span
				class="absolute -inset-x-[40%] -inset-y-[20%] animate-leak bg-[linear-gradient(105deg,transparent_40%,rgb(232_110_50/0.16)_48%,rgb(255_206_140/0.12)_52%,transparent_60%)] motion-reduce:hidden"
			></span>
		</div>
	{/if}
	<div
		aria-hidden="true"
		class="absolute inset-0 -z-10 bg-linear-to-b from-[rgb(43_36_32/0.45)] via-[rgb(43_36_32/0)] via-35% to-[rgb(43_36_32/0.78)]"
	></div>

	<div class="grid min-w-0 flex-none gap-0 sm:flex-1">
		<StatusChip
			{status}
			dot
			class="w-fit border-transparent bg-panel/90 {status === 'shared' ? 'text-info' : ''}"
		/>
		{@render heading()}
		<p class="mb-3 text-[0.8125rem] opacity-90 sm:mb-0 sm:text-sm">{meta}</p>
	</div>
	<div class="flex gap-2.5 [&>*]:max-sm:flex-1">
		<Button
			variant="outline"
			size="sm"
			class="min-h-10! gap-2 border-panel/40! bg-panel/15! font-semibold text-panel! backdrop-blur-[6px] hover:bg-panel/25!"
			onclick={onpreview}><Eye size={16} aria-hidden="true" />Preview</Button
		>
		{#if archived}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={onrestore}><ArchiveRestore size={16} aria-hidden="true" />Restore</Button
			>
		{:else if !shared}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={onshare}><Send size={16} aria-hidden="true" />Share</Button
			>
		{:else if !delivered}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={ondeliver}><Send size={16} aria-hidden="true" />Deliver</Button
			>
		{/if}
	</div>

	{#if !archived}<JourneySteps {steps} />{/if}
</div>
