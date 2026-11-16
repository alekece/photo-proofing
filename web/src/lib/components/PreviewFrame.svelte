<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Monitor, Smartphone } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { PreviewViewport } from '#lib/gallery/preview.js';
	import { cn } from '#lib/utils.js';

	let {
		mode = $bindable<PreviewViewport>('desktop'),
		compactToggle = false,
		toggleClass,
		stageClass = 'min-h-0 flex-1',
		phoneClass = 'h-[min(680px,calc(100dvh-110px))]',
		header,
		children
	}: {
		mode?: PreviewViewport;
		compactToggle?: boolean;
		toggleClass?: string;
		stageClass?: string;
		phoneClass?: string;
		header: Snippet<[Snippet]>;
		children: Snippet;
	} = $props();

	const views = $derived([
		{ id: 'desktop' as const, label: m.preview_desktop(), icon: Monitor },
		{ id: 'phone' as const, label: m.preview_phone(), icon: Smartphone }
	]);
</script>

{#snippet toggle()}
	<div
		role="group"
		aria-label={m.preview_size()}
		class={cn('flex rounded-full bg-[#3a302a] p-[3px]', toggleClass)}
	>
		{#each views as { id, label, icon: Icon } (id)}
			<button
				type="button"
				aria-label={label}
				aria-pressed={mode === id}
				onclick={() => (mode = id)}
				class={cn(
					'flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[0.8125rem] font-semibold focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus',
					mode === id ? 'bg-panel text-foreground' : 'text-[#cdbfae]'
				)}
			>
				<Icon size={16} aria-hidden="true" />
				<span class={cn(compactToggle && 'max-[700px]:hidden')}>{label}</span>
			</button>
		{/each}
	</div>
{/snippet}

{@render header(toggle)}
<div
	class={cn(
		stageClass,
		mode === 'phone'
			? 'grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_50%_40%,#3a302a,#1d1815_70%)] p-4 sm:p-6'
			: 'overflow-auto bg-background'
	)}
>
	<div
		data-frame={mode}
		class={cn(
			mode === 'phone' &&
				'w-[340px] max-w-full rounded-[40px] bg-black p-2.5 shadow-[0_30px_80px_rgb(0_0_0/0.5)]',
			mode === 'phone' && phoneClass
		)}
	>
		<div
			class={cn(
				'relative bg-background text-foreground',
				mode === 'phone' &&
					'h-full translate-z-0 [scrollbar-width:none] overflow-auto rounded-[30px]'
			)}
		>
			{@render children()}
		</div>
	</div>
</div>
