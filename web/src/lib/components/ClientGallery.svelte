<script lang="ts">
	import { Download, Heart } from '@lucide/svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import { Selection } from '#lib/gallery/selection.svelte.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import type { Signature } from '#lib/settings/settings.js';
	import CoverBackdrop from './CoverBackdrop.svelte';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import FilmStrip from './FilmStrip.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import ProofViewer from './ProofViewer.svelte';
	import StudioSignature from './StudioSignature.svelte';

	let {
		token = '',
		gallery,
		proofs = [],
		coverUrl,
		favorites = [],
		studio = null,
		studioLogo = null,
		signature = 'name',
		downloadAllUrl,
		preview = false,
		framed = false
	}: {
		/** The private link's token; the owner's preview has none. */
		token?: string;
		gallery: SharedGallery;
		proofs?: Proof[];
		/** Clean hero preview, resolved independently of the visible client proof set. */
		coverUrl?: string;
		favorites?: string[];
		/** The studio's signature from Settings; the cover and footer stay unsigned without it. */
		studio?: string | null;
		/** The studio's logo image URL from Settings. */
		studioLogo?: string | null;
		signature?: Signature;
		/** Once delivered: the archive of every finished photo. Without it no Download all is offered. */
		downloadAllUrl?: string;
		/** The owner's read-only preview: marks the page and gates every write action. */
		preview?: boolean;
		/** Shown inside the preview's phone frame, which has a fixed height. */
		framed?: boolean;
	} = $props();
	const SEPARATOR = ' · ';
	const CTA =
		'inline-flex cursor-pointer items-center gap-2 rounded-full border px-5 py-2.5 text-xs tracking-[0.16em] uppercase transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus';
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

	const delivered = $derived(gallery.delivered);
	const shown = $derived(
		onlyFavorites && !delivered ? proofs.filter(({ id }) => selection.has(id)) : proofs
	);
	const count = $derived(`${proofs.length} ${proofs.length === 1 ? 'photo' : 'photos'}`);
	const title = $derived(gallery.title ?? UNTITLED_GALLERY);
	const footer = $derived(`${studio ? `${studio.toUpperCase()}${SEPARATOR}` : ''}PHOTOPROOFING`);

	function viewGallery() {
		const reduced = matchMedia?.('(prefers-reduced-motion: reduce)').matches;

		grid?.scrollIntoView?.({ behavior: reduced ? 'auto' : 'smooth' });
	}
	let cover = $derived(coverUrl);
</script>

