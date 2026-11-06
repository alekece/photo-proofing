<script lang="ts">
	import { CaseSensitive, Grid2x2, Image, Square, Type } from '@lucide/svelte';
	import IconButtonGroup from '#lib/components/ui/icon-button-group/icon-button-group.svelte';
	import {
		SIZE_STEPS,
		type Mark,
		type Pattern,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';

	let { settings = $bindable() }: { settings: WorkspaceSettings } = $props();

	const SAMPLES = 6;
	const noLogo = $derived(settings.logo === null);
	const marks = $derived([
		{ id: 'name', label: 'Name', icon: Type },
		{ id: 'logo', label: 'Logo', icon: Image, disabled: noLogo },
		{ id: 'both', label: 'Name and logo', icon: CaseSensitive, disabled: noLogo }
	]);
	const patterns = [
		{ id: 'repeat', label: 'Repeat', icon: Grid2x2 },
		{ id: 'once', label: 'Once', icon: Square }
	];
	const showName = $derived(settings.mark !== 'logo' || noLogo);
	const showLogo = $derived(settings.mark !== 'name' && !noLogo);
	const name = $derived(settings.studioName.trim() || 'Your studio');
	// Rendered text grows with the size step; the logo scales with it.
	const scale = $derived(0.6 + settings.size * 0.3);
	const repeat = $derived(settings.pattern === 'repeat');

	function placement(cell: number) {
		const [row, column] = [Math.floor(cell / 3), cell % 3];

		return `top:${[8, 50, 92][row]}%;left:${[12, 50, 88][column]}%;transform:translate(-${[12, 50, 88][column]}%,-${[8, 50, 92][row]}%) scale(${scale})`;
	}
</script>

{#snippet stamp()}
	<span class="inline-flex items-center gap-1 text-inverse/80">
		{#if showLogo}<img src={settings.logo} alt="" class="h-4 w-auto" />{/if}
		{#if showName}<span class="font-mono text-[10px] tracking-widest uppercase">{name}</span>{/if}
	</span>
{/snippet}

<div class="grid gap-3">
	<div
		class="flex flex-wrap items-center gap-x-6 gap-y-3 rounded-card border border-border bg-panel p-3"
		role="toolbar"
		aria-label="Watermark"
	>
		<div class="grid gap-1">
			<span class="text-xs text-muted-foreground">Mark</span>
			<IconButtonGroup
				label="Mark"
				items={marks}
				value={settings.mark}
				onchange={(value) => (settings.mark = value as Mark)}
			/>
			{#if noLogo}<span class="text-xs text-muted-foreground">Upload a logo for more marks.</span
				>{/if}
		</div>
		<div class="grid gap-1">
			<span class="text-xs text-muted-foreground">Pattern</span>
			<IconButtonGroup
				label="Pattern"
				items={patterns}
				value={settings.pattern}
				onchange={(value) => (settings.pattern = value as Pattern)}
			/>
		</div>
		<fieldset class="grid gap-1" disabled={repeat}>
			<legend class="text-xs text-muted-foreground">Position</legend>
			<div class="grid grid-cols-3 gap-1" class:opacity-50={repeat}>
				{#each Array.from({ length: 9 }, (_, cell) => cell) as cell (cell)}
					<button
						type="button"
						aria-label="Position {cell + 1}"
						aria-pressed={settings.position === cell}
						class="grid size-8 place-items-center rounded-control disabled:cursor-not-allowed"
						onclick={() => (settings.position = cell)}
					>
						<span
							class="size-2.5 rounded-full {settings.position === cell
								? 'bg-foreground'
								: 'bg-border-strong'}"
						></span>
					</button>
				{/each}
			</div>
		</fieldset>
		<div class="grid gap-1">
			<label for="watermark-size" class="text-xs text-muted-foreground">Size</label>
			<div class="flex items-center gap-2">
				<span aria-hidden="true" class="text-xs">A</span>
				<input
					id="watermark-size"
					type="range"
					min="0"
					max={SIZE_STEPS - 1}
					step="1"
					bind:value={settings.size}
					class="w-32 accent-foreground"
				/>
				<span aria-hidden="true" class="text-xl">A</span>
			</div>
		</div>
	</div>
	<div
		class="grid grid-cols-2 gap-2 sm:grid-cols-3"
		role="img"
		aria-label="Preview of six proofs with the watermark"
	>
		{#each Array.from({ length: SAMPLES }, (_, i) => i) as sample (sample)}
			<div
				class="relative aspect-[3/2] overflow-hidden rounded-control bg-placeholder-tile {sample %
					2 ===
				0
					? 'bg-placeholder-tile-strong'
					: ''}"
			>
				{#if repeat}
					<div
						class="absolute -inset-1/2 flex -rotate-[20deg] flex-wrap content-around items-center justify-around gap-y-4"
						style="font-size:{scale}em"
					>
						{#each Array.from({ length: 12 }, (_, i) => i) as tile (tile)}
							<div class="bg-foreground/40 px-1 py-0.5">{@render stamp()}</div>
						{/each}
					</div>
				{:else}
					<div class="absolute bg-foreground/40 px-1 py-0.5" style={placement(settings.position)}>
						{@render stamp()}
					</div>
				{/if}
			</div>
		{/each}
	</div>
</div>
