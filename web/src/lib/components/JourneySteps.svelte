<script lang="ts">
	import { Check } from '@lucide/svelte';

	type Step = {
		id: string;
		label: string;
		done: boolean;
		/** What the step stands at: a count, a date, or what to do next. */
		fact: string;
		/** Makes the fact bold and warm, e.g. a selection over the allowance. */
		accent?: boolean;
	};

	/** The first step that isn't done is the current one; later steps are dim. */
	let { steps }: { steps: readonly Step[] } = $props();

	const current = $derived(steps.findIndex(({ done }) => !done));
	const focus = $derived(steps[current] ?? steps.at(-1));
</script>

<nav
	aria-label="Gallery progress"
	class="absolute inset-x-0 bottom-0 border-t border-panel/20 bg-scrim text-panel/85 backdrop-blur-[14px] backdrop-saturate-120"
>
	<ol class="hidden items-center gap-3.5 px-8 sm:flex">
		{#each steps as { id, label, done, fact, accent }, index (id)}
			{#if index > 0}
				<li
					aria-hidden="true"
					class="h-[1.5px] max-w-30 flex-1 {steps[index - 1].done
						? 'bg-ok-on-dark/80'
						: 'bg-panel/25'}"
				></li>
			{/if}
			<li
				class="relative flex items-center gap-2 py-3 text-[0.78rem] leading-tight whitespace-nowrap {index ===
				current
					? 'shadow-[inset_0_-2px_0_var(--color-warning-on-dark)]'
					: done
						? ''
						: 'opacity-75'}"
				aria-current={index === current ? 'step' : undefined}
			>
				<span
					class="grid size-6 shrink-0 place-items-center rounded-full border-[1.5px] text-[0.625rem] font-bold {done
						? 'border-ok-on-dark bg-ok-on-dark text-panel'
						: index === current
							? 'border-warning-on-dark text-warning-on-dark'
							: 'border-panel/50 text-panel'}"
				>
					{#if done}<Check size={12} strokeWidth={3} aria-hidden="true" />{:else}{index + 1}{/if}
				</span>
				<span class="flex flex-col">
					<strong class="text-[0.84rem] font-semibold text-panel">{label}</strong>
					<span class={accent ? 'font-extrabold text-warning-on-dark' : ''} data-slot="step-fact"
						>{fact}</span
					>
				</span>
			</li>
		{/each}
	</ol>
	<div class="flex items-center gap-3 px-[1.125rem] py-[0.6875rem] text-[0.78rem] sm:hidden">
		<ol class="flex gap-[0.3125rem]">
			{#each steps as { id, label, done }, index (id)}
				<li
					class="h-1 w-[1.375rem] rounded-xs {done
						? 'bg-ok-on-dark'
						: index === current
							? 'bg-warning-on-dark'
							: 'bg-panel/30'}"
					aria-current={index === current ? 'step' : undefined}
				>
					<span class="sr-only">{label}{done ? ' (done)' : ''}</span>
				</li>
			{/each}
		</ol>
		<span aria-hidden="true" class="min-w-0 truncate"
			><b class="font-semibold text-panel">{focus?.label}</b>
			<span class="opacity-80">· {focus?.fact}</span></span
		>
	</div>
</nav>
