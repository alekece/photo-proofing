<script module lang="ts">
	export type Step = {
		id: 'upload' | 'share' | 'collect' | 'retouch' | 'deliver';
		label: string;
		done: boolean;
		fact?: string;
		accent?: boolean;
		onactivate?: () => void;
	};
</script>

<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ChevronRight } from '@lucide/svelte';

	let { steps }: { steps: readonly Step[] } = $props();

	const current = $derived(steps.findIndex(({ done }) => !done));
	const stepStyle = 'flex w-full min-w-0 items-center gap-1.5 rounded-control p-1';
</script>

{#snippet marker(step: Step, index: number)}
	<span
		aria-hidden="true"
		class="grid size-6 shrink-0 place-items-center rounded-full border-[1.5px] text-[0.6875rem] font-bold {step.done
			? 'border-ok bg-ok text-inverse'
			: index === current
				? 'border-terracotta-border bg-terracotta-soft text-terracotta'
				: 'border-border-strong bg-background text-muted-foreground'}"
	>
		{index + 1}
	</span>
	<span class="flex min-w-0 flex-col {index === current ? '' : 'sr-only @min-[56rem]:not-sr-only'}">
		<strong
			class="truncate text-[0.8125rem] leading-[1.15] {step.done || index === current
				? 'font-semibold text-foreground'
				: 'font-medium text-muted-foreground'}"
		>
			{step.label}
		</strong>
		{#if step.done}<span class="sr-only">{m.step_done()}</span>{/if}
		{#if step.fact}
			<span
				class="hidden text-[0.6875rem] leading-[1.2] @min-[72rem]:block {step.accent
					? 'font-extrabold text-danger'
					: 'text-muted-foreground'}"
				data-slot="step-fact"
			>
				{step.fact}
			</span>
		{/if}
	</span>
{/snippet}

<nav aria-label={m.step_progress()} class="flex min-w-0 flex-1 items-center justify-center gap-2">
	<ol class="flex min-w-0 items-center justify-center gap-1">
		{#each steps as step, index (step.id)}
			{#if index > 0}
				<li
					aria-hidden="true"
					class="shrink-0 {steps[index - 1].done
						? index === current
							? 'text-foreground'
							: 'text-ok'
						: 'text-border-strong'}"
				>
					<ChevronRight size={16} strokeWidth={2.5} />
				</li>
			{/if}
			<li aria-current={index === current ? 'step' : undefined} class="min-w-0">
				{#if step.onactivate}
					<button
						type="button"
						data-share-trigger={step.id === 'share' ? true : undefined}
						aria-label={m.step_shortcut({ step: step.label })}
						title={step.label}
						onclick={step.onactivate}
						class="{stepStyle} min-h-7 min-w-7 cursor-pointer text-left hover:bg-background focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
					>
						{@render marker(step, index)}
					</button>
				{:else}
					<span class={stepStyle} title={step.label}>{@render marker(step, index)}</span>
				{/if}
			</li>
		{/each}
	</ol>
</nav>
