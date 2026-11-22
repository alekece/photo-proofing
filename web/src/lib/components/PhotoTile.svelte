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
	export const COLLECTION_GRID_CLASSES =
		'grid grid-cols-2 gap-2 sm:gap-2.5 @min-[36rem]:grid-cols-3 @min-[58rem]:grid-cols-4';

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
	import { Download, Heart, RotateCw, X } from '@lucide/svelte';
	import SelectionDisc from './SelectionDisc.svelte';
	import { formatDateTime } from '#lib/gallery/format.js';
	import Checkbox from '#lib/components/ui/Checkbox/Checkbox.svelte';
	import ClickRipple from './ClickRipple.svelte';
	import CommentBadge from './CommentBadge.svelte';

	let {
		name,
		state: status,
		ratio,
		error,
		thumbnailUrl,
		cleanReady = false,
		selected = false,
		oncancelUpload,
		caption,
		onretry,
		onopen,
		onselect,
		commentCount = 0,
		persistentSelect = false,
		collectionSelect,
		source
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
		/** Cancels a local upload queue item, not a photo in the collection. */
		oncancelUpload?: () => void;
		/** When the client favorited or downloaded the photo, shown over its bottom-left corner. */
		caption?: { kind: 'favorited' | 'downloaded'; at: string };
		onretry?: () => void;
		onopen?: () => void;
		/** Puts the tile in select mode: clicking it toggles `selected`; no remove, retry or open. */
		onselect?: () => void;
		commentCount?: number;
		/**
		 * Shows an always-present selection circle alongside `onopen`, instead of the two being
		 * mutually exclusive: the tile opens on click, and the circle (not the tile) toggles
		 * `selected`. Requires both `onopen` and `onselect`.
		 */
		persistentSelect?: boolean;
		/** A hover/focus/touch circle; in active mode the tile body also toggles selection. */
		collectionSelect?: 'idle' | 'active';
		/** The owning gallery, shown on tiles in the workspace photo library. */
		source?: string;
	} = $props();

	const failed = $derived(
		status === 'upload-failed' || status === 'rejected' || status === 'process-failed'
	);
	const reveal =
		'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100';
	const busy = $derived(!failed && status !== 'ready');
	const checkboxSelect = $derived(onselect && !persistentSelect && !collectionSelect);
	const selectingCollection = $derived(collectionSelect === 'active' && !!onselect);
	const showPhoto = $derived(status === 'ready' || (cleanReady && thumbnailUrl));

	const captions = {
		favorited: {
			icon: Heart,
			color: 'text-danger',
			fill: 'currentColor',
			label: m.tile_favorited_at
		},
		downloaded: {
			icon: Download,
			color: 'rounded-full bg-dark p-1 text-white size-6',
			fill: 'none',
			label: m.tile_downloaded_at
		}
	};

	let rippling = $state(false);
	let loaded = $state(false);
	const fade = $derived(loaded ? 'animate-tile-in' : 'opacity-0');

	/** A cached or broken image can finish before its listeners are attached. */
	function seen(image: HTMLImageElement) {
		if (image.complete) loaded = true;
	}

	function select() {
		rippling = !selected;
		onselect?.();
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
		onclick={onretry}
	>
		<RotateCw size={14} aria-hidden="true" />{m.action_retry()}
	</button>
{/snippet}

<div class="relative isolate">
	<svelte:element
		this={checkboxSelect ? 'label' : 'div'}
		class={[
			'group relative block overflow-hidden bg-placeholder-tile',
			ratio === undefined && 'aspect-square',
			checkboxSelect && 'cursor-pointer',
			!collectionSelect && !persistentSelect && 'rounded-md'
		]}
		style:aspect-ratio={ratio}
	>
		<CommentBadge count={commentCount} />
		{#if showPhoto && checkboxSelect}
			<img
				src={thumbnailUrl}
				alt={name}
				loading="lazy"
				class="size-full object-cover transition-transform {fade} {selected
					? 'scale-[0.96] rounded'
					: ''}"
				onload={() => (loaded = true)}
				onerror={() => (loaded = true)}
				{@attach seen}
			/>
		{:else if showPhoto}
			<button
				type="button"
				class={['block size-full', selectingCollection ? 'cursor-pointer' : 'cursor-zoom-in']}
				aria-label={selectingCollection ? name : m.tile_open({ name })}
				onclick={selectingCollection ? select : onopen}
			>
				<img
					src={thumbnailUrl}
					alt={name}
					loading="lazy"
					class={[
						'size-full object-cover',
						fade,
						collectionSelect && 'transition-transform',
						(collectionSelect || persistentSelect) &&
							selected &&
							'scale-[0.975] motion-reduce:transition-none'
					]}
					onload={() => (loaded = true)}
					onerror={() => (loaded = true)}
					{@attach seen}
				/>
			</button>
		{:else if status !== 'ready'}
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
				{#if failed && error && error !== LABELS[status]()}<span class="line-clamp-1">
						{error}
					</span>{/if}
				{#if !checkboxSelect && !selectingCollection && onretry && (status === 'upload-failed' || status === 'process-failed')}
					{@render retry('mt-0.5')}
				{/if}
			</div>
			{#if busy}{@render progress()}{/if}
			{#if selectingCollection}
				<button
					type="button"
					class="absolute inset-0 w-full cursor-pointer focus-visible:outline-offset-[-3px]"
					aria-label={name}
					onclick={select}
				></button>
			{/if}
		{/if}

		{#if cleanReady && thumbnailUrl && status !== 'ready' && !checkboxSelect}
			<div
				class={[
					'pointer-events-none absolute inset-x-0 bottom-0 flex flex-col items-center gap-1 bg-gradient-to-t from-black/60 to-transparent px-2 pt-8 text-center text-white',
					STATUS_TEXT,
					busy ? 'pb-5' : 'pb-2'
				]}
			>
				<span>{LABELS[status]()}</span>
				{#if failed && onretry && !selectingCollection}{@render retry('pointer-events-auto')}{/if}
			</div>
			{#if busy}{@render progress()}{/if}
		{/if}

		{#if checkboxSelect}
			<span class="absolute top-3 left-3 z-10 grid size-6 text-panel">
				<Checkbox
					class="size-6 cursor-pointer appearance-none rounded-full border-2 border-panel bg-foreground/25 bg-center bg-no-repeat shadow-[0_1px_4px_rgb(0_0_0/0.3)] transition-colors checked:bg-panel checked:bg-[url('data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22%23201a16%22%20stroke-width%3D%223%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M5%2012.5l4.5%204.5L19%207.5%22%2F%3E%3C%2Fsvg%3E')] checked:bg-[length:70%]"
					aria-label={m.tile_select({ name })}
					checked={selected}
					onchange={() => {
						rippling = !selected;
						onselect?.();
					}}
				/>
				{#if rippling && selected}<ClickRipple onend={() => (rippling = false)} />{/if}
			</span>
		{:else if !selectingCollection && oncancelUpload}
			<button
				type="button"
				class={[
					'absolute top-1.5 right-1.5 grid size-6 cursor-pointer place-items-center rounded-full bg-panel/90 text-foreground',
					failed ? 'opacity-100' : reveal
				]}
				aria-label={m.tile_remove({ name })}
				onclick={oncancelUpload}
			>
				<X size={14} aria-hidden="true" />
			</button>
		{/if}
		{#if (persistentSelect || collectionSelect) && onselect}
			<SelectionDisc
				label={m.tile_select({ name })}
				{selected}
				visible={persistentSelect || selectingCollection}
				onclick={select}
			/>
		{/if}
	</svelte:element>
	{#if source}
		<p
			class="tile-caption pointer-events-none absolute inset-x-0 bottom-0 truncate bg-gradient-to-t from-black/85 to-transparent px-2 pt-6 pb-1.5 text-xs font-semibold text-white"
		>
			{source}
		</p>
	{/if}

	{#if caption && status === 'ready'}
		{@const date = formatDateTime(caption.at)}
		{@const look = captions[caption.kind]}
		<p
			class="tile-caption pointer-events-none absolute inset-x-0 bottom-0 flex items-center gap-1 bg-gradient-to-t from-black/85 to-transparent pt-6 pr-11 pb-1.5 pl-2 text-xs font-semibold text-white"
		>
			<span class="sr-only">{look.label({ date })}</span>
			<look.icon size={16} class="shrink-0 {look.color}" fill={look.fill} aria-hidden="true" />
			<time datetime={caption.at} class="truncate" aria-hidden="true">{date}</time>
		</p>
	{/if}
</div>

<style>
	:global([data-studio-mode='print']) .tile-caption {
		position: static;
		background: var(--panel);
		color: var(--foreground);
		padding: 0.5rem;
	}
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
