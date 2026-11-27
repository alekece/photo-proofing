<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import type { HTMLDialogAttributes } from 'svelte/elements';
	import CloseButton from '#lib/components/ui/CloseButton/CloseButton.svelte';
	import { cn } from '#lib/utils.js';

	/**
	 * Native modal `<dialog>` opened on mount. `children` receives `close`, which dismisses it.
	 * `onclose` is called from the native `close` event (Escape, `close()`, Android back), once per
	 * closure; only where `dialog.close` is missing (jsdom) does `close()` call it directly.
	 * Focus returns to the previously focused element on unmount.
	 */
	type Props = Omit<HTMLDialogAttributes, 'children' | 'onclose' | 'aria-labelledby'> & {
		onclose: () => void;
		labelledby?: string;
		label?: string;
		/** Non-modal dialogs stay in the page flow, so a parent can contain them. */
		modal?: boolean;
		/** Prevents user dismissal while a confirmed action runs. */
		dismissible?: boolean;
		closeLabel?: string;
		closeClass?: string;
		focusClose?: boolean;
		/** Reverse the entrance on dismissal (default), but not after completing a form. */
		formMotion?: boolean;
		/** Appears and leaves by fading only, without the pop-in scale, for full-screen surfaces. */
		fade?: boolean;
		children?: Snippet<[dismiss: () => void, complete: () => void]>;
		header?: Snippet;
		content?: Snippet;
		actions?: Snippet<[dismiss: () => void, complete: () => void]>;
	};

	let {
		onclose,
		labelledby,
		label,
		modal = true,
		dismissible = true,
		closeLabel,
		closeClass,
		focusClose = false,
		formMotion = true,
		fade = false,
		header,
		content,
		actions,
		class: className,
		onkeydown,
		oncancel,
		children,
		'data-slot': dataSlot = 'dialog',
		...restProps
	}: Props = $props();
	const structured = $derived(
		header !== undefined || content !== undefined || actions !== undefined
	);

	let dialog: HTMLDialogElement;
	let closeButton = $state<HTMLButtonElement | null>(null);
	let motion: Animation | undefined;
	let dismissing = false;

	onMount(() => {
		const opener = document.activeElement;

		// jsdom and very old browsers lack showModal; fall back to a plain open dialog.
		if (modal && typeof dialog.showModal === 'function') dialog.showModal();
		else if (!modal && typeof dialog.show === 'function') dialog.show();
		else dialog.setAttribute('open', '');

		if (focusClose) closeButton?.focus();

		if (modal && !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			motion = fade
				? dialog.animate?.([{ opacity: 0 }, { opacity: 1 }], {
						duration: 220,
						easing: 'ease-out'
					})
				: dialog.animate?.(
						[
							{ opacity: 0.3, transform: 'scale(.975)' },
							{ opacity: 1, transform: 'scale(1.012)', offset: 0.6 },
							{ opacity: 1, transform: 'scale(1.012)', offset: 0.7 },
							{ opacity: 1, transform: 'none' }
						],
						{ duration: 250, easing: 'cubic-bezier(0.2, 0, 0, 1)' }
					);
		}

		return () => {
			motion?.cancel();
			if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
		};
	});

	// Closing the dialog before it unmounts is what returns focus to the button that opened it.
	// With native dialogs, the `close` event reports every closure (including Android back), so
	// the parent is notified there; the fallback has no such event.
	function close() {
		if (typeof dialog.close === 'function') dialog.close();
		else onclose();
	}

	function dismiss() {
		if (dismissing) return;
		if (!formMotion || !motion || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			close();

			return;
		}

		dismissing = true;
		dialog.setAttribute('inert', '');
		motion.cancel();
		// By default a quick swell, then it shrinks away while fading out; `fade` only fades.
		const leaving = fade
			? dialog.animate([{ opacity: 1 }, { opacity: 0 }], {
					duration: 200,
					easing: 'ease-in',
					fill: 'forwards'
				})
			: dialog.animate(
					[
						{ opacity: 1, transform: 'scale(1)' },
						{ opacity: 1, transform: 'scale(1.025)', offset: 0.3 },
						{ opacity: 0, transform: 'scale(0.9)' }
					],
					{ duration: 200, easing: 'cubic-bezier(0.4, 0, 1, 1)', fill: 'forwards' }
				);

		void leaving.finished.then(close, () => {});
	}

	function handleKeydown(event: KeyboardEvent & { currentTarget: HTMLDialogElement }) {
		if (event.key === 'Escape') {
			event.preventDefault();
			// A dialog nested in another must not dismiss its parent too.
			event.stopPropagation();
			if (dismissible) dismiss();

			return;
		}

		onkeydown?.(event);
	}
</script>

<dialog
	bind:this={dialog}
	data-slot={dataSlot}
	aria-labelledby={labelledby}
	aria-label={label}
	onkeydown={handleKeydown}
	oncancel={(event) => {
		if (!dismissible) event.preventDefault();
		else {
			oncancel?.(event);
			if (formMotion && motion && !event.defaultPrevented) {
				event.preventDefault();
				dismiss();
			}
		}
	}}
	{onclose}
	class={cn(
		'backdrop:bg-overlay',
		structured && 'max-h-[calc(100dvh-2rem)] overflow-hidden',
		className
	)}
	{...restProps}
>
	{#if structured}
		<div
			data-slot="dialog-layout"
			class="grid h-full max-h-[calc(100dvh-2rem)] min-h-0 grid-rows-[auto_minmax(0,1fr)_auto] p-5 sm:p-6"
		>
			{#if header}
				<div data-slot="dialog-header" class="min-w-0">
					{@render header()}
				</div>
			{/if}
			{#if content}
				<div data-slot="dialog-content" class="min-h-0 overflow-y-auto overscroll-contain">
					{@render content()}
				</div>
			{/if}
			{#if actions}
				<div data-slot="dialog-actions" class="min-w-0">
					{@render actions(dismiss, close)}
				</div>
			{/if}
		</div>
	{:else}
		{@render children?.(dismiss, close)}
	{/if}
	{#if modal}
		<CloseButton
			bind:ref={closeButton}
			label={closeLabel}
			disabled={!dismissible}
			onclick={dismiss}
			class={cn('absolute top-2 right-2 z-10', closeClass)}
		/>
	{/if}
</dialog>
