<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { tick, type Snippet } from 'svelte';
	import { LoaderCircle } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import ActionButton from '#lib/components/ui/ActionButton/ActionButton.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';

	/**
	 * Asks before an action. `onconfirm` may reject with a message to show and retry from; when it
	 * resolves the dialog closes, or swaps to `result` when one is given. `visual` receives whether
	 * the action is done, so a picture can react to it.
	 */
	type Props = {
		title: string;
		confirmLabel: string;
		cancelLabel?: string;
		/** Danger style for destructive actions. */
		destructive?: boolean;
		/** Holds the confirm button back, e.g. while the dialog is still gathering facts. */
		busy?: boolean;
		pendingDelayMs?: number;
		onconfirm: () => void | Promise<void>;
		onclose: () => void;
		visual?: Snippet<[boolean]>;
		description?: Snippet;
		content?: Snippet<[boolean]>;
		secondaryAction?: Snippet<[boolean]>;
		expanded?: boolean;
		/** Shown once the action has succeeded; replaces the question. */
		result?: { title: string; body: Snippet };
	};

	let {
		title,
		confirmLabel,
		cancelLabel = m.common_cancel(),
		destructive = false,
		busy = false,
		pendingDelayMs,
		onconfirm,
		onclose,
		visual,
		description,
		content,
		secondaryAction,
		expanded = false,
		result
	}: Props = $props();

	const id = $props.id();

	let pending = $state(false);
	let done = $state(false);
	let error = $state('');
	let head = $state<HTMLElement>();

	const easing = 'cubic-bezier(0.2, 0.8, 0.2, 1)';
	const calm = () => !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
	type Size = { width: number; height: number };
	let beforeResize: Size | undefined;

	/** Layout size, unlike a bounding rect it ignores a transform that is mid-animation. */
	const sizeOf = (frame?: HTMLElement | null): Size | undefined =>
		frame ? { width: frame.offsetWidth, height: frame.offsetHeight } : undefined;

	function frameOf() {
		return head?.closest<HTMLElement>('dialog');
	}

	function ease(frame: HTMLElement, from: Size, to: Size) {
		if (calm()) return;

		frame.animate?.(
			[
				{ width: `${from.width}px`, height: `${from.height}px` },
				{ width: `${to.width}px`, height: `${to.height}px` }
			],
			{ duration: 320, easing }
		);
	}

	// Opening or closing the customization eases the dialog between its two sizes.
	$effect.pre(() => {
		void expanded;
		beforeResize = sizeOf(frameOf());
	});
	$effect(() => {
		void expanded;

		const frame = frameOf();
		const from = beforeResize;

		beforeResize = undefined;
		if (frame && from) ease(frame, from, { width: frame.offsetWidth, height: frame.offsetHeight });
	});

	/** Swaps to the result; the dialog eases from its old size to the new one. */
	async function reveal() {
		const frame = frameOf();
		const from = sizeOf(frame);

		done = true;
		await tick();

		const to = sizeOf(frame);

		if (frame && from && to) ease(frame, from, to);
	}

	async function confirm(close: () => void) {
		if (pending) return false;

		pending = true;
		error = '';

		try {
			await onconfirm();

			if (result) await reveal();
			else close();
			return true;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : m.api_error_generic();
			return false;
		} finally {
			pending = false;
		}
	}
</script>

{#snippet header()}
	<div
		bind:this={head}
		class="grid content-center items-center gap-4 {visual
			? 'sm:grid-cols-[6rem_1fr] sm:gap-x-[1.125rem]'
			: ''}"
		aria-busy={pending}
	>
		{@render visual?.(done)}
		<div class="grid content-start gap-2">
			{#if done && result}
				<h2 id="{id}-title" class="animate-photo-in pr-10 text-2xl motion-reduce:animate-none">
					{result.title}
				</h2>
				<div class="grid animate-photo-in gap-3 motion-reduce:animate-none">
					{@render result.body()}
				</div>
			{:else}
				<h2 id="{id}-title" class="pr-10 text-xl">{title}</h2>
				{@render description?.()}
			{/if}
		</div>
	</div>
{/snippet}

{#snippet body()}
	{#if !(done && result)}
		{#if content}<div class="min-w-0 {expanded ? 'pt-4' : ''}">
				{@render content(pending)}
			</div>{/if}
		{#if error}
			<div class="pt-4">
				{#if pendingDelayMs !== undefined}<Toaster
						studio
						localError={{ message: error, dismiss: () => (error = '') }}
					/>
				{:else}<p role="alert" class="text-sm text-danger">{error}</p>{/if}
			</div>
		{/if}
	{/if}
{/snippet}

{#snippet buttons(close: () => void)}
	<div
		class="flex flex-wrap gap-2.5 pt-4 sm:justify-end max-sm:[&>*]:flex-1 {expanded
			? 'mt-4 border-t border-border'
			: ''}"
	>
		<Button variant="outline" disabled={pending} onclick={close}>{cancelLabel}</Button>
		{@render secondaryAction?.(pending)}
		{#if pendingDelayMs !== undefined}
			<ActionButton
				{pendingDelayMs}
				variant={destructive ? 'destructive' : 'default'}
				disabled={busy || pending}
				labels={{
					idle: confirmLabel,
					pending: destructive ? m.dialog_deleting() : m.dialog_working()
				}}
				onaction={() => confirm(close)}
			/>
		{:else}
			<Button
				variant={destructive ? 'destructive' : 'default'}
				disabled={pending || busy}
				onclick={() => void confirm(close)}
			>
				{#if pending}
					<LoaderCircle
						size={16}
						class="animate-spin motion-reduce:animate-none"
						aria-hidden="true"
					/>
					{destructive ? m.dialog_deleting() : m.dialog_working()}
				{:else}
					{confirmLabel}
				{/if}
			</Button>
		{/if}
	</div>
{/snippet}

<Dialog
	{onclose}
	dismissible={!pending}
	labelledby="{id}-title"
	class="m-auto {expanded && !(done && result)
		? 'w-[min(64rem,calc(100vw-2rem))]'
		: 'w-[min(32rem,calc(100vw-2rem))]'} rounded-dialog border border-border bg-panel text-foreground shadow-xl"
	{header}
	content={body}
	actions={done && result ? undefined : buttons}
/>
