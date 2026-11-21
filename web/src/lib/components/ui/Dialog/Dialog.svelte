<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import type { HTMLDialogAttributes } from 'svelte/elements';
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
		/** Holds Escape and native cancellation back while a confirmed action runs. */
		dismissible?: boolean;
		/** A short entrance from the focused opener, reversed on dismissal only. */
		formMotion?: boolean;
		children: Snippet<[dismiss: () => void, complete: () => void]>;
	};

	let {
		onclose,
		labelledby,
		label,
		modal = true,
		dismissible = true,
		formMotion = false,
		class: className,
		onkeydown,
		oncancel,
		children,
		'data-slot': dataSlot = 'dialog',
		...restProps
	}: Props = $props();

	let dialog: HTMLDialogElement;
	let motion: Animation | undefined;
	let dismissing = false;

	onMount(() => {
		const opener = document.activeElement;
		const source =
			opener instanceof HTMLElement && opener !== document.body
				? opener.getBoundingClientRect()
				: null;

		// jsdom and very old browsers lack showModal; fall back to a plain open dialog.
		if (modal && typeof dialog.showModal === 'function') dialog.showModal();
		else if (!modal && typeof dialog.show === 'function') dialog.show();
		else dialog.setAttribute('open', '');

		if (
			formMotion &&
			source &&
			modal &&
			!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
		) {
			const target = dialog.getBoundingClientRect();
			const x = source.x + (source.width - target.width) / 2 - target.x;
			const y = source.y + (source.height - target.height) / 2 - target.y;
			const clamp = (distance: number) => Math.max(-60, Math.min(60, distance));

			motion = dialog.animate?.(
				[
					{ opacity: 0.3, transform: `translate(${clamp(x)}px, ${clamp(y)}px) scale(.96)` },
					{ opacity: 1, transform: 'none' }
				],
				{ duration: 150, easing: 'ease-out', fill: 'both' }
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
		if (!motion || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			close();

			return;
		}

		dismissing = true;
		dialog.setAttribute('inert', '');
		motion.reverse();
		void motion.finished.then(close, () => {});
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
			if (motion && !event.defaultPrevented) {
				event.preventDefault();
				dismiss();
			}
		}
	}}
	{onclose}
	class={cn('backdrop:bg-overlay', className)}
	{...restProps}
>
	{@render children(dismiss, close)}
</dialog>
