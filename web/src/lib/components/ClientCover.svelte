<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Download } from '@lucide/svelte';
	import { formatFullDate } from '#lib/gallery/format.js';
	import { DEFAULT_FOCAL_POINT, type CoverLayout, type FocalPoint } from '#lib/gallery/schemas.js';
	import { DEFAULT_SIZE, type Signature } from '#lib/settings/settings.js';
	import CoverBackdrop from './CoverBackdrop.svelte';
	import ExpiryBadge from './ExpiryBadge.svelte';
	import StudioSignature from './StudioSignature.svelte';

	let {
		hero = $bindable(),
		title,
		rawTitle,
		coverLayout = 'full_bleed',
		focalPoint = DEFAULT_FOCAL_POINT,
		shootDate = null,
		expiresAt = null,
		count,
		coverUrl,
		studio = null,
		studioLogo = null,
		signature = 'name',
		signatureSize = DEFAULT_SIZE,
		delivered = false,
		downloading = false,
		preview = false,
		framed = false,
		onviewgallery,
		ondownloadall
	}: {
		hero?: HTMLElement;
		title: string;
		rawTitle: string | null;
		coverLayout?: CoverLayout;
		focalPoint?: FocalPoint;
		shootDate?: string | null;
		expiresAt?: string | null;
		count: string;
		coverUrl?: string;
		studio?: string | null;
		studioLogo?: string | null;
		signature?: Signature;
		signatureSize?: number;
		delivered?: boolean;
		downloading?: boolean;
		preview?: boolean;
		framed?: boolean;
		onviewgallery: () => void;
		ondownloadall: () => void;
	} = $props();

	const CTA =
		'inline-flex cursor-pointer items-center gap-2 rounded-full border px-5 py-2.5 text-xs tracking-[0.16em] uppercase transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-focus';

	const objectPosition = $derived(`${focalPoint.x}% ${focalPoint.y}%`);
	// Shoot date is a calendar day: read at local noon so time zones never shift it.
	const formattedShootDate = $derived(shootDate ? formatFullDate(`${shootDate}T12:00`) : null);
	const heightClass = $derived(
		framed
			? 'min-h-[600px] @max-[700px]:min-h-[600px]'
			: preview
				? 'min-h-[max(520px,calc(100dvh-56px))] @max-[700px]:min-h-[max(480px,100svh)]'
				: 'min-h-[max(520px,100dvh)] @max-[700px]:min-h-[max(480px,100svh)]'
	);
</script>

