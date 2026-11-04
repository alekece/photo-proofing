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

	const LABELS: Record<Exclude<TileState, 'ready'>, string> = {
		'waiting-upload': 'Waiting to upload',
		uploading: 'Uploading',
		'upload-failed': 'Upload failed',
		rejected: 'Rejected',
		'waiting-process': 'Waiting to process',
		processing: 'Processing',
		'process-failed': 'Processing failed'
	};
</script>

<script lang="ts">
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';

	let {
		name,
		state,
		error,
		thumbnailUrl,
		selected = false,
		onremove,
		onretry,
		onopen,
		onselect
	}: {
		name: string;
		state: TileState;
		/** Replaces the label of a failed or rejected file, e.g. "Storage full". */
		error?: string;
		thumbnailUrl?: string;
		selected?: boolean;
		onremove: () => void;
		onretry?: () => void;
		onopen?: () => void;
		/** Puts the tile in select mode: clicking it toggles `selected`; no remove, retry or open. */
		onselect?: () => void;
	} = $props();

	const failed = $derived(
		state === 'upload-failed' || state === 'rejected' || state === 'process-failed'
	);
	const looping = $derived(state === 'uploading' || state === 'processing');
	const still = $derived(state === 'waiting-upload' || state === 'waiting-process');
</script>

<svelte:element
	this={onselect ? 'label' : 'div'}
	class={[
		'group relative block aspect-square overflow-hidden rounded-xs bg-placeholder-tile',
		onselect && 'cursor-pointer',
		selected && 'outline-2 outline-offset-2 outline-foreground'
	]}
>
	{#if state === 'ready' && onselect}
		<img src={thumbnailUrl} alt={name} loading="lazy" class="size-full object-cover" />
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
			<span>{failed ? (error ?? LABELS[state]) : LABELS[state]}</span>
		</div>
		{#if looping || still}
			<div class="absolute inset-x-1.5 bottom-1.5 h-[3px] overflow-hidden rounded-full bg-white/60">
				{#if looping}<span class="bar block h-full w-2/5 rounded-full bg-foreground/60"></span>{/if}
			</div>
		{/if}
	{/if}

	{#if onselect}
		<Checkbox
			class="absolute top-1.5 left-1.5 size-5 cursor-pointer accent-foreground"
			aria-label="Select {name}"
			checked={selected}
			onchange={onselect}
		/>
	{:else}
		<button
			type="button"
			class="absolute top-1.5 right-1.5 grid size-6 cursor-pointer place-items-center rounded-full bg-panel/90 text-xs text-foreground opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
			aria-label="Remove {name}"
			onclick={onremove}>✕</button
		>
		{#if onretry && (state === 'upload-failed' || state === 'process-failed')}
			<button
				type="button"
				class="absolute right-1.5 bottom-1.5 grid size-8 cursor-pointer place-items-center rounded-full bg-panel text-sm text-foreground shadow"
				aria-label="Retry {name}"
				onclick={onretry}>↻</button
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
