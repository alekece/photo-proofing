<script lang="ts">
	import { onMount } from 'svelte';
	import Dialog from '#lib/components/ui/dialog/dialog.svelte';
	import type { Proof } from '../gallery/types';

	let { proofs, index = $bindable(0), onclose }: {
		proofs: Proof[];
		index?: number;
		onclose: () => void;
	} = $props();

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
<!-- Key handling lives on the dialog so it applies wherever focus is inside it. -->
<Dialog
	{onclose}
	{onkeydown}
	label="Proof viewer"
	aria-modal="true"
	class="m-0 h-dvh max-h-dvh w-screen max-w-screen border-0 bg-viewer p-4 text-viewer-foreground open:flex open:flex-col open:gap-4"
>
	{#snippet children(close)}
		<div class="bar">
			<span class="count" aria-live="polite">{index + 1} of {proofs.length}</span>
			<button type="button" bind:this={closeButton} onclick={close} aria-label="Close viewer">
				Close
			</button>
		</div>
		<div class="stage">
			<button type="button" onclick={() => go(index - 1)} aria-label="Previous proof">‹</button>
			<img src={proof.previewUrl} alt={proof.alt} />
			<button type="button" onclick={() => go(index + 1)} aria-label="Next proof">›</button>
		</div>
	{/snippet}
</Dialog>

<style>
	.bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.stage {
		flex: 1;
		min-height: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	img {
		flex: 1;
		min-width: 0;
		max-height: 100%;
		object-fit: contain;
	}
	button {
		min-width: 2.75rem;
		min-height: 2.75rem;
		padding: 0 1rem;
		border: 2px solid var(--color-viewer-foreground);
		border-radius: 0.25rem;
		background: var(--color-viewer-control);
		color: var(--color-viewer-foreground);
		font: inherit;
		font-size: 1.25rem;
		cursor: pointer;
	}
	button:focus-visible {
		outline: 3px solid var(--color-viewer-focus);
		outline-offset: 2px;
	}
</style>
