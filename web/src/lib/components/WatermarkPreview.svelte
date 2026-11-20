<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { WorkspaceSettings } from '#lib/settings/settings.js';

	let { settings }: { settings: WorkspaceSettings } = $props();

	const SCALES = [1.4, 1.6, 1.9, 2.2, 2.5, 2.85, 3.2, 3.6, 4, 4.8];
	const ALIGN = ['start', 'center', 'end'];
	const showLogo = $derived(settings.mark !== 'name' && settings.logo !== null);
	const showName = $derived(settings.mark !== 'logo' || settings.logo === null);
	const name = $derived(settings.studioName.trim() || m.watermark_your_studio());
	const scale = $derived(SCALES[Math.min(settings.size, SCALES.length - 1)]);
	const opacity = $derived(settings.watermarkOpacity / 100);
</script>

{#snippet stamp()}
	<span class="inline-flex items-center gap-[0.45em] font-serif whitespace-nowrap italic">
		{#if showLogo}<img src={settings.logo} alt="" class="size-[1.6em] object-contain" />{/if}
		{#if showName}
			<span
				class="[text-shadow:0_1px_4px_rgb(0_0_0/0.35)]"
				style:color={settings.watermarkTextColor}>{name}</span
			>
		{/if}
	</span>
{/snippet}

{#if settings.pattern === 'repeat'}
	<div
		class="absolute -inset-[40%] grid -rotate-[24deg] grid-cols-[repeat(4,max-content)] content-center justify-center gap-x-[4em] gap-y-[3.2em]"
		style:font-size="{scale}cqw"
		style:opacity
	>
		{#each Array.from({ length: 40 }, (_, i) => i) as tile (tile)}
			<span class={tile % 8 >= 4 ? '-translate-x-1/2' : ''}>{@render stamp()}</span>
		{/each}
	</div>
{:else}
	<div class="absolute inset-0 grid p-[5cqw]" style:font-size="{scale}cqw" style:opacity>
		<div
			style:align-self={ALIGN[Math.floor(settings.position / 3)]}
			style:justify-self={ALIGN[settings.position % 3]}
		>
			{@render stamp()}
		</div>
	</div>
{/if}
