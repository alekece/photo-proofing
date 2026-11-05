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
		children: Snippet<[() => void]>;
	};

	let {
		onclose,
		labelledby,
		label,
		modal = true,
		class: className,
		onkeydown,
		children,
		'data-slot': dataSlot = 'dialog',
		...restProps
	}: Props = $props();

	let dialog: HTMLDialogElement;

	onMount(() => {
		const opener = document.activeElement;

		// jsdom and very old browsers lack showModal; fall back to a plain open dialog.
		if (modal && typeof dialog.showModal === 'function') dialog.showModal();
		else if (!modal && typeof dialog.show === 'function') dialog.show();
		else dialog.setAttribute('open', '');

		return () => {
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

	function handleKeydown(event: KeyboardEvent & { currentTarget: HTMLDialogElement }) {
		if (event.key === 'Escape') {
			event.preventDefault();
			// A dialog nested in another must not dismiss its parent too.
			event.stopPropagation();
			close();

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
	{onclose}
	class={cn('backdrop:bg-overlay', className)}
	{...restProps}
>
	{@render children(close)}
</dialog>
