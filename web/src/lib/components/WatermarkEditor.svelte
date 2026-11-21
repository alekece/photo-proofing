<script lang="ts">
	import { Crosshair } from '@lucide/svelte';
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
	import SettingsEditor from './SettingsEditor.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import WatermarkPreview from './WatermarkPreview.svelte';
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
	const minOpacity = 5;
	const rows = ['self-start', 'self-center', 'self-end'];
	const columns = ['justify-self-start', 'justify-self-center', 'justify-self-end'];
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

<p class="text-sm text-muted-foreground">{m.watermark_enabled_per_gallery()}</p>
<SettingsEditor label={m.watermark_preview()}>
	{#snippet toolbar()}
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
					</Button>
				{/each}
			</div>
		</fieldset>
		<SizeSlider bind:value={settings.size} label={m.watermark_size()} />
		<label class="grid min-w-0 gap-1 text-sm font-semibold">
			{m.watermark_opacity()}
			<input
				type="range"
				min={minOpacity}
				max="100"
				step="1"
				aria-valuetext={opacity}
				value={Math.max(minOpacity, settings.watermarkOpacity)}
				oninput={(event) =>
					(settings.watermarkOpacity = Math.max(minOpacity, event.currentTarget.valueAsNumber))}
				class="h-11 w-24 accent-foreground"
			/>
		</label>
		<fieldset disabled={settings.mark === 'logo'} class="min-w-0 disabled:opacity-45">
			<legend class="mb-1 text-sm font-semibold">{m.watermark_text_color()}</legend>
			<div class="flex items-center gap-2">
				<Input
					type="color"
					value={settings.watermarkTextColor}
					aria-label={m.watermark_text_color()}
					oninput={(event) => pickColor(event.currentTarget.value)}
					class="size-11 cursor-pointer p-1"
				/>
				<Input
					value={hexEdit ?? settings.watermarkTextColor}
					aria-label={m.watermark_color_hex()}
					aria-invalid={invalidHex || undefined}
					aria-describedby={invalidHex ? errorId : undefined}
					oninput={(event) => typeHex(event.currentTarget.value)}
					autocomplete="off"
					spellcheck={false}
					class="w-24 px-2 font-mono"
				/>
			</div>
			{#if invalidHex}<p id={errorId} class="mt-1 text-sm text-danger">
					{m.watermark_color_invalid()}
				</p>{/if}
		</fieldset>
	{/snippet}
	{#snippet preview()}<WatermarkPreview {settings} />{/snippet}
	{#snippet overlay()}
		{#if settings.pattern === 'once'}
			<fieldset class="pointer-events-none absolute inset-0">
				<legend class="sr-only">{m.watermark_position()}</legend>
				<div class="grid h-full grid-cols-3 grid-rows-3 p-2 sm:p-3">
					{#each Array.from({ length: 9 }, (_, cell) => cell) as cell (cell)}
						<Button
							variant="outline"
							aria-label={m.watermark_position_cell({ cell: cell + 1 })}
							aria-pressed={settings.position === cell}
							class="pointer-events-auto size-11 min-h-0 control-choice p-0 {rows[
								Math.floor(cell / 3)
							]} {columns[cell % 3]}"
							onclick={() => (settings.position = cell)}
						>
							<Crosshair size={16} aria-hidden="true" />
						</Button>
					{/each}
				</div>
			</fieldset>
		{/if}
	{/snippet}
</SettingsEditor>
