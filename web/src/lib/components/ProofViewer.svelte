<script lang="ts">
	import { onMount } from 'svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import { cn } from '#lib/utils.js';
	import type { Proof } from '../gallery/types';

	let {
		proofs,
		index = $bindable(0),
		class: className,
		contained = false,
		onclose
	}: {
		proofs: Proof[];
		index?: number;
		class?: string;
		/** Opens inside the nearest positioned ancestor instead of over the whole page. */
		contained?: boolean;
		onclose: () => void;
	} = $props();

	const controlClass =
		'min-h-11 min-w-11 cursor-pointer rounded-sm border-2 border-viewer-foreground bg-viewer-control px-4 text-[1.25rem] text-viewer-foreground focus-visible:outline-3 focus-visible:outline-offset-2! focus-visible:outline-viewer-focus!';

	let closeButton: HTMLButtonElement;
	const proof = $derived(proofs[index]);

	const go = (to: number) => (index = (to + proofs.length) % proofs.length);

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
				const items = [...dialog.querySelectorAll<HTMLElement>('button')];
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
		'm-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-viewer p-4 text-viewer-foreground open:flex open:flex-col open:gap-4',
		contained && 'absolute inset-0 h-full max-h-full w-full max-w-full',
		className
	)}
>
	{#snippet children(close)}
		<div class="flex items-center justify-between">
			<span aria-live="polite">{index + 1} of {proofs.length}</span>
			<button
				type="button"
				class={controlClass}
				bind:this={closeButton}
				onclick={close}
				aria-label="Close viewer"
			>
				Close
			</button>
		</div>
		<div class="flex min-h-0 flex-1 items-center gap-2">
			<button
				type="button"
				class={controlClass}
				onclick={() => go(index - 1)}
				aria-label="Previous proof">‹</button
			>
			<img
				src={proof.previewUrl}
				alt={proof.alt}
				class="max-h-full min-w-0 flex-1 object-contain"
			/>
			<button
				type="button"
				class={controlClass}
				onclick={() => go(index + 1)}
				aria-label="Next proof">›</button
			>
		</div>
	{/snippet}
</Dialog>
