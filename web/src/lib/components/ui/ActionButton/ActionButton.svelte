<script lang="ts" module>
	import type { Snippet } from 'svelte';
	import type { ButtonVariant } from '#lib/components/ui/Button/Button.svelte';

	export type ActionButtonProps = {
		labels: { idle: string; pending: string; succeeded?: string };
		onaction: () => Promise<boolean>;
		confirmation?: { duration: number; cancelLabel: string; description: string };
		confirmationDuration?: number;
		onsettled?: () => void;
		disabled?: boolean;
		form?: string;
		variant?: ButtonVariant;
		icon?: Snippet;
		compactOnMobile?: boolean;
		class?: string;
		idleLabel?: string;
	};
</script>

<script lang="ts">
	import { Check, LoaderCircle, X } from '@lucide/svelte';
	import { onDestroy } from 'svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import ButtonFace from '#lib/components/ButtonFace.svelte';

	let {
		labels,
		onaction,
		confirmation,
		confirmationDuration = 1600,
		onsettled,
		disabled = false,
		form,
		variant = 'default',
		icon,
		compactOnMobile = false,
		class: className,
		idleLabel
	}: ActionButtonProps = $props();

	const warningId = $props.id();
	let state = $state<'idle' | 'armed' | 'pending' | 'confirmed'>('idle');
	let timer: ReturnType<typeof setTimeout> | undefined;
	let disposed = false;
	const busy = $derived(state === 'pending' || state === 'confirmed');
	const labelClass = $derived(compactOnMobile && icon ? 'sr-only sm:not-sr-only' : undefined);
	const label = $derived(
		state === 'armed'
			? confirmation?.cancelLabel
			: state === 'pending'
				? labels.pending
				: state === 'confirmed'
					? labels.succeeded
					: (idleLabel ?? labels.idle)
	);

	function reset(): void {
		clearTimeout(timer);
		timer = undefined;
		state = 'idle';
	}

	async function run(): Promise<void> {
		if (disabled) {
			reset();

			return;
		}

		state = 'pending';
		let succeeded = false;

		try {
			succeeded = await onaction();
		} catch {
			// The caller owns failure messaging.
		}

		if (disposed) return;

		if (!succeeded || !labels.succeeded) {
			reset();
			if (succeeded) onsettled?.();

			return;
		}

		state = 'confirmed';
		timer = setTimeout(() => {
			reset();
			onsettled?.();
		}, confirmationDuration);
	}

	export function activate(): void {
		if (disposed || disabled || busy) return;

		if (state === 'armed') {
			reset();
		} else if (confirmation) {
			state = 'armed';
			timer = setTimeout(() => void run(), confirmation.duration);
		} else {
			void run();
		}
	}

	onDestroy(() => {
		disposed = true;
		clearTimeout(timer);
	});
</script>

<span class="relative inline-grid motion-reduce:[&_*]:transition-none">
	<Button
		disabled={disabled && !busy}
		{form}
		{variant}
		type={form ? 'submit' : 'button'}
		aria-label={label}
		aria-disabled={busy || undefined}
		aria-describedby={state === 'armed' ? warningId : undefined}
		class="relative inline-grid overflow-hidden aria-disabled:cursor-default {compactOnMobile &&
		icon
			? 'max-sm:min-w-11 max-sm:px-0 max-sm:[&>span[aria-hidden]]:grid-cols-1 max-sm:[&>span[aria-hidden]]:gap-0 max-sm:[&>span[aria-hidden]>span:last-child]:hidden'
			: ''} {className ?? ''}"
		onclick={(event) => {
			if (busy) event.preventDefault();
			else if (!form) activate();
		}}
		onkeydown={(event) => {
			if (event.key !== 'Escape' || state !== 'armed') return;

			event.preventDefault();
			event.stopPropagation();
			reset();
		}}
	>
		<ButtonFace shown={state === 'idle'} {icon}>
			<span class={labelClass}>{labels.idle}</span>
		</ButtonFace>
		{#if confirmation}
			<ButtonFace shown={state === 'armed'}>
				{#snippet icon()}
					<X size={16} aria-hidden="true" />
				{/snippet}
				<span class={labelClass}>{confirmation.cancelLabel}</span>
			</ButtonFace>
		{/if}
		<ButtonFace shown={state === 'pending'}>
			{#snippet icon()}
				<LoaderCircle
					size={16}
					class="animate-spin motion-reduce:animate-none"
					aria-hidden="true"
				/>
			{/snippet}
			<span class={labelClass}>{labels.pending}</span>
		</ButtonFace>
		{#if labels.succeeded}
			<ButtonFace shown={state === 'confirmed'}>
				{#snippet icon()}
					<Check size={16} aria-hidden="true" />
				{/snippet}
				<span class={labelClass}>{labels.succeeded}</span>
			</ButtonFace>
		{/if}
		{#if state === 'armed' && confirmation}
			<span
				aria-hidden="true"
				class="countdown absolute inset-x-0 bottom-0 h-1 origin-left bg-danger motion-reduce:animate-none"
				style:--duration={`${confirmation.duration}ms`}
			></span>
		{/if}
	</Button>
	{#if state === 'armed' && confirmation}
		<span
			id={warningId}
			class="absolute top-full right-0 z-10 mt-1 w-48 rounded-control border border-border bg-panel p-2 text-sm text-foreground shadow-sm transition-opacity duration-150 starting:opacity-0"
		>
			{confirmation.description}
		</span>
	{/if}
	<span role="status" class="sr-only">
		{state === 'armed' ? confirmation?.description : state === 'idle' ? '' : label}
	</span>
</span>

<style>
	.countdown {
		animation: countdown var(--duration) linear both;
	}

	@media (prefers-reduced-motion: reduce) {
		.countdown {
			animation: none;
		}
	}

	@keyframes countdown {
		to {
			transform: scaleX(0);
		}
	}
</style>
