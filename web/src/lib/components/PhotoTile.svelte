<script lang="ts" module>
	import { m } from '#lib/paraglide/messages.js';
	export type TileState =
		| 'waiting-upload'
		| 'uploading'
		| 'upload-failed'
		| 'rejected'
		| 'waiting-process'
		| 'processing'
		| 'process-failed'
		| 'ready';

	/** The tile grid of a gallery tab: staggered columns or even rows, as the gallery's layout says. */
	export const GRID_CLASSES = {
		masonry: 'block columns-2 gap-2 sm:columns-[17rem_4] sm:gap-2.5',
		grid: 'grid grid-cols-2 gap-2 sm:grid-cols-[repeat(auto-fill,minmax(11.25rem,1fr))] sm:gap-2.5'
	} as const;
	export const ITEM_CLASSES = {
		masonry: 'mb-2 break-inside-avoid sm:mb-2.5',
		grid: ''
	} as const;

	/** Type style of every status label on a tile. */
	const STATUS_TEXT = 'text-xs font-semibold';

	const LABELS: Record<Exclude<TileState, 'ready'>, () => string> = {
		'waiting-upload': m.tile_waiting,
		uploading: m.tile_uploading,
		'upload-failed': m.upload_failed,
		rejected: m.tile_rejected,
		'waiting-process': m.tile_waiting,
		processing: m.tile_processing,
		'process-failed': m.tile_processing_failed
	};
</script>

<script lang="ts">
	import { Download, Heart, LoaderCircle, RotateCw, Star, X } from '@lucide/svelte';
	import { formatDateTime } from '#lib/gallery/format.js';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';

	let {
		name,
		state: status,
		ratio,
		error,
		thumbnailUrl,
		cleanReady = false,
		selected = false,
		onremove,
		ondownload,
		downloadBusy = false,
		caption,
		onretry,
		onopen,
		onselect,
		hero = false,
		onhero
	}: {
		name: string;
		state: TileState;
		/** Width over height of the photo; the tile is square without it. */
		ratio?: number;
		/** Replaces the label of a failed or rejected file, e.g. "Storage full". */
		error?: string;
		thumbnailUrl?: string;
		cleanReady?: boolean;
		selected?: boolean;
		/** Deletes the photo; a read-only tile, like a favorite, has none. */
		onremove?: () => void;
		/** Downloads this photo; shown on hover and always on touch screens. */
		ondownload?: () => void;
		/** A download is being checked; show progress and prevent another click. */
		downloadBusy?: boolean;
		/** When the client favorited or downloaded the photo, shown over its bottom-left corner. */
		caption?: { kind: 'favorited' | 'downloaded'; at: string };
		onretry?: () => void;
		onopen?: () => void;
		/** Puts the tile in select mode: clicking it toggles `selected`; no remove, retry or open. */
		onselect?: () => void;
		/** Whether this photo is the gallery's hero. */
		hero?: boolean;
		/** Toggles the hero star; only a ready photo outside select mode offers it. */
		onhero?: () => void;
	} = $props();

	const failed = $derived(
		status === 'upload-failed' || status === 'rejected' || status === 'process-failed'
	);
	const reveal =
		'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100';
	const busy = $derived(!failed && status !== 'ready');

	let loaded = $state(false);
	const fade = $derived(loaded ? 'animate-tile-in' : 'opacity-0');

	/** A cached image can finish before its load listener is attached. */
	function seen(image: HTMLImageElement) {
		if (image.complete && image.naturalWidth > 0) loaded = true;
	}
</script>

