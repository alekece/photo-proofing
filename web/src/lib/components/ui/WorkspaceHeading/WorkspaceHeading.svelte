<script lang="ts">
	import { Plus } from '@lucide/svelte';
	import { cn } from '#lib/utils.js';
	import Button from '#lib/components/ui/Button/Button.svelte';

	let {
		title,
		count,
		subtitle,
		create,
		createRef = $bindable(null),
		class: className
	}: {
		title: string;
		count?: string;
		subtitle?: string;
		create?: { label: string; onclick: () => void; disabled?: boolean };
		createRef?: HTMLButtonElement | null;
		class?: string;
	} = $props();
</script>

<div
	class={cn(
		create && 'grid items-end gap-3 py-4 sm:grid-cols-[1fr_auto] sm:gap-6 sm:py-6',
		className
	)}
>
	<div>
		<div class="flex flex-wrap items-center gap-2.5">
			<h1 class="text-xl leading-tight sm:sr-only">{title}</h1>
			{#if count !== undefined}
				<span class="pill bg-border/60 text-xs font-medium text-foreground tabular-nums">
					{count}
				</span>
			{/if}
		</div>
		{#if subtitle}
			<p class="mt-1 text-sm text-muted-foreground">{subtitle}</p>
		{/if}
	</div>
	{#if create}
		<Button
			bind:ref={createRef}
			class="justify-self-start sm:justify-self-auto"
			onclick={create.onclick}
			disabled={create.disabled}
		>
			<Plus size={16} aria-hidden="true" />
			{create.label}
		</Button>
	{/if}
</div>
