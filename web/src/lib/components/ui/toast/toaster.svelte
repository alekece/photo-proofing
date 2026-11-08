<script lang="ts">
	import { Check, CircleAlert, X } from '@lucide/svelte';
	import { cubicIn } from 'svelte/easing';
	import { fly } from 'svelte/transition';
	import { toasts } from './toasts.svelte.js';
</script>

<div class="pointer-events-none fixed inset-x-0 bottom-4 z-50 grid justify-items-center gap-2 px-4">
	{#each toasts.items as toast (toast.id)}
		<div
			role={toast.kind === 'error' ? 'alert' : 'status'}
			data-slot="toast"
			out:fly={{ y: 8, duration: 200, easing: cubicIn }}
			class="pointer-events-auto relative flex max-w-md animate-toast-in items-center gap-3 overflow-hidden rounded-full px-4 py-2 text-sm text-inverse shadow-lg {toast.kind ===
			'error'
				? 'bg-danger'
				: 'bg-foreground'}"
		>
			{#if toast.kind === 'error'}
				<CircleAlert size={18} aria-hidden="true" />
			{:else}
				<Check size={18} aria-hidden="true" class="text-[#8fd19e]" />
			{/if}
			<span>{toast.message}</span>
			{#if toast.retry}
				<button
					type="button"
					class="font-semibold underline"
					onclick={() => {
						toast.retry?.();
						toasts.dismiss(toast.id);
					}}>Retry</button
				>
			{/if}
			<button type="button" aria-label="Dismiss" onclick={() => toasts.dismiss(toast.id)}>
				<X size={16} aria-hidden="true" />
			</button>
			{#if toast.duration}
				<span
					data-slot="toast-countdown"
					aria-hidden="true"
					class="toast-countdown absolute inset-x-0 bottom-0 h-0.5 origin-left bg-white/50"
					style="--toast-ms: {toast.duration}ms"
				></span>
			{/if}
		</div>
	{/each}
</div>
