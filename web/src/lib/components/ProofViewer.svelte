<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		ChevronLeft,
		ChevronRight,
		Download,
		Heart,
		LoaderCircle,
		MessageSquare,
		MessageSquareOff,
		X
	} from '@lucide/svelte';
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
		trailing,
		thread,
		commentCount,
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
		/** Actions that follow the comments button, such as the destructive ones. */
		trailing?: Snippet<[Proof]>;
		thread?: Snippet<[Proof]>;
		commentCount?: (id: string) => number;
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
	let commentsButton = $state<HTMLButtonElement>();
	const proof = $derived(proofs[index]);
	const status = $derived(proof.status ?? 'ready');
	const client = $derived(title !== undefined);
	// The photographer reads feedback while reviewing, so the thread starts open; the client's starts closed.
	// svelte-ignore state_referenced_locally
	let threadOpen = $state(!client && !!thread);

	const go = (to: number) => (index = Math.min(Math.max(to, 0), proofs.length - 1));

	onMount(() => closeButton.focus());

	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLDialogElement }) {
		const dialog = event.currentTarget;
		if ((event.target as HTMLElement).closest('dialog') !== dialog) return;
		if (event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement) {
			if (event.key !== 'Tab') return;
		}

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
				const items = [
					...dialog.querySelectorAll<HTMLElement>(
						'button:not(:disabled), a[href], textarea:not(:disabled), input:not(:disabled)'
					)
				];
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
		{isSelected(proof.id) ? m.viewer_favorite() : m.viewer_add_favorite()}
	</FavoriteButton>
{/snippet}

<!-- Key handling lives on the dialog so it applies wherever focus is inside it. -->
<Dialog
	{onclose}
	{onkeydown}
	onkeydowncapture={(event) => {
		if (
			event.key === 'Escape' &&
			threadOpen &&
			(event.target as HTMLElement).closest('dialog') === event.currentTarget
		) {
			event.preventDefault();
			event.stopImmediatePropagation();
			threadOpen = false;
			commentsButton?.focus();
		}
	}}
	oncancel={(event) => {
		if (threadOpen) {
			event.preventDefault();
			threadOpen = false;
			commentsButton?.focus();
		}
	}}
	label={m.viewer_label()}
	aria-modal="true"
	class={cn(
		'proof-viewer m-0 h-dvh max-h-dvh w-screen max-w-screen animate-photo-in overflow-hidden border-0 bg-dark p-0 text-[0.875rem] font-medium text-[#efe6da] open:grid',
		client
			? 'grid-rows-[60px_1fr_44px] max-[700px]:grid-rows-[56px_1fr_104px]'
			: 'grid-rows-[60px_1fr_44px] max-[700px]:grid-rows-[56px_1fr_20px]',
		className,
		threadOpen &&
			'comments-open max-[700px]:grid-rows-[56px_minmax(0,1fr)_minmax(0,45dvh)_104px] min-[701px]:grid-cols-[minmax(0,1fr)_minmax(280px,360px)]'
	)}
>
	{#snippet children(close)}
		<div
			class="col-span-full flex min-w-0 items-center gap-3.5 px-5 max-[700px]:gap-2 max-[700px]:px-3"
		>
			{#if client}
				<span class="truncate font-serif text-[1.1875rem] text-panel max-[700px]:hidden"
					>{title}</span
				>
			{:else}
				<span class="font-mono text-[0.8125rem] whitespace-nowrap text-[#a8998a]">{proof.alt}</span>
			{/if}
			<span aria-live="polite" class="whitespace-nowrap text-[#a8998a] tabular-nums"
				>{m.viewer_position({ position: index + 1, total: proofs.length })}</span
			>
			<span class="flex-1"></span>
			{#if favoriteCount !== undefined}
				<span class="flex items-center gap-1.5 font-semibold text-panel">
					<Heart size={17} aria-hidden="true" fill="currentColor" class="text-[#e4573b]" />
					<span class="sr-only">{m.viewer_favorites()}</span>{favoriteCount}
				</span>
			{/if}
			{#if client && wide.current && status === 'ready' && (onselect || readonly) && !proof.downloadUrl}
				{@render heartPill(false)}
			{/if}
			{#if client && wide.current && status === 'ready' && proof.downloadUrl}
				<a
					href={proof.downloadUrl}
					download
					class="inline-flex h-10 items-center gap-2 rounded-full bg-panel px-3.5 text-[0.84375rem] font-semibold whitespace-nowrap text-foreground focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
				>
					<Download size={17} aria-hidden="true" />{m.action_download()}
				</a>
			{/if}
			{@render actions?.(proof)}
			{#if thread}
				{@const count = commentCount?.(proof.id) ?? 0}
				<button
					type="button"
					bind:this={commentsButton}
					aria-label={m.comments_title()}
					aria-expanded={threadOpen}
					onclick={() => (threadOpen = !threadOpen)}
					class="inline-flex min-h-10 min-w-10 cursor-pointer items-center justify-center gap-1.5 rounded-full bg-white/8 px-3 text-panel hover:bg-white/14 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
				>
					<span class="relative mr-2 inline-grid">
						{#if threadOpen}
							<MessageSquareOff size={18} aria-hidden="true" />
						{:else}
							<MessageSquare size={18} aria-hidden="true" />
						{/if}
						{#if count > 0}
							<span
								class="absolute -top-2 -right-2.5 grid h-4 min-w-4 place-items-center rounded-full bg-[#e4573b] px-1 text-[0.6875rem] leading-none font-semibold text-white tabular-nums"
								>{count}</span
							>
						{/if}
					</span>
					<span class="max-[700px]:hidden"
						>{threadOpen ? m.comments_hide() : m.comments_title()}</span
					>
				</button>
			{/if}
			{@render trailing?.(proof)}
			<button
				type="button"
				bind:this={closeButton}
				onclick={close}
				aria-label={m.viewer_close()}
				class="inline-grid size-10 cursor-pointer place-items-center rounded-full bg-white/8 text-[#efe6da] transition-colors hover:bg-white/14 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus"
			>
				<X size={20} aria-hidden="true" />
			</button>
		</div>
		<div
			class="photo-stage relative flex min-h-0 items-center justify-center overflow-hidden px-[84px] max-[700px]:px-0"
		>
			{#if status === 'ready'}
				{#key proof.id}
					<span class="relative flex animate-photo-in">
						<img
							src={proof.previewUrl}
							alt={proof.alt}
							class={cn(
								'block max-h-[calc(100dvh-124px)] max-w-[calc(100vw-168px)] rounded-[2px] object-contain shadow-[0_30px_80px_rgb(0_0_0/0.5)] max-[700px]:max-h-[calc(100dvh-160px)] max-[700px]:max-w-screen max-[700px]:rounded-none max-[700px]:shadow-none',
								threadOpen &&
									'max-[700px]:max-h-[calc(55dvh-160px)] min-[701px]:max-w-[calc(100vw-528px)]'
							)}
						/>
						{#if !client && readonly && isSelected(proof.id)}
							<span
								role="img"
								aria-label={m.viewer_marked()}
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
					{m.viewer_processing()}
				</p>
			{:else}
				<div
					class="grid aspect-3/2 w-[min(560px,100%)] place-items-center rounded-md border border-dashed border-[#5a4d43] bg-[#241e1a] text-center text-[#a8998a]"
				>
					<div>
						<p role="alert" class="mb-3 text-[0.9375rem] font-semibold text-warning-on-dark">
							{m.viewer_failed()}
						</p>
						{#if onretry}
							<ViewerButton onclick={() => onretry(proof.id)}>{m.viewer_try_again()}</ViewerButton>
						{/if}
					</div>
				</div>
			{/if}
			<button
				type="button"
				class="{arrow} left-5"
				aria-disabled={index === 0}
				onclick={() => go(index - 1)}
				aria-label={m.viewer_previous()}><ChevronLeft size={22} aria-hidden="true" /></button
			>
			<button
				type="button"
				class="{arrow} right-5"
				aria-disabled={index === proofs.length - 1}
				onclick={() => go(index + 1)}
				aria-label={m.viewer_next()}><ChevronRight size={22} aria-hidden="true" /></button
			>
		</div>
		{#if threadOpen && thread}
			<aside
				class="min-h-0 overflow-hidden border-panel/20 bg-dark py-4 pr-6 pl-4 max-[700px]:row-start-3 max-[700px]:border-t min-[701px]:col-start-2 min-[701px]:row-span-2 min-[701px]:row-start-2 min-[701px]:border-l"
			>
				{@render thread(proof)}
			</aside>
		{/if}
		<div class="viewer-footer flex items-center justify-center gap-3">
			{#if client && !wide.current && status === 'ready'}
				{#if proof.downloadUrl}
					<a
						href={proof.downloadUrl}
						download
						class="{pill} bg-panel pr-[22px] pl-[22px] text-foreground"
					>
						<Download size={20} aria-hidden="true" />{m.action_download()}
					</a>
				{:else if onselect || readonly}
					{@render heartPill(true)}
				{/if}
			{/if}
		</div>
	{/snippet}
</Dialog>

<style>
	/* A modal dialog leaves the page scrollable behind it, and its scrollbar would show over the photo. */
	:global(html:has(dialog.proof-viewer[open])) {
		overflow: hidden;
	}

	@media (min-width: 560px) and (max-width: 700px) and (max-height: 500px) {
		:global(dialog.comments-open) {
			grid-template-columns: minmax(0, 1fr) minmax(280px, 45%);
			grid-template-rows: 56px minmax(0, 1fr);
		}

		:global(dialog.comments-open) .photo-stage {
			padding-inline: 24px;
		}

		:global(dialog.comments-open) img {
			max-width: calc(55vw - 48px);
			max-height: calc(100dvh - 100px);
		}

		:global(dialog.comments-open) aside {
			grid-column: 2;
			grid-row: 2;
			border-top: 0;
			border-left: 1px solid rgb(255 255 255 / 20%);
		}

		:global(dialog.comments-open) .viewer-footer {
			display: none;
		}
	}
</style>
