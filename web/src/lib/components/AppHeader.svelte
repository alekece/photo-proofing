<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';

	let {
		back,
		onsignout
	}: {
		back?: { href: string; label: string };
		onsignout?: () => void | Promise<void>;
	} = $props();
</script>

<header
	class="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-panel px-6 py-3"
>
	<div class="flex flex-wrap items-center gap-4">
		<span class="brand text-xl text-foreground">PhotoProofing</span>
		{#if back}
			<a href={back.href} class="text-sm text-muted-foreground underline">
				<span aria-hidden="true">←</span>
				{back.label}
			</a>
		{/if}
	</div>
	<Button variant="outline" size="sm" onclick={() => onsignout?.()}>Sign out</Button>
</header>
