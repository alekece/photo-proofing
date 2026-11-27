<script lang="ts" module>
	import type { Snippet } from 'svelte';
	import type { ButtonVariant } from '#lib/components/ui/Button/Button.svelte';

	export type ActionButtonProps = {
		labels: { idle: string; pending: string; succeeded?: string };
		onaction: () => Promise<boolean>;
		confirmation?: {
			duration: number;
			cancelLabel: string;
			cancelAccessibleLabel?: string;
			description: string;
		};
		confirmationDuration?: number;
		/** Delay visual feedback, not dispatch; a slow write shows only the wheel. */
		pendingDelayMs?: number;
		/** Minimum time the visible spinner stays on screen, in milliseconds. Defaults to 100. */
		minPendingMs?: number;
		onsettled?: () => void;
		disabled?: boolean;
		form?: string;
		variant?: ButtonVariant;
		icon?: Snippet;
		/** Shows only the icon until the `lg` breakpoint, where the label fits beside it. */
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
		pendingDelayMs,
		minPendingMs = 100,
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
	let phase = $state<'idle' | 'armed' | 'pending' | 'confirmed'>('idle');
	let timer: ReturnType<typeof setTimeout> | undefined;
	let disposed = false;
	let showPending = $state(false);
	const busy = $derived(phase === 'pending' || phase === 'confirmed');
	const labelClass = $derived(compactOnMobile && icon ? 'sr-only lg:not-sr-only' : undefined);
	const label = $derived(
		phase === 'armed'
			? (confirmation?.cancelAccessibleLabel ?? confirmation?.cancelLabel)
			: phase === 'pending' && showPending
				? pendingDelayMs === undefined
					? labels.pending
					: (idleLabel ?? labels.idle)
				: phase === 'confirmed'
					? labels.succeeded
					: (idleLabel ?? labels.idle)
	);

	function reset(): void {
		clearTimeout(timer);
		timer = undefined;
		phase = 'idle';
		showPending = false;
	}

	async function run(): Promise<void> {
		if (disabled) {
			reset();

			return;
		}

		phase = 'pending';
		showPending = !pendingDelayMs;
		let pendingSince = showPending ? Date.now() : undefined;
		if (pendingDelayMs) {
			timer = setTimeout(() => {
				pendingSince = Date.now();
				showPending = true;
			}, pendingDelayMs);
		}
		let succeeded = false;

		try {
			succeeded = await onaction();
		} catch {
			// The caller owns failure messaging.
		}

		if (disposed) return;
		clearTimeout(timer);

		const remaining = pendingSince === undefined ? 0 : minPendingMs - (Date.now() - pendingSince);
		if (remaining > 0) timer = setTimeout(() => settle(succeeded), remaining);
		else settle(succeeded);
	}

	function settle(succeeded: boolean): void {
		if (!succeeded || !labels.succeeded) {
			reset();
			if (succeeded) onsettled?.();

			return;
		}

		phase = 'confirmed';
		timer = setTimeout(() => {
			reset();
			onsettled?.();
		}, confirmationDuration);
	}

	export function activate(): void {
		if (disposed || disabled || busy) return;

		if (phase === 'armed') {
			reset();
		} else if (confirmation) {
			phase = 'armed';
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

<span class="relative inline-grid w-max shrink-0 motion-reduce:[&_*]:transition-none">
	<Button
		disabled={disabled && !busy}
		{form}
		{variant}
		type={form ? 'submit' : 'button'}
		aria-label={label}
		aria-disabled={busy || undefined}
		aria-busy={phase === 'pending' || undefined}
		aria-describedby={phase === 'armed' ? warningId : undefined}
		class="relative inline-grid overflow-hidden aria-disabled:cursor-default {phase === 'pending' ||
		pendingDelayMs !== undefined
			? '[&>span]:transition-none'
			: ''} {compactOnMobile && icon ? 'max-lg:min-w-0 max-lg:px-2.5' : ''} {className ?? ''}"
		onclick={(event) => {
			if (busy) event.preventDefault();
			else if (!form) activate();
		}}
		onkeydown={(event) => {
			if (event.key !== 'Escape' || phase !== 'armed') return;

			event.preventDefault();
			event.stopPropagation();
			reset();
		}}
	>
		<ButtonFace shown={phase === 'idle' || (phase === 'pending' && !showPending)} {icon}>
			<span class={labelClass}>{labels.idle}</span>
		</ButtonFace>
		{#if confirmation}
			<ButtonFace shown={phase === 'armed'}>
				{#snippet icon()}
					<X size={16} aria-hidden="true" />
				{/snippet}
				<span class={labelClass}>{confirmation.cancelLabel}</span>
			</ButtonFace>
		{/if}
		{#if pendingDelayMs === undefined}
			<ButtonFace shown={phase === 'pending' && showPending}>
				{#snippet icon()}
					<LoaderCircle
						size={16}
						class="animate-spin motion-reduce:animate-none"
						aria-hidden="true"
					/>
				{/snippet}
				<span class={labelClass}>{labels.pending}</span>
			</ButtonFace>
		{:else if phase === 'pending' && showPending}
			<LoaderCircle
				size={16}
				aria-hidden="true"
				class="col-start-1 row-start-1 animate-spin place-self-center motion-reduce:animate-none"
			/>
		{/if}
		{#if labels.succeeded}
			<ButtonFace shown={phase === 'confirmed'}>
				{#snippet icon()}
					<Check size={16} aria-hidden="true" />
				{/snippet}
				<span class={labelClass}>{labels.succeeded}</span>
			</ButtonFace>
		{/if}
		{#if phase === 'armed' && confirmation}
			<span
				aria-hidden="true"
				class="countdown absolute inset-x-0 bottom-0 h-1 origin-left bg-danger motion-reduce:animate-none"
				style:--duration={`${confirmation.duration}ms`}
			></span>
		{/if}
	</Button>
	{#if phase === 'armed' && confirmation}
		<span
			id={warningId}
			class="absolute top-full right-0 z-10 mt-1 w-48 rounded-control border border-border bg-panel p-2 text-sm text-foreground shadow-sm transition-opacity duration-150 starting:opacity-0"
		>
			{confirmation.description}
		</span>
	{/if}
	<span role="status" class="sr-only">
		{phase === 'armed'
			? confirmation?.description
			: phase === 'pending'
				? labels.pending
				: phase === 'idle'
					? ''
					: label}
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
