<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { LoaderCircle, X } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';

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
		onconfirm: () => void | Promise<void>;
		onclose: () => void;
		visual?: Snippet<[boolean]>;
		description?: Snippet;
		/** Shown once the action has succeeded; replaces the question. */
		result?: { title: string; body: Snippet<[() => void]> };
	};

	let {
		title,
		confirmLabel,
		cancelLabel = m.common_cancel(),
		destructive = false,
		busy = false,
		onconfirm,
		onclose,
		visual,
		description,
		result
	}: Props = $props();

	const id = $props.id();

	let pending = $state(false);
	let done = $state(false);
	let error = $state('');

	async function confirm(close: () => void) {
		if (pending) return;

		pending = true;
		error = '';

		try {
			await onconfirm();

			if (result) done = true;
			else close();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : m.api_error_generic();
		} finally {
			pending = false;
		}
	}
</script>

<Dialog
	{onclose}
	dismissible={!pending}
	labelledby="{id}-title"
	class="m-auto w-[min(27.5rem,calc(100vw-2rem))] rounded-dialog border border-border bg-panel p-6 text-foreground shadow-xl"
>
	{#snippet children(close)}
		<div
			class="relative grid gap-4 {visual && !(done && result)
				? 'sm:grid-cols-[6rem_1fr] sm:gap-x-[1.125rem]'
				: ''}"
			aria-busy={pending}
		>
			{@render visual?.(done)}
			{#if done && result}
				<Button
					variant="ghost"
					size="sm"
					class="absolute -top-3 -right-3 min-h-9! px-2.5!"
					aria-label={m.action_close()}
					onclick={close}><X size={18} aria-hidden="true" /></Button
				>
				<h2 id="{id}-title" class="text-center text-xl">{result.title}</h2>
				{@render result.body(close)}
			{:else}
				<div class="grid content-start gap-2">
					<h2 id="{id}-title" class="text-xl">{title}</h2>
					{@render description?.()}
				</div>
				{#if error}<p role="alert" class="text-sm text-danger sm:col-span-2">{error}</p>{/if}
				<div class="flex gap-2.5 sm:col-span-2 sm:justify-end max-sm:[&>*]:flex-1">
					<Button variant="outline" disabled={pending} onclick={close}>{cancelLabel}</Button>
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
				</div>
			{/if}
		</div>
	{/snippet}
</Dialog>
