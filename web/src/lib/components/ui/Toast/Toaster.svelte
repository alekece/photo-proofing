<script lang="ts">
	import { CheckCircle2, CircleAlert, X } from '@lucide/svelte';
	import { cubicIn } from 'svelte/easing';
	import { fly } from 'svelte/transition';
	import { m } from '#lib/paraglide/messages.js';
	import { toasts } from './toasts.svelte.js';
</script>

<div class="pointer-events-none fixed inset-x-0 bottom-4 z-50 grid justify-items-center gap-2 px-4">
	{#each toasts.items as toast (toast.id)}
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
					}}>{m.toast_retry()}</button
				>
			{/if}
			<button type="button" aria-label={m.toast_dismiss()} onclick={() => toasts.dismiss(toast.id)}>
				<X size={16} aria-hidden="true" />
			</button>
		</div>
	{/each}
</div>
