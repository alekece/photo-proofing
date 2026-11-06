<script lang="ts">
	import { Check } from '@lucide/svelte';

	type Step = { id: string; label: string; done: boolean; fact?: string };

	/** The first step that isn't done is the current one; later steps are dim. */
	let { steps }: { steps: readonly Step[] } = $props();

	const current = $derived(steps.findIndex(({ done }) => !done));
	const currentLabel = $derived(steps[current]?.label ?? steps.at(-1)?.label ?? '');
</script>

<nav aria-label="Gallery progress" class="text-inverse">
	<ol class="hidden gap-4 sm:flex">
		{#each steps as { id, label, done, fact }, index (id)}
			<li
				class="flex min-w-0 flex-1 items-center gap-2 border-t-2 pt-2 text-sm {done
					? 'border-ok'
					: index === current
						? 'border-inverse'
						: 'border-white/30 opacity-60'}"
				aria-current={index === current ? 'step' : undefined}
			>
				{#if done}
					<span class="grid size-5 shrink-0 place-items-center rounded-full bg-ok">
						<Check size={12} aria-hidden="true" />
					</span>
				{/if}
				<span class="min-w-0">
					<span class="block truncate {index === current ? 'font-semibold' : ''}">{label}</span>
					{#if done && fact}<span class="block truncate text-xs opacity-80">{fact}</span>{/if}
				</span>
			</li>
		{/each}
	</ol>
	<div class="flex items-center gap-3 text-sm sm:hidden">
		<ol class="flex gap-1.5">
			{#each steps as { id, label, done }, index (id)}
				<li
					class="size-2.5 rounded-full {done
						? 'bg-ok'
						: index === current
							? 'bg-inverse'
							: 'bg-white/30'}"
					aria-current={index === current ? 'step' : undefined}
				>
					<span class="sr-only">{label}{done ? ' (done)' : ''}</span>
				</li>
			{/each}
		</ol>
		<span aria-hidden="true">{currentLabel}</span>
	</div>
</nav>
