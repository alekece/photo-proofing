<script lang="ts">
	import { ArrowLeft, LogOut, User } from '@lucide/svelte';
	import Popover from '#lib/components/ui/popover/popover.svelte';
	import type { Snippet } from 'svelte';

	let {
		back,
		actions,
		onsignout
	}: {
		back?: { href: string; label: string };
		/** Room beside the account menu, for the notification bell once #72 provides its data. */
		actions?: Snippet;
		onsignout?: () => void | Promise<void>;
	} = $props();
</script>

<header
	class="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-panel px-6 py-3"
>
	<div class="flex flex-wrap items-center gap-4">
		<span class="brand text-xl text-foreground">PhotoProofing</span>
		{#if back}
			<a
				href={back.href}
				class="inline-flex items-center gap-1 text-sm text-muted-foreground underline"
			>
				<ArrowLeft size={14} aria-hidden="true" />
				{back.label}
			</a>
		{/if}
	</div>
	<div class="flex items-center gap-2">
		{@render actions?.()}
		<Popover
			label="Account menu"
			align="end"
			class="grid size-10 place-items-center rounded-full border border-border-strong bg-background text-foreground"
			panelClass="w-48"
		>
			{#snippet trigger()}<User size={18} aria-hidden="true" />{/snippet}
			<button
				type="button"
				class="flex min-h-10 w-full cursor-pointer items-center gap-2 rounded-control px-3 text-left text-sm text-foreground hover:bg-border/60"
				onclick={() => onsignout?.()}
			>
				<LogOut size={16} aria-hidden="true" />
				Sign out
			</button>
		</Popover>
	</div>
</header>
