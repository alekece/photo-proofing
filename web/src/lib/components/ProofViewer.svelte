<script lang="ts">
	import { ChevronLeft, ChevronRight, LoaderCircle, X } from '@lucide/svelte';
	import { onMount, type Snippet } from 'svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import { cn } from '#lib/utils.js';
	import type { Proof } from '../gallery/types';
	import FavoriteButton from './FavoriteButton.svelte';

	let {
		proofs,
		index = $bindable(0),
		isSelected = () => false,
		class: className,
		contained = false,
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
		/** Opens inside the nearest positioned ancestor instead of over the whole page. */
		contained?: boolean;
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

	const controlClass =
		'grid min-h-11 min-w-11 cursor-pointer place-items-center rounded-full border border-viewer-foreground/30 bg-viewer-control text-viewer-foreground focus-visible:outline-3 focus-visible:outline-offset-2! focus-visible:outline-viewer-focus! disabled:cursor-default disabled:opacity-40 aria-disabled:cursor-default aria-disabled:opacity-40';

	let closeButton: HTMLButtonElement;
	const proof = $derived(proofs[index]);
	const status = $derived(proof.status ?? 'ready');
	const hearted = $derived(onselect !== undefined || readonly);

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
				const items = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled)')];
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

<!-- Key handling lives on the dialog so it applies wherever focus is inside it. -->
<Dialog
	{onclose}
	{onkeydown}
	label="Proof viewer"
	modal={!contained}
	aria-modal="true"
	class={cn(
		'm-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-viewer p-3 text-viewer-foreground open:flex open:flex-col open:gap-3 sm:p-4',
		contained && 'absolute inset-0 h-full max-h-full w-full max-w-full',
		className
	)}
>
	{#snippet children(close)}
		<div class="flex flex-wrap items-center gap-x-3 gap-y-2">
			<span class="min-w-0 flex-1 truncate font-medium">{proof.alt}</span>
			<span aria-live="polite" class="text-sm text-viewer-foreground/70"
				>{index + 1} of {proofs.length}</span
			>
			{@render actions?.(proof)}
			<button
				type="button"
				class={controlClass}
				bind:this={closeButton}
				onclick={close}
				aria-label="Close viewer"
			>
				<X size={20} aria-hidden="true" />
			</button>
		</div>
		<div class="flex min-h-0 flex-1 items-center gap-2">
			<button
				type="button"
				class={controlClass}
				aria-disabled={index === 0}
				onclick={() => go(index - 1)}
				aria-label="Previous proof"><ChevronLeft size={22} aria-hidden="true" /></button
			>
			<div class="relative grid h-full min-w-0 flex-1 place-items-center">
				{#if status === 'ready'}
					{#key proof.id}
						<img
							src={proof.previewUrl}
							alt={proof.alt}
							class="max-h-full max-w-full animate-photo-in object-contain"
						/>
					{/key}
					{#if hearted}
						<FavoriteButton
							selected={isSelected(proof.id)}
							label="Favorite proof {index + 1}"
							disabled={!onselect}
							onclick={() => onselect?.(proof.id)}
							class={cn(
								controlClass,
								'absolute top-2 right-2',
								!onselect && 'disabled:opacity-100'
							)}
						/>
					{/if}
				{:else if status === 'processing'}
					<p role="status" class="flex items-center gap-2 text-viewer-foreground/80">
						<LoaderCircle size={20} class="animate-spin-slow" aria-hidden="true" />
						Processing…
					</p>
				{:else}
					<div class="grid justify-items-center gap-3 text-center">
						<p role="alert">This photo could not be processed</p>
						{#if onretry}
							<Button
								variant="outline"
								size="sm"
								class="border-viewer-foreground/50 text-viewer-foreground"
								onclick={() => onretry(proof.id)}>Try again</Button
							>
						{/if}
					</div>
				{/if}
			</div>
			<button
				type="button"
				class={controlClass}
				aria-disabled={index === proofs.length - 1}
				onclick={() => go(index + 1)}
				aria-label="Next proof"><ChevronRight size={22} aria-hidden="true" /></button
			>
		</div>
	{/snippet}
</Dialog>
