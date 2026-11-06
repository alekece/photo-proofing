<script lang="ts">
	import { Download } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import { Selection } from '#lib/gallery/selection.svelte.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import FilmStrip from './FilmStrip.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';

	let {
		token = '',
		gallery,
		proofs = [],
		favorites = [],
		studio = null,
		downloadAllUrl,
		preview = false,
		contained = false
	}: {
		/** The private link's token; the owner's preview has none. */
		token?: string;
		gallery: SharedGallery;
		proofs?: Proof[];
		favorites?: string[];
		/** The studio's signature from Settings; the cover and footer stay unsigned without it. */
		studio?: string | null;
		/** Once delivered: the archive of every finished photo. Without it no Download all is offered. */
		downloadAllUrl?: string;
		/** The owner's read-only preview: marks the page and gates every write action. */
		preview?: boolean;
		/** Opens the photo viewer inside the nearest positioned ancestor, e.g. a preview frame. */
		contained?: boolean;
	} = $props();
	const SEPARATOR = ' · ';
	let viewerIndex = $state<number | null>(null);
	// Frozen while the viewer is open, so unfavoriting under the ♥ filter doesn't shift its photos.
	let viewerProofs = $state.raw<Proof[]>([]);
	let onlyFavorites = $state(false);
	let grid = $state<HTMLElement>();

	// The page is loaded once per link, so the initial values are read once on purpose.
	// svelte-ignore state_referenced_locally
	const selection = new Selection({ token, initial: favorites });
	const select = $derived(
		preview || gallery.delivered ? undefined : (id: string) => void selection.toggle(id)
	);
	const extra = $derived(
		gallery.included_photos === null ? 0 : Math.max(0, selection.count - gallery.included_photos)
	);

	const summary = $derived(
		`${selection.count} selected` +
			(gallery.included_photos === null ? '' : `${SEPARATOR}${gallery.included_photos} included`)
	);

	// Another browser on the same link changes the selection; a resync means events were missed.
	onMount(() => {
		if (preview || gallery.delivered) return;

		const source = new EventSource(`/api/v1/galleries/shared/${encodeURIComponent(token)}/events`);
		const refresh = () => void selection.refresh();

		source.addEventListener('favorites', refresh);
		source.addEventListener('resync', refresh);

		return () => source.close();
	});
	const delivered = $derived(gallery.delivered);
	const shown = $derived(
		onlyFavorites && !delivered ? proofs.filter(({ id }) => selection.has(id)) : proofs
	);
	const count = $derived(`${proofs.length} ${proofs.length === 1 ? 'photo' : 'photos'}`);

	function viewGallery() {
		const reduced = matchMedia?.('(prefers-reduced-motion: reduce)').matches;

		grid?.scrollIntoView?.({ behavior: reduced ? 'auto' : 'smooth' });
	}
	// Proofs are the ready photos, so the cover can only ever be a ready photo's preview.
	let cover = $derived(proofs.find(({ id }) => id === gallery.hero_id));
</script>

