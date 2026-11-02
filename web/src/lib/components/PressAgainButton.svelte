<script lang="ts" module>
	/** How long the second press stays valid after arming. */
	export const CONFIRM_WINDOW = 5000;
</script>

<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';
	import type { ButtonVariant } from '#lib/components/ui/button/button.svelte';

	let {
		label,
		confirmLabel,
		onconfirm,
		disabled = false,
		variant = 'default'
	}: {
		label: string;
		confirmLabel: string;
		onconfirm: () => void | Promise<void>;
		disabled?: boolean;
		variant?: ButtonVariant;
	} = $props();

	let armed = $state(false);
	let button = $state<HTMLButtonElement | null>(null);

	function disarm() {
		armed = false;
	}

	function press() {
		if (disabled) return;

		if (!armed) {
			armed = true;

			return;
		}

		armed = false;
		void onconfirm();
	}

	$effect(() => {
		if (!armed) return;

		const timer = setTimeout(disarm, CONFIRM_WINDOW);
		const onKeydown = (event: KeyboardEvent) => {
			if (event.key === 'Escape') disarm();
		};
		const onPointerdown = (event: PointerEvent) => {
			if (!button?.contains(event.target as Node)) disarm();
		};

		document.addEventListener('keydown', onKeydown);
		document.addEventListener('pointerdown', onPointerdown);

		return () => {
			clearTimeout(timer);
			document.removeEventListener('keydown', onKeydown);
			document.removeEventListener('pointerdown', onPointerdown);
		};
	});
</script>

<Button bind:ref={button} {variant} {disabled} onclick={press} class="relative overflow-hidden">
	{#if armed}
		<span
			class="fill absolute inset-0 origin-left bg-[color-mix(in_srgb,var(--color-inverse)_28%,transparent)]"
			style:animation-duration="{CONFIRM_WINDOW}ms"
			aria-hidden="true"
		></span>
	{/if}
	<span class="relative">{armed ? confirmLabel : label}</span>
</Button>
<span class="sr-only" aria-live="polite">{armed ? confirmLabel : ''}</span>

<style>
	/* Keyframes are scoped to this component, so the animation must be declared here. */
	.fill {
		animation: drain linear forwards;
	}
	@keyframes drain {
		from {
			transform: scaleX(1);
		}
		to {
			transform: scaleX(0);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.fill {
			animation: none;
		}
	}
</style>