<!-- Container queries stand in for the prototype's 700px media rule, so the phone preview narrows too. -->
{#if proofs.length === 0 && !delivered && !cover}
	<main
		class="@container flex min-h-dvh flex-col items-center overflow-hidden bg-[radial-gradient(120%_70%_at_50%_0,#2e241d,#16120f_60%)] pt-[34px] pb-10 text-center text-[#f4efe8]"
	>
		<StudioSignature
			{studio}
			logo={studioLogo}
			{signature}
			stacked
			class="text-[2.75rem] opacity-95 @max-[700px]:text-4xl"
		/>
		<div class="mt-auto px-6">
			<h1 class="font-serif text-[clamp(40px,6vw,64px)] leading-none wrap-anywhere">{title}</h1>
			{#if gallery.expires_at}
				<div class="mt-3.5 text-xs tracking-[0.22em] uppercase opacity-75">
					<ExpiryBadge expiresAt={gallery.expires_at} />
				</div>
			{/if}
		</div>
		<FilmStrip
			class="mt-10 mb-[34px] w-full scale-[1.06] rotate-[-3deg] shadow-[0_20px_40px_-20px_#000]"
		/>
		<div>
			<p class="font-serif text-[1.625rem] italic">Your photos are on their way.</p>
			<p class="mt-1.5 text-[#bfb2a4]">Check back soon.</p>
		</div>
		<p class="mt-auto pt-10 text-xs tracking-[0.06em] text-[#8d7f72]">Shared with PhotoProofing</p>
	</main>
{:else if proofs.length === 0 && delivered}
	<main class="@container flex min-h-dvh flex-col bg-background">
		<header
			class="relative isolate flex h-[300px] flex-none flex-col justify-end overflow-hidden bg-[#2b2420] px-8 pb-[26px] text-panel"
		>
			{#if cover}
				<img
					src={cover}
					alt="Gallery cover"
					class="absolute inset-0 -z-20 size-full object-cover object-[center_35%]"
				/>
				<div
					class="absolute inset-0 -z-10 bg-[linear-gradient(rgb(43_36_32/0.3),transparent_30%,transparent_45%,rgb(43_36_32/0.75))]"
				></div>
			{:else}
				<CoverBackdrop title={gallery.title} />
			{/if}
			<StudioSignature
				{studio}
				logo={studioLogo}
				{signature}
				class="absolute inset-x-0 top-[22px] font-serif text-[1.1875rem] italic"
			/>
			<h1 class="font-serif text-[2.75rem] leading-none wrap-anywhere">{title}</h1>
			{#if gallery.expires_at}
				<div class="mt-2.5 text-xs tracking-[0.2em] uppercase opacity-85">
					<ExpiryBadge expiresAt={gallery.expires_at} />
				</div>
			{/if}
		</header>
		<div
			class="flex flex-1 flex-col items-center justify-center gap-[22px] px-5 py-[60px] text-center"
		>
			<FilmStrip
				class="w-[min(440px,90%)] rounded-[6px] [mask-image:linear-gradient(90deg,transparent,#000_15%,#000_85%,transparent)] pt-[18px]! pb-6! [--h:62px] [--perf:5px] @max-[700px]:[--h:52px]"
				count={6}
			/>
			<div>
				<p class="font-serif text-2xl italic">Your finished photos are on their way.</p>
				<p class="mt-1.5 text-muted-foreground">Check back soon.</p>
			</div>
		</div>
	</main>
{:else}
	<main class="@container bg-background">
		<header
			class="relative isolate flex min-h-[max(520px,100dvh)] flex-col overflow-hidden bg-[#2b2420] text-center text-panel @max-[700px]:min-h-[max(480px,100svh)] {framed
				? 'min-h-[600px]! @max-[700px]:min-h-[600px]!'
				: preview
					? 'min-h-[max(520px,calc(100dvh-56px))]'
					: ''}"
		>
			{#if cover}
				<img
					src={cover}
					alt="Gallery cover"
					class="absolute inset-0 -z-20 size-full object-cover object-[center_35%]"
				/>
				<div
					class="absolute inset-0 -z-10 bg-[linear-gradient(180deg,rgb(43_36_32/0.35),transparent_22%,transparent_50%,rgb(43_36_32/0.8))]"
				></div>
			{:else}
				<CoverBackdrop title={gallery.title} centered />
			{/if}
			<StudioSignature
				{studio}
				logo={studioLogo}
				{signature}
				class="px-5 pt-7 font-serif text-[1.1875rem] italic @max-[700px]:text-[0.9375rem]"
			/>
			<div class="mt-auto animate-photo-in px-6 pb-[34px]">
				{#if delivered}
					<p class="font-serif text-lg italic">Your photos are ready</p>
				{/if}
				<h1
					class="mb-3 font-serif text-[4rem] leading-none wrap-anywhere @max-[700px]:text-[2.5rem]"
				>
					{title}
				</h1>
				<p class="text-xs tracking-[0.2em] uppercase opacity-85">{count}</p>
				{#if gallery.expires_at}
					<div class="mt-1 text-xs opacity-85"><ExpiryBadge expiresAt={gallery.expires_at} /></div>
				{/if}
				<div
					class="mt-5 flex justify-center gap-2.5 @max-[700px]:flex-col @max-[700px]:items-center"
				>
					{#if delivered && downloadAllUrl}
						<a
							href={downloadAllUrl}
							download
							class="{CTA} border-panel bg-panel font-bold text-foreground hover:bg-panel"
						>
							<Download size={15} aria-hidden="true" />Download all
						</a>
					{/if}
					<button
						type="button"
						class="{CTA} border-panel/55 hover:bg-panel/15"
						onclick={viewGallery}
					>
						View the gallery
					</button>
				</div>
			</div>
		</header>
		<div bind:this={grid}>
			<div
				class="sticky top-0 z-10 flex items-center gap-3.5 border-b border-border bg-[rgb(244_239_232/0.96)] px-7 py-3 text-sm font-medium backdrop-blur-[10px] @max-[700px]:px-3.5 @max-[700px]:py-2.5"
			>
				<h2 class="min-w-0 truncate font-serif text-[1.25rem] @max-[700px]:text-[1.0625rem]">
					{title}
				</h2>
				<span class="flex-1"></span>
				{#if delivered}
					{#if downloadAllUrl}
						<Button size="sm" href={downloadAllUrl} download>
							<Download size={16} aria-hidden="true" class="mr-2" />Download all
						</Button>
					{/if}
				{:else}
					{#if extra > 0}
						<span class="font-bold text-warning">{extra} extra</span>
					{/if}
					<div
						role="group"
						aria-label="Show"
						class="flex rounded-full border border-border bg-panel p-[3px]"
					>
						{#each [false, true] as only (only)}
							<button
								type="button"
								aria-pressed={onlyFavorites === only}
								aria-label={only ? 'Favorites' : undefined}
								onclick={() => (onlyFavorites = only)}
								class="group inline-flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-[5px] text-[0.8125rem] font-semibold text-muted-foreground focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-foreground aria-pressed:text-panel"
							>
								{#if only}
									<Heart
										size={14}
										aria-hidden="true"
										fill="currentColor"
										class="text-danger group-aria-pressed:text-[#ff8f75]"
									/>
									{#key selection.count}<span class="inline-block animate-heart-pop tabular-nums"
											>{selection.count}</span
										>{/key}
									{#if gallery.included_photos !== null}
										<span class="font-medium opacity-75">of {gallery.included_photos}</span>
									{/if}
								{:else}
									All
								{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>
			{#if selection.error}
				<p role="alert" class="py-2 text-center font-bold text-warning">{selection.error}</p>
			{/if}
			{#if shown.length === 0}
				<p class="py-[50px] text-center text-muted-foreground">
					{proofs.length === 0 ? 'Your photos are on their way.' : 'No favorites yet'}
				</p>
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
		</div>
		<footer class="px-5 pt-2.5 pb-10 text-center text-xs tracking-[0.06em] text-muted-foreground">
			{footer}
		</footer>
	</main>
{/if}
{#if viewerIndex !== null}
	<ProofViewer
		proofs={viewerProofs}
		bind:index={viewerIndex}
		{title}
		favoriteCount={delivered ? undefined : selection.count}
		readonly={preview}
		isSelected={(id) => selection.has(id)}
		onclose={() => (viewerIndex = null)}
		onselect={select}
	/>
{/if}
