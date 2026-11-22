<script module lang="ts">
	export type Step = {
		id: 'upload' | 'share' | 'collect' | 'publish';
		label: string;
		done: boolean;
		fact?: string;
		accent?: boolean;
		onactivate?: () => void;
	};
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check } from '@lucide/svelte';

	let { steps }: { steps: readonly Step[] } = $props();

	const current = $derived(steps.findIndex(({ done }) => !done));
	const stepStyle =
		'flex flex-col items-start gap-1 rounded-control p-1 @min-[41rem]:flex-row @min-[41rem]:items-center @min-[41rem]:gap-1.5';
</script>

{#snippet marker(step: Step, index: number)}
	<span
		aria-hidden="true"
		class="h-1 w-[1.125rem] shrink-0 rounded-xs @min-[41rem]:grid @min-[41rem]:size-6 @min-[41rem]:place-items-center @min-[41rem]:rounded-full @min-[41rem]:border-[1.5px] @min-[41rem]:text-[0.6875rem] @min-[41rem]:font-bold {step.done
			? 'border-ok bg-ok text-inverse'
			: index === current
				? 'border-danger bg-danger @min-[41rem]:bg-golden-hour @min-[41rem]:text-danger'
				: 'border-border-strong bg-border-strong @min-[41rem]:bg-background @min-[41rem]:text-muted-foreground'}"
	>
		<span class="hidden @min-[41rem]:inline">
			{#if step.done}<Check size={12} strokeWidth={3} />{:else}{index + 1}{/if}
		</span>
	</span>
	<span class="flex flex-col">
		<strong
			class="text-[0.8125rem] leading-[1.15] {step.done || index === current
				? 'font-semibold text-foreground'
				: 'font-medium text-muted-foreground'}">{step.label}</strong
		>
		{#if step.done}<span class="sr-only">{m.step_done()}</span>{/if}
		{#if step.fact}
			<span
				class="hidden text-[0.6875rem] leading-[1.2] @min-[41rem]:block {step.accent
					? 'font-extrabold text-danger'
					: 'text-muted-foreground'}"
				data-slot="step-fact">{step.fact}</span
			>
		{/if}
	</span>
{/snippet}

<nav aria-label={m.step_progress()} class="flex min-w-0 items-center gap-2">
	<ol class="flex flex-wrap items-start gap-1 @min-[41rem]:items-center @min-[41rem]:gap-2">
		{#each steps as step, index (step.id)}
			{#if index > 0}
				<li
					aria-hidden="true"
					class="hidden h-[1.5px] w-5 shrink-0 @min-[41rem]:block {steps[index - 1].done
						? 'bg-ok/55'
						: 'bg-border'}"
				></li>
			{/if}
			<li aria-current={index === current ? 'step' : undefined}>
				{#if step.onactivate}
					<button
						type="button"
						data-share-trigger={step.id === 'share' ? true : undefined}
						aria-label={m.step_shortcut({ step: step.label })}
						onclick={step.onactivate}
						class="{stepStyle} min-h-7 min-w-7 cursor-pointer text-left hover:bg-background focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
					>
						{@render marker(step, index)}
					</button>
				{:else}
					<span class={stepStyle}>{@render marker(step, index)}</span>
				{/if}
			</li>
		{/each}
	</ol>
</nav>
