<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { CommentsClient } from '#lib/gallery/comments.js';
	import { PhotoComments } from '#lib/gallery/photoComments.svelte.js';
	import CommentThread from './CommentThread.svelte';
	import { Download, Heart, Play } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { formatFullDate } from '#lib/gallery/format.js';
	import { DEFAULT_FOCAL_POINT, HEADING_FONT_FAMILIES } from '#lib/gallery/schemas.js';
	import { Selection } from '#lib/gallery/selection.svelte.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';
	import { DEFAULT_SIZE, type Signature } from '#lib/settings/settings.js';
	import ClientCover from './ClientCover.svelte';
	import CoverBackdrop from './CoverBackdrop.svelte';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import FilmStrip from './FilmStrip.svelte';
	import ProofGrid from './ProofGrid.svelte';
	import PhotoViewer from './PhotoViewer.svelte';
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
		signatureSize = DEFAULT_SIZE,
		preview = false,
		framed = false,
		showCoverWhenEmpty = false,
		embedded = false
	}: {
		/** The private link's token; the owner's preview has none. */
		token?: string;
		gallery: SharedGallery;
		proofs?: Proof[];
		/** Clean hero preview, resolved independently of the visible client proof set. */
		coverUrl?: string;
		favorites?: string[];
		/** The studio's signature from Settings; the cover stays unsigned without it. */
		studio?: string | null;
		/** The studio's logo image URL from Settings. */
		studioLogo?: string | null;
		signature?: Signature;
		/** Size step of the signature, from Settings. */
		signatureSize?: number;
		/** The owner's read-only preview: marks the page and gates every write action. */
		preview?: boolean;
		/** Shown inside the preview's phone frame, which has a fixed height. */
		framed?: boolean;
		/** Renders the cover template even when the gallery has zero photos and no hero (for live Design editing). */
		showCoverWhenEmpty?: boolean;
		/** Embedded inside a page that already provides a `<main>` landmark (Studio Design). */
		embedded?: boolean;
	} = $props();
	let viewerIndex = $state<number | null>(null);
	let viewerAutoplay = $state(false);
	// Frozen while the viewer is open, so unfavoriting under the ♥ filter doesn't shift its photos.
	let viewerProofs = $state.raw<Proof[]>([]);
	let onlyFavorites = $state(false);
	let grid = $state<HTMLElement>();
	let hero = $state<HTMLElement>();
	// Server and first client render agree on a hidden bar, so hydration matches.
	let heroAtRest = $state(true);

	$effect(() => {
		if (!hero) return;

		// Without an observer the bar stays available instead of never appearing.
		if (typeof IntersectionObserver === 'undefined') {
			heroAtRest = false;

			return;
		}

		// The hero only counts as at rest while (almost) all of it is in view, so the bar
		// appears as soon as scrolling starts.
		const observer = new IntersectionObserver(
			([entry]) => (heroAtRest = entry.intersectionRatio >= 0.99),
			{ threshold: [0.99, 1] }
		);

		observer.observe(hero);

		return () => observer.disconnect();
	});

	// The page is loaded once per link, so the initial values are read once on purpose.
	// svelte-ignore state_referenced_locally
	const selection = new Selection({ token, initial: favorites });

	// Owner previews load and refresh favorites asynchronously after mount.
	$effect(() => {
		if (!preview) return;

		selection.ids.clear();
		for (const id of favorites) selection.ids.add(id);
	});
	const select = $derived(
		preview || gallery.delivered ? undefined : (id: string) => void selection.toggle(id)
	);
	const over = $derived(
		gallery.included_photos !== null && selection.count > gallery.included_photos
	);

	const delivered = $derived(gallery.delivered);
	const comments = $derived(
		token && !preview && !delivered
			? new PhotoComments(
					new CommentsClient(`/api/v1/galleries/shared/${encodeURIComponent(token)}`)
				)
			: null
	);
	$effect(() => comments?.start());
	const shown = $derived(
		onlyFavorites && !delivered ? proofs.filter(({ id }) => selection.has(id)) : proofs
	);
	// Never hide the bar when it is the only way back to the full gallery. The hero fills the
	// screen, so an unfiltered page with photos always scrolls and hides nothing the client needs.
	const barHidden = $derived(heroAtRest && !(onlyFavorites && !delivered));
	const count = $derived(m.summary_photos({ count: proofs.length }));
	const title = $derived(gallery.title ?? m.gallery_untitled());

	const downloadAllUrl = $derived(`/api/v1/galleries/shared/${encodeURIComponent(token)}/download`);
	let downloading = $state(false);

	/** Asks for the delivery ZIP until it is ready, then lets the browser stream it. */
	async function downloadAll() {
		if (downloading || preview) return;

		downloading = true;

		try {
			for (;;) {
				const response = await fetch(downloadAllUrl, { method: 'POST' });

				if (response.status === 200) break;
				if (response.status !== 202) throw new Error('The download could not be prepared.');

				await new Promise((resolve) => setTimeout(resolve, 1500));
			}

			const link = document.createElement('a');

			link.href = downloadAllUrl;
			link.download = '';
			link.click();
		} catch {
			toasts.error(m.client_download_failed());
		} finally {
			downloading = false;
		}
	}

	function viewGallery() {
		const reduced = matchMedia?.('(prefers-reduced-motion: reduce)').matches;

		grid?.scrollIntoView?.({ behavior: reduced ? 'auto' : 'smooth' });
	}
	let cover = $derived(coverUrl);
	const focalPoint = $derived(gallery.focal_point ?? DEFAULT_FOCAL_POINT);
	const objectPosition = $derived(`${focalPoint.x}% ${focalPoint.y}%`);
	const headingFont = $derived(gallery.heading_font ?? 'editorial');
	const headingFontFamily = $derived(HEADING_FONT_FAMILIES[headingFont]);
	const formattedShootDate = $derived(
		gallery.shoot_date ? formatFullDate(`${gallery.shoot_date}T12:00`) : null
	);
	const rootTag = $derived(embedded ? 'div' : 'main');
