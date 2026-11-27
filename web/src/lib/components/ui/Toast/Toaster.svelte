<script lang="ts">
	import { CheckCircle2, CircleAlert, X } from '@lucide/svelte';
	import { cubicIn } from 'svelte/easing';
	import { fly } from 'svelte/transition';
	import { m } from '#lib/paraglide/messages.js';
	import { toasts } from './toasts.svelte.js';
	let {
		studio = false,
		localError
	}: { studio?: boolean; localError?: { message: string; dismiss: () => void } } = $props();
	const items = $derived(
		localError
			? [{ id: -1, kind: 'error' as const, message: localError.message, retry: undefined }]
			: toasts.items
	);
</script>

<div
	class="pointer-events-none fixed z-50 grid gap-2 px-4 {studio
		? 'inset-x-0 top-4 justify-items-end'
		: 'inset-x-0 bottom-4 justify-items-center'}"
>
	{#each items as toast (toast.id)}
		{@const isSuccess = toast.kind === 'success'}
		<div
			role={isSuccess ? 'status' : 'alert'}
			data-slot="toast"
			out:fly={{ y: 8, duration: 200, easing: cubicIn }}
			class="pointer-events-auto flex max-w-md animate-toast-in items-center gap-3 rounded-full px-4 py-2 text-sm shadow-lg {isSuccess
				? 'bg-foreground text-background'
				: 'bg-danger text-inverse'}"
		>
			{#if isSuccess}
				<CheckCircle2 size={18} aria-hidden="true" />
			{:else}
				<CircleAlert size={18} aria-hidden="true" />
			{/if}
			<span>{toast.message}</span>
			{#if toast.retry}
				<button
					type="button"
					class="font-semibold underline"
					onclick={() => {
						toast.retry?.();
						toasts.dismiss(toast.id);
					}}
				>
					{m.toast_retry()}
				</button>
			{/if}
			<button
				type="button"
				class="relative grid size-9 shrink-0 place-items-center rounded-full [--icon-hover-color:rgb(255_255_255/0.1)] max-sm:hover:bg-white/10 sm:icon-action"
				aria-label={m.toast_dismiss()}
				onclick={() => (localError ? localError.dismiss() : toasts.dismiss(toast.id))}
			>
				<X size={16} aria-hidden="true" />
			</button>
		</div>
	{/each}
</div>
