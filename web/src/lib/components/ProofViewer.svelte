<script lang="ts">
	import { ChevronLeft, ChevronRight, Download, Heart, LoaderCircle, X } from '@lucide/svelte';
	import { onMount, type Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import { cn } from '#lib/utils.js';
	import type { Proof } from '../gallery/types';
	import FavoriteButton from './FavoriteButton.svelte';
	import ViewerButton from './ViewerButton.svelte';

	let {
		proofs,
		index = $bindable(0),
		isSelected = () => false,
		class: className,
		title,
		favoriteCount,
		readonly = false,
		actions,
		onclose,
		onselect,
		onretry
	}: {
		proofs: Proof[];
		index?: number;
		isSelected?: (id: string) => boolean;
		class?: string;
		/**
		 * The gallery's title in the top bar, as the client sees it. Without it the viewer is the
		 * photographer's: the file name takes its place.
		 */
		title?: string;
		/** Shown in the top bar beside the photo's position. */
		favoriteCount?: number;
		/** Shows the heart without letting it change, as in the owner's views. */
		readonly?: boolean;
		/** The photo's actions in the top bar, which depend on where the viewer was opened from. */
		actions?: Snippet<[Proof]>;
		onclose: () => void;
		/** Without it the proof cannot be selected, as once the gallery is delivered. */
		onselect?: (id: string) => void;
		/** Offered when a photo failed processing. */
		onretry?: (id: string) => void;
	} = $props();

	// The heart sits in the top bar on wide screens and under the photo, in thumb reach, on phones.
	const wide =
		typeof matchMedia === 'function' ? new MediaQuery('min-width: 701px') : { current: true };
	const arrow =
		'absolute top-1/2 z-[2] grid size-12 -translate-y-1/2 cursor-pointer place-items-center rounded-full bg-white/8 text-panel transition-colors hover:bg-white/16 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus aria-disabled:cursor-default aria-disabled:opacity-40 aria-disabled:hover:bg-white/8 max-[700px]:hidden';
	const overlay =
		'absolute right-3.5 bottom-3.5 z-[3] grid size-[46px] place-items-center rounded-full backdrop-blur-[4px] transition-[background-color,opacity] focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus';
	const pill =
		'inline-flex h-[52px] cursor-pointer items-center gap-2.5 rounded-full pr-[22px] pl-[18px] text-[0.9375rem] font-semibold transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus';

	let closeButton: HTMLButtonElement;
	const proof = $derived(proofs[index]);
	const status = $derived(proof.status ?? 'ready');
	const client = $derived(title !== undefined);

	const go = (to: number) => (index = Math.min(Math.max(to, 0), proofs.length - 1));

	onMount(() => closeButton.focus());

	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLDialogElement }) {
		const dialog = event.currentTarget;

		switch (event.key) {
			case 'ArrowRight':
				go(index + 1);
				break;
			case 'ArrowLeft':
				go(index - 1);
				break;
			case 'Home':
				go(0);
				break;
			case 'End':
				go(proofs.length - 1);
				break;
			case 'Tab': {
				const items = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled), a[href]')];
				const first = items[0];
				const last = items[items.length - 1];
				const active = document.activeElement;
				if (!dialog.contains(active)) {
					event.preventDefault();
					first.focus();
				} else if (event.shiftKey && active === first) {
					event.preventDefault();
					last.focus();
				} else if (!event.shiftKey && active === last) {
					event.preventDefault();
					first.focus();
				}
				return;
			}
			default:
				return;
		}
		event.preventDefault();
	}
</script>