{#snippet metaAndActions(onDark: boolean)}
	<p class="text-xs tracking-[0.2em] uppercase {onDark ? 'opacity-85' : 'text-muted-foreground'}">
		{#if formattedShootDate && shootDate}
			<time datetime={shootDate}>{formattedShootDate}</time><span aria-hidden="true"> · </span>
		{/if}
		<span>{count}</span>
	</p>
	{#if expiresAt}
		<div class="mt-1 text-xs {onDark ? 'opacity-85' : 'text-muted-foreground'}">
			<ExpiryBadge {expiresAt} />
		</div>
	{/if}
	<div
		class="mt-5 flex flex-wrap justify-center gap-2.5 @max-[700px]:flex-col @max-[700px]:items-center"
	>
		{#if delivered}
			<button
				type="button"
				disabled={downloading}
				onclick={ondownloadall}
				class="{CTA} font-bold disabled:cursor-progress disabled:opacity-80 {onDark
					? 'border-panel bg-panel text-foreground hover:bg-panel'
					: 'border-foreground bg-foreground text-panel hover:bg-foreground/90'}"
			>
				<Download size={15} aria-hidden="true" />{downloading
					? m.favorites_downloading()
					: m.favorites_download_all()}
			</button>
		{/if}
		<button
			type="button"
			class="{CTA} {onDark
				? 'border-panel/55 hover:bg-panel/15'
				: 'border-foreground/35 text-foreground hover:bg-foreground/5'}"
			onclick={onviewgallery}
		>
			{m.client_view_gallery()}
		</button>
	</div>
{/snippet}

{#if coverLayout === 'split'}
	<header
		bind:this={hero}
		data-cover-layout="split"
		class="grid {heightClass} grid-cols-2 overflow-hidden border-b border-border bg-background text-center text-foreground @max-[700px]:grid-cols-1"
	>
		<div class="relative isolate min-h-[260px] overflow-hidden bg-[#2b2420]">
			{#if coverUrl}
				<img
					src={coverUrl}
					alt={m.client_cover_alt()}
					style:object-position={objectPosition}
					class="absolute inset-0 size-full object-cover"
				/>
				<div data-image-context-menu-overlay class="absolute inset-0"></div>
			{:else}
				<CoverBackdrop title={rawTitle} edge={formattedShootDate} centered />
			{/if}
		</div>
		<div
			class="flex flex-col items-center justify-between px-8 py-9 @max-[700px]:px-6 @max-[700px]:py-7"
		>
			<StudioSignature
				{studio}
				logo={studioLogo}
				{signature}
				size={signatureSize}
				class="font-serif text-[1.1875rem] text-foreground italic @max-[700px]:text-[0.9375rem]"
			/>
			<div class="my-auto max-w-md animate-photo-in py-6">
				<h1
					class="mb-3 font-serif text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] wrap-anywhere text-foreground"
				>
					{title}
				</h1>
				{@render metaAndActions(false)}
			</div>
			<span aria-hidden="true"></span>
		</div>
	</header>
{:else if coverLayout === 'framed'}
	<header
		bind:this={hero}
		data-cover-layout="framed"
		class="flex {heightClass} flex-col items-center justify-between border-b border-border bg-background px-6 py-8 text-center text-foreground @max-[700px]:px-4 @max-[700px]:py-6"
	>
		<StudioSignature
			{studio}
			logo={studioLogo}
			{signature}
			size={signatureSize}
			class="font-serif text-[1.1875rem] text-foreground italic @max-[700px]:text-[0.9375rem]"
		/>
		<div
			class="my-4 w-full max-w-[44rem] rounded-card border border-border bg-panel p-3 shadow-[0_18px_40px_-24px_rgb(43_36_32/0.45)] @max-[700px]:p-2"
		>
			<div
				class="relative isolate aspect-[3/2] w-full overflow-hidden rounded-[0.4375rem] bg-[#2b2420]"
			>
				{#if coverUrl}
					<img
						src={coverUrl}
						alt={m.client_cover_alt()}
						style:object-position={objectPosition}
						class="size-full object-cover"
					/>
					<div data-image-context-menu-overlay class="absolute inset-0"></div>
				{:else}
					<CoverBackdrop title={rawTitle} edge={formattedShootDate} centered />
				{/if}
			</div>
		</div>
		<div class="max-w-xl animate-photo-in">
			<h1
				class="mb-2.5 font-serif text-[clamp(2rem,4vw,3.25rem)] leading-[1.06] wrap-anywhere text-foreground"
			>
				{title}
			</h1>
			{@render metaAndActions(false)}
		</div>
	</header>
{:else if coverLayout === 'banner'}
	<header
		bind:this={hero}
		data-cover-layout="banner"
		class="flex {heightClass} flex-col justify-between border-b border-border bg-background pt-8 text-center text-foreground @max-[700px]:pt-6"
	>
		<div
			class="mx-auto flex max-w-2xl flex-col items-center px-6 pb-6 @max-[700px]:px-4 @max-[700px]:pb-5"
		>
			<StudioSignature
				{studio}
				logo={studioLogo}
				{signature}
				size={signatureSize}
				class="font-serif text-[1.1875rem] text-foreground italic @max-[700px]:text-[0.9375rem]"
			/>
			<div class="mt-4 animate-photo-in">
				<h1
					class="mb-2.5 font-serif text-[clamp(2.25rem,4.5vw,3.5rem)] leading-[1.06] wrap-anywhere text-foreground"
				>
					{title}
				</h1>
				{@render metaAndActions(false)}
			</div>
		</div>
		<div
			class="relative isolate mx-auto mb-6 min-h-[220px] w-[min(72rem,calc(100%-3rem))] flex-1 overflow-hidden rounded-card bg-[#2b2420] @max-[700px]:mb-4 @max-[700px]:aspect-[16/10] @max-[700px]:w-[calc(100%-1.5rem)] @max-[700px]:flex-none"
		>
			{#if coverUrl}
				<img
					src={coverUrl}
					alt={m.client_cover_alt()}
					style:object-position={objectPosition}
					class="absolute inset-0 size-full object-cover"
				/>
				<div data-image-context-menu-overlay class="absolute inset-0"></div>
			{:else}
				<CoverBackdrop title={rawTitle} edge={formattedShootDate} centered />
			{/if}
		</div>
	</header>
{:else}
	<header
		bind:this={hero}
		data-cover-layout="full_bleed"
		class="relative isolate flex {heightClass} flex-col overflow-hidden bg-[#2b2420] text-center text-panel"
	>
		{#if coverUrl}
			<img
				src={coverUrl}
				alt={m.client_cover_alt()}
				style:object-position={objectPosition}
				class="absolute inset-0 -z-20 size-full object-cover"
			/>
			<div
				data-image-context-menu-overlay
				class="absolute inset-0 -z-10 bg-[linear-gradient(180deg,rgb(43_36_32/0.35),transparent_22%,transparent_50%,rgb(43_36_32/0.8))]"
			></div>
		{:else}
			<CoverBackdrop title={rawTitle} edge={formattedShootDate} centered />
		{/if}
		<StudioSignature
			{studio}
			logo={studioLogo}
			{signature}
			size={signatureSize}
			class="px-5 pt-7 font-serif text-[1.1875rem] italic @max-[700px]:text-[0.9375rem]"
		/>
		<div class="mt-auto animate-photo-in px-6 pb-[34px]">
			<h1 class="mb-3 font-serif text-[4rem] leading-none wrap-anywhere @max-[700px]:text-[2.5rem]">
				{title}
			</h1>
			{@render metaAndActions(true)}
		</div>
	</header>
{/if}