</script>

<!-- Container queries stand in for the prototype's 700px media rule, so the phone preview narrows too. -->
{#if proofs.length === 0 && !delivered && !cover && !showCoverWhenEmpty}
	<svelte:element
		this={rootTag}
		data-heading-font={headingFont}
		style:--gallery-heading-font={headingFontFamily}
		class="@container flex flex-1 flex-col items-center overflow-hidden bg-[radial-gradient(120%_70%_at_50%_0,#2e241d,#16120f_60%)] pt-[34px] pb-10 text-center text-[#f4efe8]"
	>
		<StudioSignature
			{studio}
			logo={studioLogo}
			{signature}
			size={signatureSize}
			stacked
			class="text-[2.75rem] opacity-95 @max-[700px]:text-4xl"
		/>
		<div class="mt-auto px-6">
			<h1
				class="font-[family-name:var(--gallery-heading-font)] text-[clamp(40px,6vw,64px)] leading-none wrap-anywhere"
			>
				{title}
			</h1>
			{#if formattedShootDate && gallery.shoot_date}
				<p class="mt-3 text-xs tracking-[0.22em] uppercase opacity-75">
					<time datetime={gallery.shoot_date}>{formattedShootDate}</time>
				</p>
			{/if}
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
			<p class="font-serif text-[1.625rem] italic">{m.client_on_their_way()}</p>
			<p class="mt-1.5 text-[#bfb2a4]">{m.client_check_back()}</p>
		</div>
		<p class="mt-auto pt-10 text-xs tracking-[0.06em] text-[#8d7f72]">{m.client_shared_with()}</p>
	</svelte:element>
{:else if proofs.length === 0 && delivered && !showCoverWhenEmpty}
	<svelte:element
		this={rootTag}
		data-heading-font={headingFont}
		style:--gallery-heading-font={headingFontFamily}
		class="@container flex flex-1 flex-col bg-background"
	>
		<header
			class="relative isolate flex h-[300px] flex-none flex-col justify-end overflow-hidden bg-[#2b2420] px-8 pb-[26px] text-panel"
		>
			{#if cover}
				<img
					src={cover}
					alt={m.client_cover_alt()}
					style:object-position={objectPosition}
					class="absolute inset-0 -z-20 size-full object-cover"
				/>
				<div
					data-image-context-menu-overlay
					class="absolute inset-0 -z-10 bg-[linear-gradient(rgb(43_36_32/0.3),transparent_30%,transparent_45%,rgb(43_36_32/0.75))]"
				></div>
			{:else}
				<CoverBackdrop title={gallery.title} edge={formattedShootDate} />
			{/if}
			<StudioSignature
				{studio}
				logo={studioLogo}
				{signature}
				size={signatureSize}
				class="absolute inset-x-0 top-[22px] font-serif text-[1.1875rem] italic"
			/>
			<h1
				class="font-[family-name:var(--gallery-heading-font)] text-[2.75rem] leading-none wrap-anywhere"
			>
				{title}
			</h1>
			{#if formattedShootDate && gallery.shoot_date}
				<p class="mt-2 text-xs tracking-[0.2em] uppercase opacity-85">
					<time datetime={gallery.shoot_date}>{formattedShootDate}</time>
				</p>
			{/if}
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
				<p class="font-serif text-2xl italic">{m.client_finals_on_their_way()}</p>
				<p class="mt-1.5 text-muted-foreground">{m.client_check_back()}</p>
			</div>
		</div>
	</svelte:element>
{:else}
	<svelte:element
		this={rootTag}
		data-heading-font={headingFont}
		style:--gallery-heading-font={headingFontFamily}
		class="@container bg-background"
	>
		<ClientCover
			bind:hero
			{title}
			rawTitle={gallery.title}
			coverLayout={gallery.cover_layout ?? 'full_bleed'}
			{focalPoint}
			shootDate={gallery.shoot_date ?? null}
			expiresAt={gallery.expires_at}
			{count}
			coverUrl={cover}
			{studio}
			{studioLogo}
			{signature}
			{signatureSize}
			{delivered}
			{downloading}
			{preview}
			{framed}
			onviewgallery={viewGallery}
			ondownloadall={() => void downloadAll()}
		/>
		<div bind:this={grid}>
			<!-- The row collapses to zero height while hidden, so there is no gap, and expands in flow so the bar never covers photos. -->
			<div
				class={[
					'sticky top-0 z-10 grid transition-[grid-template-rows,opacity] duration-300 motion-reduce:transition-none',
					barHidden ? 'grid-rows-[0fr] opacity-0' : 'grid-rows-[1fr] opacity-100'
				]}
			>
				<div class="min-h-0 overflow-hidden">
					<div
						inert={barHidden}
						aria-hidden={barHidden}
						class="flex items-center gap-3.5 border-b border-border bg-[rgb(244_239_232/0.65)] px-7 py-3 text-sm font-medium backdrop-blur-[14px] backdrop-saturate-120 @max-[700px]:px-3.5 @max-[700px]:py-2.5"
					>
						<h2
							class="min-w-0 truncate font-[family-name:var(--gallery-heading-font)] text-[1.25rem] @max-[700px]:text-[1.0625rem]"
						>
							{title}
						</h2>
						<span class="flex-1"></span>
						{#if shown.length > 1}
							<button
								type="button"
								aria-label={m.client_slideshow()}
								onclick={() => {
									viewerProofs = shown;
									viewerAutoplay = true;
									viewerIndex = 0;
								}}
								class="inline-flex min-h-9 min-w-9 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-full border border-border bg-panel px-3.5 py-1.5 text-[0.8125rem] font-semibold text-foreground transition-colors hover:bg-foreground/5 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus @max-[700px]:px-2.5"
							>
								<Play size={14} aria-hidden="true" />
								<span class="@max-[700px]:hidden">{m.client_slideshow()}</span>
							</button>
						{/if}
						{#if delivered}
							<Button size="sm" disabled={downloading} onclick={downloadAll}>
								<Download size={16} aria-hidden="true" class="mr-2" />{downloading
									? m.favorites_downloading()
									: m.favorites_download_all()}
							</Button>
						{:else}
							<div
								role="group"
								aria-label={m.client_show()}
								class="flex rounded-full border border-border bg-panel p-[3px]"
							>
								{#each [false, true] as only (only)}
									<button
										type="button"
										aria-pressed={onlyFavorites === only}
										onclick={() => (onlyFavorites = only)}
										class="group inline-flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-[5px] text-[0.8125rem] font-semibold text-muted-foreground focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus aria-pressed:bg-foreground aria-pressed:text-panel"
									>
										{#if only}
											<span class="sr-only">{m.tab_favorites()}</span>
											<Heart
												size={14}
												aria-hidden="true"
												fill="currentColor"
												class={gallery.included_photos === null
													? 'text-current'
													: 'text-danger group-aria-pressed:text-[#ff8f75]'}
											/>
											{#key selection.count}<span
													class={[
														'inline-block animate-heart-pop tabular-nums',
														over &&
															'font-extrabold text-warning group-aria-pressed:text-warning-on-dark'
													]}>{selection.count}</span
												>{/key}
											{#if gallery.included_photos !== null}
												<span class="font-medium whitespace-nowrap opacity-75"
													>{m.client_of_included({ count: gallery.included_photos })}</span
												>
											{/if}
										{:else}
											{m.client_all()}
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					</div>
				</div>
			</div>
			{#if selection.error}
				<p role="alert" class="py-2 text-center font-bold text-warning">{selection.error}</p>
			{/if}
			{#if comments?.error}<p role="alert" class="py-2 text-center text-danger">
					{comments.error}
				</p>{/if}
			{#if shown.length === 0}
				<p class="py-[50px] text-center text-muted-foreground">
					{proofs.length === 0 ? m.client_on_their_way() : m.favorites_empty()}
				</p>
			{:else}
				<ProofGrid
					proofs={shown}
					layout={gallery.layout}
					density={gallery.grid_density ?? 'compact'}
					spacing={gallery.grid_spacing ?? 'compact'}
					isSelected={(id) => selection.has(id)}
					readonly={preview}
					commentCount={comments ? (id) => comments.count(id) : undefined}
					onopen={(i) => {
						viewerAutoplay = false;
						viewerProofs = shown;
						viewerIndex = i;
					}}
					onselect={select}
				/>
			{/if}
		</div>
	</svelte:element>
{/if}
{#snippet photoThread(proof: Proof)}
	{#if comments}
		<CommentThread
			client={comments.client}
			photoId={proof.id}
			viewer="client"
			onchange={() => void comments?.refresh()}
		/>
	{/if}
{/snippet}
{#if viewerIndex !== null}
	<PhotoViewer
		proofs={viewerProofs}
		bind:index={viewerIndex}
		{title}
		readonly={preview}
		autoplay={viewerAutoplay}
		isSelected={(id) => selection.has(id)}
		onclose={() => {
			viewerIndex = null;
			viewerAutoplay = false;
		}}
		onselect={select}
		thread={comments ? photoThread : undefined}
		commentCount={(id) => comments?.count(id) ?? 0}
	/>
{/if}
