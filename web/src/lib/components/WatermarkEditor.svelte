<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		SIZE_STEPS,
		type Mark,
		type Pattern,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';

	let {
		settings = $bindable(),
		samples = []
	}: {
		settings: WorkspaceSettings;
		/** Proof preview URLs shown under the toolbar; empty falls back to plain tiles. */
		samples?: string[];
	} = $props();

	// Relative width of each sample tile, two rows of three like a contact sheet.
	const SHEET = [
		[1.5, 0.67, 1.5],
		[0.67, 1.5, 0.75]
	];
	// Watermark text size, as a share of the tile width, per size step.
	const SCALES = [1.4, 1.9, 2.5, 3.2, 4];
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
	const scale = $derived(SCALES[Math.min(settings.size, SIZE_STEPS - 1)]);
	const item =
		'grid h-[2.625rem] min-w-[2.625rem] cursor-pointer place-items-center rounded-xl px-1.5 sm:h-12 sm:min-w-[3.25rem] sm:px-2.5 transition-colors hover:bg-[#f3ede5] aria-pressed:bg-foreground aria-pressed:text-inverse disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent';
	const glyph = 'inline-flex items-center gap-1 font-serif text-xl italic';
</script>

{#snippet logo(size: string)}
	{#if settings.logo}
		<img src={settings.logo} alt="" class="{size} object-contain" />
	{:else}
		<svg viewBox="0 0 48 48" aria-hidden="true" class={size}>
			<circle cx="24" cy="24" r="22" fill="none" stroke="currentColor" stroke-width="1.6" />
			<path
				d="M15 31c3-9 6-14 9-14s4 5 5 9 2 5 4 5"
				fill="none"
				stroke="currentColor"
				stroke-width="1.6"
				stroke-linecap="round"
			/>
			<circle cx="31" cy="15" r="1.8" fill="currentColor" />
		</svg>
	{/if}
{/snippet}

{#snippet stamp()}
	<span
		class="inline-flex items-center gap-[0.45em] font-serif whitespace-nowrap text-white italic [text-shadow:0_1px_4px_rgb(0_0_0/0.35)]"
	>
		{#if showLogo}<img src={settings.logo} alt="" class="size-[1.6em] object-contain" />{/if}
		{#if showName}{name}{/if}
	</span>
{/snippet}

<div
	class="mb-[1.125rem] flex flex-wrap items-center gap-x-1.5 gap-y-2 rounded-[1.25rem] border border-border bg-input p-2 shadow-[0_14px_30px_-18px_rgb(60_40_20/0.5)]"
	role="toolbar"
	aria-label={m.watermark_label()}
>
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
				class={item}
				onclick={() => (settings.mark = id)}
			>
				<span class={glyph}>
					{#if id !== 'name'}{@render logo('size-6')}{/if}
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
				class={item}
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
	<div
		class="flex min-w-[12.5rem] flex-1 items-center border-l border-border px-2 max-sm:basis-full max-sm:border-l-0"
	>
		<div class="flex w-full items-center gap-3 px-2">
			<span
				aria-hidden="true"
				class="font-serif text-[0.8125rem] leading-none text-muted-foreground">A</span
			>
			<input
				type="range"
				min="0"
				max={SIZE_STEPS - 1}
				step="1"
				aria-label={m.watermark_size()}
				bind:value={settings.size}
				class="h-7 flex-1 accent-foreground"
			/>
			<span aria-hidden="true" class="font-serif text-2xl leading-none text-muted-foreground"
				>A</span
			>
		</div>
	</div>
</div>
<div class="mb-14 flex flex-col gap-2" role="img" aria-label={m.watermark_preview()}>
	{#each SHEET as row, r (r)}
		<div class="flex h-[clamp(6.875rem,22vw,15.625rem)] gap-2">
			{#each row as ratio, c (c)}
				{@const sample = samples.length ? samples[(r * 3 + c) % samples.length] : null}
				<div
					class="[container-type:inline-size] relative h-full overflow-hidden rounded bg-[#222] {sample
						? ''
						: 'bg-placeholder-tile'}"
					style="flex:{ratio}"
				>
					{#if sample}<img src={sample} alt="" class="block size-full object-cover" />{/if}
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
				</div>
			{/each}
		</div>
	{/each}
</div>
