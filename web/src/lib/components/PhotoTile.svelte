<script lang="ts" module>
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
		masonry: 'block columns-2 gap-2 sm:columns-[11.25rem_4] sm:gap-2.5',
		grid: 'grid grid-cols-2 gap-2 sm:grid-cols-[repeat(auto-fill,minmax(11.25rem,1fr))] sm:gap-2.5'
	} as const;
	export const ITEM_CLASSES = {
		masonry: 'mb-2 break-inside-avoid sm:mb-2.5',
		grid: ''
	} as const;

	const LABELS: Record<Exclude<TileState, 'ready'>, string> = {
		'waiting-upload': 'Waiting',
		uploading: 'Uploading',
		'upload-failed': 'Upload failed',
		rejected: 'Rejected',
		'waiting-process': 'Waiting',
		processing: 'Processing',
		'process-failed': 'Processing failed'
	};
</script>

<script lang="ts">
	import { Download, RotateCw, Star, X } from '@lucide/svelte';
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';

	let {
		name,
		state,
		ratio,
		error,
		thumbnailUrl,
		selected = false,
		onremove,
		ondownload,
		downloaded = false,
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
		selected?: boolean;
		/** Deletes the photo; a read-only tile, like a favorite, has none. */
		onremove?: () => void;
		/** Downloads this photo; shown on hover and always on touch screens. */
		ondownload?: () => void;
		/** Marks a delivered photo the client has downloaded. */
		downloaded?: boolean;
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
		state === 'upload-failed' || state === 'rejected' || state === 'process-failed'
	);
	const reveal =
		'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100';
	const busy = $derived(!failed && state !== 'ready');
</script>

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
	{#if state === 'ready' && onselect}
		<img
			src={thumbnailUrl}
			alt={name}
			loading="lazy"
			class="size-full object-cover transition-transform {selected ? 'scale-[0.94] rounded' : ''}"
		/>
	{:else if state === 'ready'}
		<button
			type="button"
			class="block size-full cursor-zoom-in"
			aria-label="Open {name}"
			onclick={onopen}
		>
			<img src={thumbnailUrl} alt={name} loading="lazy" class="size-full object-cover" />
		</button>
	{:else}
		<div
			class={[
				'flex size-full flex-col items-center justify-center gap-1 p-2 text-center text-xs',
				failed ? 'bg-danger/10 text-danger' : 'text-placeholder-foreground'
			]}
		>
			<span class="max-w-full truncate" title={name}>{name}</span>
			<span>{LABELS[state]}</span>
			{#if failed && error && error !== LABELS[state]}<span>{error}</span>{/if}
		</div>
		{#if busy}
			<div class="absolute inset-x-1.5 bottom-1.5 h-[3px] overflow-hidden rounded-full bg-white/60">
				<span class="bar block h-full w-2/5 rounded-full bg-foreground/60"></span>
			</div>
		{/if}
	{/if}

	{#if onselect}
		<Checkbox
			class="absolute top-2 left-2 z-10 size-6 cursor-pointer appearance-none rounded-full border-2 border-panel bg-foreground/25 bg-center bg-no-repeat shadow-[0_1px_4px_rgb(0_0_0/0.3)] transition-colors checked:bg-panel checked:bg-[url('data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22%23201a16%22%20stroke-width%3D%223%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M5%2012.5l4.5%204.5L19%207.5%22%2F%3E%3C%2Fsvg%3E')] checked:bg-[length:70%]"
			aria-label="Select {name}"
			checked={selected}
			onchange={onselect}
		/>
	{:else}
		{#if state === 'ready' && onhero}
			<button
				type="button"
				class={[
					'absolute top-2 left-2 grid size-[1.875rem] cursor-pointer place-items-center rounded-full bg-panel/95',
					hero ? 'text-hero opacity-100' : `text-foreground ${reveal}`
				]}
				aria-label={hero ? 'Remove hero' : 'Set as hero'}
				title={hero ? 'Remove hero' : 'Set as hero'}
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
				aria-label="Remove {name}"
				onclick={onremove}><X size={14} aria-hidden="true" /></button
			>
		{/if}
		{#if state === 'ready' && ondownload}
			<button
				type="button"
				class="absolute right-1.5 bottom-1.5 grid size-8 cursor-pointer place-items-center rounded-full bg-panel/90 text-foreground {reveal}"
				aria-label="Download {name}"
				onclick={ondownload}><Download size={16} aria-hidden="true" /></button
			>
		{/if}
		{#if downloaded}
			<span
				class="absolute bottom-1.5 left-1.5 grid size-6 place-items-center rounded-full bg-panel/90 text-foreground"
				role="img"
				aria-label="Downloaded by the client"
				title="Downloaded by the client"><Download size={14} aria-hidden="true" /></span
			>
		{/if}
		{#if onretry && (state === 'upload-failed' || state === 'process-failed')}
			<button
				type="button"
				class="absolute inset-x-1.5 bottom-3 mx-auto flex min-h-8 w-fit cursor-pointer items-center gap-1 rounded-full bg-panel px-3 text-xs text-foreground shadow"
				aria-label="Retry {name}"
				onclick={onretry}><RotateCw size={14} aria-hidden="true" />Retry</button
			>
		{/if}
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
