<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import {
		normalizeWatermarkHex,
		type Mark,
		type Pattern,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import Button from './ui/Button/Button.svelte';
	import Input from './ui/Input/Input.svelte';
	import MarkLogo from './MarkLogo.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import { CHOICE, GLYPH } from './settings-toolbar.js';

	let {
		settings = $bindable(),
		hexEdit = $bindable()
	}: {
		settings: WorkspaceSettings;
		hexEdit: string | null;
	} = $props();

	const errorId = $props.id();
	const marks: { id: Mark; label: () => string }[] = [
		{ id: 'name', label: m.common_name },
		{ id: 'logo', label: m.common_logo },
		{ id: 'both', label: m.watermark_both }
	];
	const patterns: { id: Pattern; label: () => string }[] = [
		{ id: 'repeat', label: m.watermark_repeat },
		{ id: 'once', label: m.watermark_once }
	];
	const invalidHex = $derived(hexEdit !== null && normalizeWatermarkHex(hexEdit) === null);
	const opacity = $derived(
		new Intl.NumberFormat(getLocale(), { style: 'percent' }).format(settings.watermarkOpacity / 100)
	);

	function chooseMark(mark: Mark) {
		if (mark === 'logo' && invalidHex) hexEdit = null;
		settings.mark = mark;
	}

	function pickColor(value: string) {
		const color = normalizeWatermarkHex(value);
		if (color === null) return;
		hexEdit = null;
		settings.watermarkTextColor = color;
	}

	function typeHex(value: string) {
		hexEdit = value;
		const color = normalizeWatermarkHex(value);
		if (color !== null) settings.watermarkTextColor = color;
	}
</script>

<p class="mb-4 text-sm text-muted-foreground">{m.watermark_enabled_per_gallery()}</p>
<div class="grid min-w-0 items-start gap-3 sm:gap-4 xl:grid-cols-2">
	<fieldset class="min-w-0">
		<legend class="mb-2 text-sm font-semibold">{m.watermark_mark()}</legend>
		<div class="flex flex-wrap gap-1.5">
			{#each marks as { id, label } (id)}
				<Button
					variant="outline"
					title={label()}
					aria-label={label()}
					aria-pressed={settings.mark === id}
					disabled={settings.logo === null && id !== 'name'}
					class={CHOICE}
					onclick={() => chooseMark(id)}
				>
					<span class={GLYPH} aria-hidden="true">
						{#if id !== 'name'}<MarkLogo logo={settings.logo} class="size-5" />{/if}
						{#if id !== 'logo'}Aa{/if}
					</span>
					{#if settings.mark === id}<Check
							class="absolute top-0.5 right-0.5 size-3"
							aria-hidden="true"
						/>{/if}
				</Button>
			{/each}
		</div>
	</fieldset>
	<fieldset class="min-w-0">
		<legend class="mb-2 text-sm font-semibold">{m.watermark_pattern()}</legend>
		<div class="flex flex-wrap gap-1.5">
			{#each patterns as { id, label } (id)}
				<Button
					variant="outline"
					title={label()}
					aria-label={label()}
					aria-pressed={settings.pattern === id}
					class={CHOICE}
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
					{#if settings.pattern === id}<Check
							class="absolute top-0.5 right-0.5 size-3"
							aria-hidden="true"
						/>{/if}
				</Button>
			{/each}
		</div>
	</fieldset>
	<fieldset disabled={settings.pattern === 'repeat'} class="min-w-0 disabled:opacity-45">
		<legend class="mb-2 text-sm font-semibold">{m.watermark_position()}</legend>
		<div class="grid w-fit grid-cols-3 gap-1">
			{#each Array.from({ length: 9 }, (_, cell) => cell) as cell (cell)}
				<button
					type="button"
					aria-label={m.watermark_position_cell({ cell: cell + 1 })}
					aria-pressed={settings.position === cell}
					class="grid size-8 cursor-pointer place-items-center rounded-control border border-border bg-input hover:border-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus disabled:cursor-default aria-pressed:border-foreground"
					onclick={() => (settings.position = cell)}
				>
					{#if settings.position === cell}<Check class="size-4" aria-hidden="true" />{/if}
				</button>
			{/each}
		</div>
	</fieldset>
	<div class="grid min-w-0 gap-4">
		<div>
			<p class="mb-1 text-sm font-semibold">{m.watermark_size()}</p>
			<SizeSlider bind:value={settings.size} label={m.watermark_size()} />
		</div>
		<label class="grid gap-1 text-sm font-semibold">
			<span class="flex flex-wrap justify-between gap-x-2">
				{m.watermark_opacity()}
				<span aria-hidden="true">{opacity}</span>
			</span>
			<input
				type="range"
				min="0"
				max="100"
				step="1"
				aria-valuetext={opacity}
				bind:value={settings.watermarkOpacity}
				class="h-8 w-full accent-foreground"
			/>
		</label>
	</div>
	<fieldset disabled={settings.mark === 'logo'} class="min-w-0 disabled:opacity-45 xl:col-span-2">
		<legend class="mb-2 text-sm font-semibold">{m.watermark_text_color()}</legend>
		<div class="flex flex-wrap items-end gap-2">
			<Input
				type="color"
				value={settings.watermarkTextColor}
				aria-label={m.watermark_text_color()}
				oninput={(event) => pickColor(event.currentTarget.value)}
				class="size-11 cursor-pointer p-1"
			/>
			<label class="grid min-w-0 gap-1 text-sm text-muted-foreground">
				{m.watermark_color_hex()}
				<Input
					value={hexEdit ?? settings.watermarkTextColor}
					aria-invalid={invalidHex || undefined}
					aria-describedby={invalidHex ? errorId : undefined}
					oninput={(event) => typeHex(event.currentTarget.value)}
					autocomplete="off"
					spellcheck={false}
					class="w-36 max-w-full font-mono"
				/>
			</label>
		</div>
		{#if invalidHex}<p id={errorId} class="mt-1 text-sm text-danger">
				{m.watermark_color_invalid()}
			</p>{/if}
	</fieldset>
</div>
