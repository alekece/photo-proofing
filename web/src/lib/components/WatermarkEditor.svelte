<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Mark, Pattern, WorkspaceSettings } from '#lib/settings/settings.js';
	import MarkLogo from './MarkLogo.svelte';
	import SamplePhoto from './SamplePhoto.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import { TOOLBAR, TOOLBAR_GLYPH, TOOLBAR_ITEM } from './settings-toolbar.js';

	let { settings = $bindable() }: { settings: WorkspaceSettings } = $props();

	// Watermark text size, as a share of the preview width, per size step; the even steps are the
	// original five.
	const SCALES = [1.4, 1.6, 1.9, 2.2, 2.5, 2.85, 3.2, 3.6, 4, 4.8];
	const ALIGN = ['start', 'center', 'end'];
	const noLogo = $derived(settings.logo === null);
	const marks: { id: Mark; label: string }[] = [
		{ id: 'name', label: m.common_name() },
		{ id: 'logo', label: m.common_logo() },
		{ id: 'both', label: m.watermark_both() }
	];
	const patterns: { id: Pattern; label: string }[] = [
		{ id: 'repeat', label: m.watermark_repeat() },
		{ id: 'once', label: m.watermark_once() }
	];
	// A logo-only mark falls back to the name until a logo exists.
	const showLogo = $derived(settings.mark !== 'name' && !noLogo);
	const showName = $derived(settings.mark !== 'logo' || noLogo);
	const name = $derived(settings.studioName.trim() || m.watermark_your_studio());
	const repeat = $derived(settings.pattern === 'repeat');
	const scale = $derived(SCALES[Math.min(settings.size, SCALES.length - 1)]);
</script>

{#snippet stamp()}
	<span
		class="inline-flex items-center gap-[0.45em] font-serif whitespace-nowrap text-white italic [text-shadow:0_1px_4px_rgb(0_0_0/0.35)]"
	>
		{#if showLogo}<img src={settings.logo} alt="" class="size-[1.6em] object-contain" />{/if}
		{#if showName}{name}{/if}
	</span>
{/snippet}

<div class={TOOLBAR} role="toolbar" aria-label={m.watermark_label()}>
	<div
		role="group"
		aria-label={m.watermark_mark()}
		class="flex items-center gap-0.5 px-1 sm:gap-1 sm:px-2"
	>
		{#each marks as { id, label } (id)}
			<button
				type="button"
				title={label}
				aria-label={label}
				aria-pressed={settings.mark === id}
				disabled={noLogo && id !== 'name'}
				class={TOOLBAR_ITEM}
				onclick={() => (settings.mark = id)}
			>
				<span class={TOOLBAR_GLYPH}>
					{#if id !== 'name'}<MarkLogo logo={settings.logo} class="size-6" />{/if}
					{#if id !== 'logo'}Aa{/if}
				</span>
			</button>
		{/each}
	</div>
	<div
		role="group"
		aria-label={m.watermark_pattern()}
		class="flex items-center gap-0.5 border-l border-border px-1 sm:gap-1 sm:px-2"
	>
		{#each patterns as { id, label } (id)}
			<button
				type="button"
				title={label}
				aria-label={label}
				aria-pressed={settings.pattern === id}
				class={TOOLBAR_ITEM}
				onclick={() => (settings.pattern = id)}
			>
				<svg viewBox="0 0 30 22" aria-hidden="true" class="h-[1.375rem] w-[1.875rem]">
					<rect
						x="1"
						y="1"
						width="28"
						height="20"
						rx="3"
						fill="none"
						stroke="currentColor"
						stroke-width="1.6"
					/>
					{#if id === 'repeat'}
						{#each [[5, 5], [13, 5], [21, 5], [9, 10], [17, 10], [5, 15], [13, 15], [21, 15]] as [x, y] (`${x}-${y}`)}
							<rect {x} {y} width="4" height="2.4" rx="1" fill="currentColor" />
						{/each}
					{:else}
						<rect x="11" y="8" width="8" height="6" rx="1.5" fill="currentColor" />
					{/if}
				</svg>
			</button>
		{/each}
	</div>
	<div class="flex items-center border-l border-border px-1 sm:px-2" title={m.watermark_position()}>
		<fieldset
			disabled={repeat}
			aria-label={m.watermark_position()}
			class="m-0 grid min-w-0 grid-cols-[repeat(3,1rem)] gap-[3px] border-0 p-1 transition-opacity {repeat
				? 'opacity-40'
				: ''}"
		>
			{#each Array.from({ length: 9 }, (_, cell) => cell) as cell (cell)}
				<button
					type="button"
					aria-label={m.watermark_position_cell({ cell: cell + 1 })}
					aria-pressed={!repeat && settings.position === cell}
					class="grid h-3 w-4 cursor-pointer place-items-center rounded-[3px] bg-[#efe7dc] hover:bg-[#e2d6c6] disabled:cursor-default aria-pressed:bg-foreground aria-pressed:after:size-[5px] aria-pressed:after:rounded-[1px] aria-pressed:after:bg-inverse aria-pressed:after:content-['']"
					onclick={() => (settings.position = cell)}
				></button>
			{/each}
		</fieldset>
	</div>
	<SizeSlider bind:value={settings.size} label={m.watermark_size()} />
</div>
<SamplePhoto label={m.watermark_preview()}>
	{#if repeat}
		<div
			class="absolute -inset-[40%] grid -rotate-[24deg] grid-cols-[repeat(4,max-content)] content-center justify-center gap-x-[4em] gap-y-[3.2em] opacity-[0.42]"
			style="font-size:{scale}cqw"
		>
			{#each Array.from({ length: 40 }, (_, i) => i) as tile (tile)}
				<span class={tile % 8 >= 4 ? '-translate-x-1/2' : ''}>{@render stamp()}</span>
			{/each}
		</div>
	{:else}
		<div class="absolute inset-0 grid p-[5cqw]" style="font-size:{scale}cqw">
			<div
				class="opacity-80"
				style="align-self:{ALIGN[Math.floor(settings.position / 3)]};justify-self:{ALIGN[
					settings.position % 3
				]}"
			>
				{@render stamp()}
			</div>
		</div>
	{/if}
</SamplePhoto>
