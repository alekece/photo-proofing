<script lang="ts">
	let { title, count, subtitle }: { title: string; count?: string; subtitle?: string } = $props();
</script>

<div>
	<div class="flex flex-wrap items-center gap-2.5">
		<h1 class="text-3xl leading-tight sm:text-[2.5rem]">{title}</h1>
		{#if count !== undefined}
			<span
				class="rounded-full bg-border/60 px-2.5 py-0.5 text-xs font-medium text-foreground tabular-nums"
			>
				{count}
			</span>
		{/if}
	</div>
	{#if subtitle}
		<p class="mt-1 text-sm text-muted-foreground">{subtitle}</p>
	{/if}
</div>