{#snippet heartPill(big: boolean)}
	<FavoriteButton
		selected={isSelected(proof.id)}
		disabled={!onselect}
		onclick={() => onselect?.(proof.id)}
		size={big ? 20 : 17}
		class={big
			? cn(
					pill,
					isSelected(proof.id)
						? 'bg-panel text-foreground [&_svg]:text-danger'
						: 'bg-white/10 text-panel',
					!onselect && 'cursor-not-allowed opacity-50'
				)
			: cn(
					'inline-flex h-10 cursor-pointer items-center gap-2 rounded-full px-3.5 text-[0.84375rem] font-semibold whitespace-nowrap transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus [&_svg]:text-panel',
					isSelected(proof.id)
						? 'bg-panel text-foreground [&_svg]:text-danger'
						: 'bg-white/8 text-[#efe6da] hover:bg-white/14',
					!onselect && 'cursor-not-allowed opacity-50 hover:bg-white/8'
				)}
	>
		{isSelected(proof.id) ? 'Favorite' : 'Add to favorites'}
	</FavoriteButton>
{/snippet}

<!-- Key handling lives on the dialog so it applies wherever focus is inside it. -->
<Dialog
	{onclose}
	{onkeydown}
	label="Proof viewer"
	aria-modal="true"
	class={cn(
		'm-0 h-dvh max-h-dvh w-screen max-w-screen animate-photo-in overflow-hidden border-0 bg-dark p-0 text-[0.875rem] font-medium text-[#efe6da] open:grid',
		client
			? 'grid-rows-[60px_1fr_44px] max-[700px]:grid-rows-[56px_1fr_104px]'
			: 'grid-rows-[60px_1fr_44px] max-[700px]:grid-rows-[56px_1fr_20px]',
		className
	)}
>
	{#snippet children(close)}
		<div class="flex min-w-0 items-center gap-3.5 px-5 max-[700px]:gap-2 max-[700px]:px-3">
			{#if client}
				<span class="truncate font-serif text-[1.1875rem] text-panel max-[700px]:hidden"
					>{title}</span
				>
			{:else}
				<span class="font-mono text-[0.8125rem] whitespace-nowrap text-[#a8998a]">{proof.alt}</span>
			{/if}
			<span aria-live="polite" class="whitespace-nowrap text-[#a8998a] tabular-nums"
				>{index + 1} of {proofs.length}</span
			>
			<span class="flex-1"></span>
			{#if favoriteCount !== undefined}
				<span class="flex items-center gap-1.5 font-semibold text-panel">
					<Heart size={17} aria-hidden="true" fill="currentColor" class="text-[#e4573b]" />
					<span class="sr-only">Favorites:</span>{favoriteCount}
				</span>
			{/if}
			{#if client && wide.current && status === 'ready' && (onselect || readonly) && !proof.downloadUrl}
				{@render heartPill(false)}
			{/if}
			{@render actions?.(proof)}
			<button
				type="button"
				bind:this={closeButton}
				onclick={close}
				aria-label="Close viewer"
				class="inline-grid size-10 cursor-pointer place-items-center rounded-full bg-white/8 text-[#efe6da] transition-colors hover:bg-white/14 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
			>
				<X size={20} aria-hidden="true" />
			</button>
		</div>
		<div
			class="relative flex min-h-0 items-center justify-center overflow-hidden px-[84px] max-[700px]:px-0"
		>
			{#if status === 'ready'}
				{#key proof.id}
					<span class="relative flex animate-photo-in">
						<img
							src={proof.previewUrl}
							alt={proof.alt}
							class="block max-h-[calc(100dvh-124px)] max-w-[calc(100vw-168px)] rounded-[2px] object-contain shadow-[0_30px_80px_rgb(0_0_0/0.5)] max-[700px]:max-h-[calc(100dvh-160px)] max-[700px]:max-w-screen max-[700px]:rounded-none max-[700px]:shadow-none"
						/>
						{#if wide.current && proof.downloadUrl}
							<a
								href={proof.downloadUrl}
								download
								aria-label="Download this photo"
								class="{overlay} bg-panel text-foreground shadow-[0_6px_18px_rgb(0_0_0/0.25)]"
							>
								<Download size={20} aria-hidden="true" />
							</a>
						{:else if !client && readonly && isSelected(proof.id)}
							<span
								role="img"
								aria-label="Marked as a favorite by the client"
								class="{overlay} bg-panel text-danger"
							>
								<Heart size={20} aria-hidden="true" fill="currentColor" />
							</span>
						{/if}
					</span>
				{/key}
			{:else if status === 'processing'}
				<p role="status" class="flex items-center gap-3 font-serif text-lg text-[#a8998a] italic">
					<LoaderCircle size={20} class="animate-spin-slow" aria-hidden="true" />
					Processing…
				</p>
			{:else}
				<div
					class="grid aspect-3/2 w-[min(560px,100%)] place-items-center rounded-md border border-dashed border-[#5a4d43] bg-[#241e1a] text-center text-[#a8998a]"
				>
					<div>
						<p role="alert" class="mb-3 text-[0.9375rem] font-semibold text-warning-on-dark">
							This photo could not be processed
						</p>
						{#if onretry}
							<ViewerButton onclick={() => onretry(proof.id)}>Try again</ViewerButton>
						{/if}
					</div>
				</div>
			{/if}
			<button
				type="button"
				class="{arrow} left-5"
				aria-disabled={index === 0}
				onclick={() => go(index - 1)}
				aria-label="Previous proof"><ChevronLeft size={22} aria-hidden="true" /></button
			>
			<button
				type="button"
				class="{arrow} right-5"
				aria-disabled={index === proofs.length - 1}
				onclick={() => go(index + 1)}
				aria-label="Next proof"><ChevronRight size={22} aria-hidden="true" /></button
			>
		</div>
		<div class="flex items-center justify-center gap-3">
			{#if client && !wide.current && status === 'ready'}
				{#if proof.downloadUrl}
					<a
						href={proof.downloadUrl}
						download
						class="{pill} bg-panel pr-[22px] pl-[22px] text-foreground"
					>
						<Download size={20} aria-hidden="true" />Download
					</a>
				{:else if onselect || readonly}
					{@render heartPill(true)}
				{/if}
			{/if}
		</div>
	{/snippet}
</Dialog>
