<script lang="ts">
	import type { Snippet } from 'svelte';
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
	const meta = $derived(
		[
			photoCount === null ? '' : `${photoCount} ${photoCount === 1 ? 'photo' : 'photos'}`,
			delivered ? 'delivered' : shared ? 'shared' : 'not shared yet'
		]
			.filter(Boolean)
			.join(' · ')
	);
	const steps = $derived([
		{
			id: 'upload',
			label: 'Upload',
			done: (photoCount ?? 0) > 0,
			fact: `${photoCount} ${photoCount === 1 ? 'photo' : 'photos'}`
		},
		{ id: 'share', label: 'Share', done: shared, fact: 'Shared' },
		{
			id: 'collect',
			label: 'Collect favorites',
			done: delivered || (favoriteCount ?? 0) > 0,
			fact: `${favoriteCount ?? 0} selected`
		},
		{ id: 'deliver', label: 'Deliver', done: delivered, fact: 'Delivered' }
	]);
</script>

<header
	data-slot="gallery-header"
	class="relative isolate flex min-h-64 flex-col justify-between gap-6 overflow-hidden rounded-card bg-dark p-4 text-inverse sm:p-6"
>
	{#if gallery.hero_id}
		<img
			src={previewUrl(gallery.id, gallery.hero_id)}
			alt=""
			class="absolute inset-0 -z-20 size-full object-cover"
		/>
		<div class="absolute inset-0 -z-10 bg-linear-to-t from-black/80 via-black/30 to-black/40"></div>
	{:else}
		<div
			data-testid="no-hero"
			class="absolute inset-0 -z-20 bg-linear-to-br from-[#3a2418] via-dark to-[#2a1a12]"
		></div>
		<div
			aria-hidden="true"
			class="absolute inset-0 -z-10 grid place-items-center overflow-hidden font-serif text-[min(40vw,16rem)] leading-none text-white/[0.06] select-none"
		>
			{galleryInitials(gallery.title)}
		</div>
		<div
			aria-hidden="true"
			class="absolute -inset-y-4 left-1/4 -z-10 w-1/2 animate-leak bg-linear-to-r from-transparent via-golden-hour/25 to-transparent blur-2xl"
		></div>
		<div
			aria-hidden="true"
			class="absolute inset-x-0 top-3 -z-10 flex justify-between border-y border-white/15 px-3 py-0.5 font-mono text-[0.65rem] tracking-[0.25em] text-white/50 uppercase"
		>
			<span>PhotoProofing</span>
			<span>{created}</span>
		</div>
	{/if}

	<div class="mt-6 flex flex-wrap items-end justify-between gap-4">
		<div class="grid min-w-0 flex-1 gap-2">
			<StatusChip {status} class="w-fit border-transparent bg-inverse" />
			{@render heading()}
			<p class="px-2 text-sm opacity-90">{meta}</p>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<Button
				variant="outline"
				class="border-white/60 bg-transparent text-inverse hover:bg-white/15"
				onclick={onpreview}>Preview</Button
			>
			{#if archived}
				<Button class="bg-inverse text-foreground" onclick={onrestore}>Restore</Button>
			{:else if !shared}
				<Button class="bg-inverse text-foreground" onclick={onshare}>Share</Button>
			{:else if !delivered}
				<Button class="bg-inverse text-foreground" onclick={ondeliver}>Deliver</Button>
			{/if}
		</div>
	</div>

	{#if !archived}<JourneySteps {steps} />{/if}
</header>