{#snippet progress()}
	<div class="absolute inset-x-1.5 bottom-1.5 h-[3px] overflow-hidden rounded-full bg-white/60">
		<span class="bar block h-full w-2/5 rounded-full bg-foreground/60"></span>
	</div>
{/snippet}

{#snippet retry(classes: string)}
	<button
		type="button"
		class="{classes} flex min-h-8 w-fit cursor-pointer items-center gap-1 rounded-full bg-panel px-3 text-xs text-foreground shadow"
		aria-label={m.tile_retry({ name })}
		onclick={onretry}><RotateCw size={14} aria-hidden="true" />{m.action_retry()}</button
	>
{/snippet}

<svelte:element
	this={onselect ? 'label' : 'div'}
	class={[
		'group relative block overflow-hidden rounded-md bg-placeholder-tile',
		ratio === undefined && 'aspect-square',
		onselect && 'cursor-pointer',
		selected && 'shadow-[0_0_0_3px_var(--color-foreground)]'
	]}
	style:aspect-ratio={ratio}
>
	{#if (status === 'ready' || (cleanReady && thumbnailUrl)) && onselect}
		<img
			src={thumbnailUrl}
			alt={name}
			loading="lazy"
			class="size-full object-cover transition-transform {fade} {selected
				? 'scale-[0.94] rounded'
				: ''}"
			onload={() => (loaded = true)}
			{@attach seen}
		/>
	{:else if status === 'ready' || (cleanReady && thumbnailUrl)}
		<button
			type="button"
			class="block size-full cursor-zoom-in"
			aria-label={m.tile_open({ name })}
			onclick={onopen}
		>
			<img
				src={thumbnailUrl}
				alt={name}
				loading="lazy"
				class="size-full object-cover {fade}"
				onload={() => (loaded = true)}
				{@attach seen}
			/>
		</button>
	{:else}
		<div
			class={[
				'absolute inset-0 flex flex-col items-center p-2 text-center',
				STATUS_TEXT,
				failed ? 'justify-center gap-1 bg-danger/10 text-danger' : 'text-placeholder-foreground',
				busy &&
					'animate-shimmer bg-[linear-gradient(100deg,transparent_30%,var(--color-placeholder-tile-strong)_50%,transparent_70%)] bg-[length:200%_100%] pb-5'
			]}
		>
			{#if busy}
				<span class="grid min-h-0 w-full flex-1 place-items-center">
					<span class="max-w-full truncate font-normal" title={name}>{name}</span>
				</span>
			{:else}
				<span class="sr-only">{name}</span>
			{/if}
			<span class="font-semibold">{LABELS[status]()}</span>
			{#if failed && error && error !== LABELS[status]()}<span class="line-clamp-1">{error}</span
				>{/if}
			{#if !onselect && onretry && (status === 'upload-failed' || status === 'process-failed')}
				{@render retry('mt-0.5')}
			{/if}
		</div>
		{#if busy}{@render progress()}{/if}
	{/if}

	{#if cleanReady && thumbnailUrl && status !== 'ready' && !onselect}
		<div
			class={[
				'pointer-events-none absolute inset-x-0 bottom-0 flex flex-col items-center gap-1 bg-gradient-to-t from-black/60 to-transparent px-2 pt-8 text-center text-white',
				STATUS_TEXT,
				busy ? 'pb-5' : 'pb-2'
			]}
		>
			<span>{LABELS[status]()}</span>
			{#if failed && onretry}{@render retry('pointer-events-auto')}{/if}
		</div>
		{#if busy}{@render progress()}{/if}
	{/if}

	{#if onselect}
		<Checkbox
			class="absolute top-2 left-2 z-10 size-6 cursor-pointer appearance-none rounded-full border-2 border-panel bg-foreground/25 bg-center bg-no-repeat shadow-[0_1px_4px_rgb(0_0_0/0.3)] transition-colors checked:bg-panel checked:bg-[url('data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22%23201a16%22%20stroke-width%3D%223%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M5%2012.5l4.5%204.5L19%207.5%22%2F%3E%3C%2Fsvg%3E')] checked:bg-[length:70%]"
			aria-label={m.tile_select({ name })}
			checked={selected}
			onchange={onselect}
		/>
	{:else}
		{#if (status === 'ready' || (cleanReady && thumbnailUrl)) && onhero}
			<button
				type="button"
				class={[
					'absolute top-2 left-2 grid size-[1.875rem] cursor-pointer place-items-center rounded-full bg-panel/95',
					hero ? 'text-hero opacity-100' : `text-foreground ${reveal}`
				]}
				aria-label={hero ? m.cover_remove() : m.cover_set()}
				title={hero ? m.cover_remove() : m.cover_set()}
				aria-pressed={hero}
				onclick={onhero}
				><Star size={14} fill={hero ? 'currentColor' : 'none'} aria-hidden="true" /></button
			>
		{/if}
		{#if onremove}
			<button
				type="button"
				class={[
					'absolute top-1.5 right-1.5 grid size-6 cursor-pointer place-items-center rounded-full bg-panel/90 text-foreground',
					failed ? 'opacity-100' : reveal
				]}
				aria-label={m.tile_remove({ name })}
				onclick={onremove}><X size={14} aria-hidden="true" /></button
			>
		{/if}
		{#if status === 'ready' && ondownload}
			<button
				type="button"
				class="absolute right-1.5 bottom-1.5 grid size-8 cursor-pointer place-items-center rounded-full bg-panel/90 text-foreground {reveal}"
				aria-label={m.tile_download({ name })}
				aria-busy={downloadBusy}
				disabled={downloadBusy}
				onclick={ondownload}
			>
				{#if downloadBusy}
					<LoaderCircle
						size={16}
						class="animate-spin motion-reduce:animate-none"
						aria-hidden="true"
					/>
				{:else}
					<Download size={16} aria-hidden="true" />
				{/if}
			</button>
		{/if}
	{/if}

	{#if caption && status === 'ready'}
		{@const date = formatDateTime(caption.at)}
		{@const Icon = caption.kind === 'favorited' ? Heart : Download}
		<p
			class="pointer-events-none absolute inset-x-0 bottom-0 flex items-center gap-1 bg-gradient-to-t from-black/60 to-transparent pt-6 pr-11 pb-1.5 pl-2 text-xs font-semibold text-white"
		>
			<span class="sr-only"
				>{caption.kind === 'favorited'
					? m.tile_favorited_at({ date })
					: m.tile_downloaded_at({ date })}</span
			>
			<Icon
				size={16}
				class="shrink-0 {caption.kind === 'favorited' ? 'text-danger' : 'text-ok-on-dark'}"
				fill={caption.kind === 'favorited' ? 'currentColor' : 'none'}
				aria-hidden="true"
			/>
			<time datetime={caption.at} class="truncate" aria-hidden="true">{date}</time>
		</p>
	{/if}
</svelte:element>

<style>
	.bar {
		animation: loop 1.4s ease-in-out infinite;
	}

	@keyframes loop {
		from {
			transform: translateX(-100%);
		}

		to {
			transform: translateX(250%);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.bar {
			width: 100%;
			animation: none;
		}
	}
</style>