{#if proofs.length === 0 && !delivered}
	<main
		class="grid min-h-dvh grid-rows-[auto_1fr_auto] gap-8 overflow-hidden bg-dark p-6 text-center text-inverse sm:p-10"
	>
		{#if studio}
			<p class="font-mono text-sm tracking-[0.25em] uppercase">{studio}</p>
		{:else}
			<span aria-hidden="true"></span>
		{/if}
		<div class="mx-auto grid w-full content-center justify-items-center gap-6">
			<h1 class="font-serif text-[2rem] wrap-anywhere sm:text-6xl">
				{gallery.title ?? UNTITLED_GALLERY}
			</h1>
			{#if gallery.expires_at}
				<ExpiryBadge expiresAt={gallery.expires_at} />
			{/if}
			<FilmStrip class="w-[120%] max-w-none -rotate-2" />
			<div class="grid gap-1">
				<p class="font-serif text-2xl italic">Your photos are on their way.</p>
				<p class="text-inverse/70">Check back soon.</p>
			</div>
		</div>
		<p class="font-mono text-xs tracking-[0.25em] text-inverse/50 uppercase">
			{#if studio}{studio}{SEPARATOR}{/if}PhotoProofing
		</p>
	</main>
{:else}
	<main class="bg-background">
		<header
			class="relative isolate flex min-h-dvh flex-col justify-between gap-8 overflow-hidden bg-dark p-6 text-center text-inverse sm:p-10"
		>
			{#if cover}
				<img
					src={cover.previewUrl}
					alt="Gallery cover"
					class="absolute inset-0 -z-20 size-full object-cover"
				/>
				<div
					class="absolute inset-0 -z-10 bg-linear-to-t from-black/80 via-black/20 to-black/40"
				></div>
			{:else}
				<div
					data-testid="no-hero"
					class="absolute inset-0 -z-20 bg-linear-to-br from-[#3a2418] via-dark to-[#2a1a12]"
				></div>
				<div
					aria-hidden="true"
					class="absolute -inset-y-4 left-1/4 -z-10 w-1/2 animate-leak bg-linear-to-r from-transparent via-golden-hour/25 to-transparent blur-2xl"
				></div>
			{/if}
			{#if studio}
				<p class="font-mono text-sm tracking-[0.25em] uppercase">{studio}</p>
			{:else}
				<span aria-hidden="true"></span>
			{/if}
			<div class="mx-auto grid max-w-3xl justify-items-center gap-4">
				{#if delivered}
					<p class="text-sm tracking-[0.2em] uppercase opacity-90">Your photos are ready</p>
				{/if}
				<h1 class="font-serif text-[2rem] wrap-anywhere sm:text-6xl">
					{gallery.title ?? UNTITLED_GALLERY}
				</h1>
				<p class="opacity-90">{count}</p>
				{#if gallery.expires_at}
					<ExpiryBadge expiresAt={gallery.expires_at} />
				{/if}
				{#if proofs.length > 0}
					<div class="flex flex-wrap justify-center gap-3">
						<Button class="bg-inverse text-foreground" onclick={viewGallery}
							>View the gallery</Button
						>
						{#if delivered && downloadAllUrl}
							<Button
								variant="outline"
								class="border-white/60 bg-transparent text-inverse hover:bg-white/15"
								href={downloadAllUrl}
								download
							>
								<Download size={18} aria-hidden="true" class="mr-2" />Download all
							</Button>
						{/if}
					</div>
				{/if}
			</div>
		</header>
		<div bind:this={grid} class="mx-auto grid max-w-7xl gap-4 px-4 pb-6">
			{#if proofs.length === 0}
				<div class="grid justify-items-center gap-4 py-16 text-center">
					<FilmStrip class="w-full max-w-md" />
					<div class="grid gap-1">
						<p class="font-serif text-2xl italic">Your finished photos are on their way.</p>
						<p>Check back soon.</p>
					</div>
				</div>
			{:else}
				<div
					class="sticky top-0 z-10 -mx-4 flex flex-wrap items-center justify-between gap-2 border-b border-border bg-background/95 px-4 py-2 backdrop-blur"
				>
					<h2 class="min-w-0 flex-1 truncate font-serif text-xl">
						{gallery.title ?? UNTITLED_GALLERY}
					</h2>
					{#if delivered}
						{#if downloadAllUrl}
							<Button variant="outline" size="sm" href={downloadAllUrl} download>
								<Download size={18} aria-hidden="true" class="mr-2" />Download all
							</Button>
						{/if}
					{:else}
						<div
							role="group"
							aria-label="Show"
							class="flex gap-1 rounded-full border border-border p-1"
						>
							{#each [false, true] as only (only)}
								<button
									type="button"
									aria-pressed={onlyFavorites === only}
									onclick={() => (onlyFavorites = only)}
									class="min-h-9 cursor-pointer rounded-full px-4 text-sm focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-dark aria-pressed:text-inverse"
								>
									{#if only}
										♥
										{#key selection.count}<span class="inline-block animate-heart-pop"
												>{selection.count}</span
											>{/key}
									{:else}
										All
									{/if}
								</button>
							{/each}
						</div>
					{/if}
				</div>
				{#if !delivered}
					<p class="text-center">
						{summary}{#if extra > 0}{SEPARATOR}<span class="font-bold text-warning"
								>{extra} extra</span
							>{/if}
					</p>
				{/if}
				{#if selection.error}
					<p role="alert" class="text-center font-bold text-warning">{selection.error}</p>
				{/if}
				{#if shown.length === 0}
					<p class="py-12 text-center text-muted-foreground">No favorites yet</p>
				{:else}
					<ProofGrid
						proofs={shown}
						layout={gallery.layout}
						isSelected={(id) => selection.has(id)}
						readonly={preview}
						onopen={(i) => {
							viewerProofs = shown;
							viewerIndex = i;
						}}
						onselect={select}
					/>
				{/if}
			{/if}
		</div>
		<footer
			class="py-8 text-center font-mono text-xs tracking-[0.25em] text-muted-foreground uppercase"
		>
			{#if studio}{studio}{SEPARATOR}{/if}PhotoProofing
		</footer>
	</main>
{/if}
{#if viewerIndex !== null}
	<ProofViewer
		proofs={viewerProofs}
		bind:index={viewerIndex}
		{contained}
		favoriteCount={delivered ? undefined : selection.count}
		readonly={preview}
		isSelected={(id) => selection.has(id)}
		onclose={() => (viewerIndex = null)}
		onselect={select}
	/>
{/if}
