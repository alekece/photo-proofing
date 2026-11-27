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
	import { WandSparkles, Download, Heart, RotateCw, X } from '@lucide/svelte';
	import SelectionDisc from './SelectionDisc.svelte';
	import { formatDateTime } from '#lib/gallery/format.js';
	import CommentBadge from './CommentBadge.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';

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
		librarySelect,
		collectionSelect,
		source,
		badge
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
		 * Shows a selection circle alongside `onopen`. Only the circle toggles selection;
		 * the tile still opens the viewer when selection is active.
		 */
		librarySelect?: 'idle' | 'active';
		/** A hover/focus/touch circle; in active mode the tile body also toggles selection. */
		collectionSelect?: 'idle' | 'active';
		/** The owning gallery, shown on tiles in the workspace photo library. */
		source?: string;
		/** Optional icon badge named by this label, such as “Retouched photo”, shown only when supplied. */
		badge?: string;
	} = $props();

	const failed = $derived(
		status === 'upload-failed' || status === 'rejected' || status === 'process-failed'
	);
	const reveal =
		'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100';
	const busy = $derived(!failed && status !== 'ready');
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
			color: 'text-current',
			fill: 'none',
			label: m.tile_downloaded_at
		}
	};

	let loaded = $state(false);
	const fade = $derived(loaded ? 'animate-tile-in' : 'opacity-0');

	/** A cached or broken image can finish before its listeners are attached. */
	function seen(image: HTMLImageElement) {
		if (image.complete) loaded = true;
	}

	function select() {
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

<div class="relative isolate" class:group={!!librarySelect}>
	<div
		data-slot="photo-tile-frame"
		class={[
			'relative',
			(librarySelect || collectionSelect) && 'transition-transform motion-reduce:transition-none',
			(librarySelect || collectionSelect) && (selected ? 'scale-[0.975]' : 'scale-100')
		]}
	>
		<div
			data-slot="photo-tile-media"
			class={[
				'group relative block overflow-hidden bg-placeholder-tile',
				ratio === undefined && 'aspect-square',
				!collectionSelect && !librarySelect && 'rounded-md'
			]}
			style:aspect-ratio={ratio}
		>
			<CommentBadge count={commentCount} />
			{#if showPhoto}
				<button
					type="button"
					class={[
						'block size-full',
						'cursor-pointer',
						librarySelect && 'focus-visible:-outline-offset-2'
					]}
					aria-label={selectingCollection ? name : m.tile_open({ name })}
					onclick={selectingCollection ? select : onopen}
				>
					<img
						src={thumbnailUrl}
						alt={name}
						loading="lazy"
						class={['size-full object-cover', fade, 'transition-transform']}
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
						failed
							? 'justify-center gap-1 bg-danger/10 text-danger'
							: 'text-placeholder-foreground',
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
					{#if !selectingCollection && onretry && (status === 'upload-failed' || status === 'process-failed')}
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

			{#if cleanReady && thumbnailUrl && status !== 'ready'}
				<Card
					class={[
						'pointer-events-none absolute inset-x-2 bottom-2 flex flex-col items-center gap-1 rounded-control px-2 py-2 text-center',
						STATUS_TEXT,
						busy ? 'pb-5' : 'pb-2'
					]}
					surface="glass"
				>
					<span>{LABELS[status]()}</span>
					{#if failed && onretry && !selectingCollection}{@render retry('pointer-events-auto')}{/if}
				</Card>
				{#if busy}{@render progress()}{/if}
			{/if}

			{#if !selectingCollection && oncancelUpload}
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
			{#if collectionSelect && onselect}
				<SelectionDisc
					label={m.tile_select({ name })}
					{selected}
					visible={selectingCollection}
					side="left"
					onclick={select}
				/>
			{/if}
		</div>
		<div data-slot="photo-tile-meta" class="photo-tile-meta pointer-events-none">
			{#if badge}
				<StatusChip
					aria-label={badge}
					title={badge}
					dataSlot="photo-tile-badge"
					workspace
					class="pointer-events-auto w-fit"
				>
					<WandSparkles class="size-3.5" aria-hidden="true" />
				</StatusChip>
			{/if}
			{#if source}
				<Card
					as="p"
					surface="glass"
					class="tile-caption media-caption truncate rounded-control media-caption-summary"
				>
					{source}
				</Card>
			{/if}

			{#if caption && status === 'ready'}
				{@const date = formatDateTime(caption.at)}
				{@const look = captions[caption.kind]}
				<Card
					as="p"
					surface="glass"
					class="tile-caption media-caption grid-flow-col items-center justify-start rounded-control media-caption-summary"
				>
					<span class="sr-only">{look.label({ date })}</span>
					<span
						data-slot="tile-caption-icon"
						class="grid size-4 shrink-0 place-items-center {look.color}"
					>
						<look.icon
							size={16}
							class="shrink-0 {look.color}"
							fill={look.fill}
							aria-hidden="true"
						/>
					</span>
					<time datetime={caption.at} class="truncate" aria-hidden="true">{date}</time>
				</Card>
			{/if}
		</div>
		{#if librarySelect && onselect}
			<SelectionDisc
				label={m.tile_select({ name })}
				{selected}
				visible={librarySelect === 'active'}
				side="left"
				onclick={select}
			/>
		{/if}
	</div>
</div>

<style>
	:global([data-studio-mode='print'] [data-slot='photo-tile-frame']) {
		padding: 0.375rem;
		background: var(--panel);
	}
	:global([data-studio-mode='full_image']) .photo-tile-meta {
		position: absolute;
		inset: auto 0.5rem 0.5rem;
		display: grid;
		gap: 0.5rem;
	}
	:global([data-studio-mode='print'] [data-slot='photo-tile-frame']) {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}
	:global([data-studio-mode='print']) .photo-tile-meta {
		display: contents;
	}
	:global([data-studio-mode='print'] [data-slot='photo-tile-media']) {
		grid-area: 1 / 1;
	}
	:global([data-studio-mode='print'] [data-slot='photo-tile-badge']) {
		z-index: 3;
		grid-area: 1 / 1;
		align-self: end;
		justify-self: start;
		margin: 0.5rem;
	}
	:global(
		[data-studio-mode='print'] [data-slot='photo-tile-frame']:not(:has(.tile-caption))::after
	) {
		display: block;
		height: 2rem;
		content: '';
	}
	:global([data-studio-mode='print'] .tile-caption) {
		position: static;
		background: var(--panel);
		border: 0;
		color: var(--foreground);
		text-shadow: none;
		box-shadow: none;
		backdrop-filter: none;
		-webkit-backdrop-filter: none;
		border-radius: 0;
		margin: 0;
		padding: 0.75rem 0.125rem 0.25rem;
	}
	:global([data-studio-mode='print'] .tile-caption [data-slot='tile-caption-icon']) {
		background: transparent;
		color: var(--foreground);
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
