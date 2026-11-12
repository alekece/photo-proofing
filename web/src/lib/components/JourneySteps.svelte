<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Check } from '@lucide/svelte';

	type Step = {
		id: string;
		label: string;
		done: boolean;
		/** What the step stands at: a count, a date, or what to do next. Omitted once it only repeats the status. */
		fact?: string;
		/** Makes the fact bold and warm, e.g. a selection over the allowance. */
		accent?: boolean;
	};

	/** The first step that isn't done is the current one; later steps are dim. */
	let { steps }: { steps: readonly Step[] } = $props();

	const current = $derived(steps.findIndex(({ done }) => !done));
	const focus = $derived(steps[current] ?? steps.at(-1));
</script>

<nav aria-label={m.step_progress()} class="min-w-0 flex-1">
	<ol class="hidden max-w-2xl items-center gap-2.5 @min-[58rem]:flex @min-[66rem]:gap-3.5">
		{#each steps as { id, label, done, fact, accent }, index (id)}
			{#if index > 0}
				<li
					aria-hidden="true"
					class="h-[1.5px] max-w-10 min-w-2 flex-1 {steps[index - 1].done
						? 'bg-ok/55'
						: 'bg-border'}"
				></li>
			{/if}
			<li
				class="relative flex min-w-0 items-center gap-2 py-0.5 text-[0.78125rem] leading-[1.25] whitespace-nowrap"
				aria-current={index === current ? 'step' : undefined}
			>
				<span
					class="grid size-6 shrink-0 place-items-center rounded-full border-[1.5px] text-[0.6875rem] font-bold {done
						? 'border-ok bg-ok text-inverse'
						: index === current
							? 'border-danger bg-golden-hour text-danger'
							: 'border-border-strong bg-background text-muted-foreground'}"
				>
					{#if done}<Check size={12} strokeWidth={3} aria-hidden="true" />{:else}{index + 1}{/if}
				</span>
				<span class="flex min-w-0 flex-col">
					<strong
						class="truncate text-[0.8125rem] leading-[1.15] {done || index === current
							? 'font-semibold text-foreground'
							: 'font-medium text-muted-foreground'}">{label}</strong
					>
					{#if done}<span class="sr-only">{m.step_done()}</span>{/if}
					{#if fact}
						<span
							class="max-w-44 truncate text-[0.6875rem] @min-[68rem]:max-w-none {accent
								? 'font-extrabold text-danger'
								: 'text-muted-foreground'}"
							data-slot="step-fact">{fact}</span
						>
					{/if}
				</span>
			</li>
		{/each}
	</ol>
	<div class="flex min-w-0 items-center gap-2.5 text-[0.78rem] @min-[58rem]:hidden">
		<ol class="flex shrink-0 gap-1">
			{#each steps as { id, label, done }, index (id)}
				<li
					class="h-1 w-[1.125rem] rounded-xs {done
						? 'bg-ok'
						: index === current
							? 'bg-danger'
							: 'bg-border-strong'}"
					aria-current={index === current ? 'step' : undefined}
				>
					<span class="sr-only">{label}{done ? ` (${m.step_done()})` : ''}</span>
				</li>
			{/each}
		</ol>
		<span aria-hidden="true" class="min-w-0 truncate text-muted-foreground"
			><b class="font-semibold text-foreground">{focus?.label}</b>
			{#if focus?.fact}<span>· </span><span class={focus.accent ? 'font-extrabold text-danger' : ''}
					>{focus.fact}</span
				>{/if}</span
		>
	</div>
</nav>
