<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { normalizeWatermarkHex } from '#lib/settings/settings.js';
	import Input from './ui/Input/Input.svelte';

	let {
		color,
		hexEdit = $bindable(),
		oncolor,
		onclear,
		disabled = false
	}: {
		color: string | null;
		hexEdit: string | null;
		oncolor: (value: string) => void;
		onclear?: () => void;
		disabled?: boolean;
	} = $props();

	const errorId = $props.id();
	const invalid = $derived(hexEdit !== null && normalizeWatermarkHex(hexEdit) === null);

	function inputHex(value: string) {
		if (onclear && value === '') {
			hexEdit = null;
			onclear();
			return;
		}

		hexEdit = value;
		const normalized = normalizeWatermarkHex(value);

		if (normalized !== null) oncolor(normalized);
	}

	function pickColor(value: string) {
		const normalized = normalizeWatermarkHex(value);

		if (normalized === null) return;

		hexEdit = null;
		oncolor(normalized);
	}
</script>

<fieldset {disabled} class="min-w-0 disabled:opacity-45">
	<legend class="mb-1 text-sm font-semibold">{m.watermark_text_color()}</legend>
	<div class="flex flex-wrap items-center gap-2">
		<Input
			type="color"
			value={color ?? '#FFFFFF'}
			aria-label={m.watermark_text_color()}
			oninput={(event) => pickColor(event.currentTarget.value)}
			class="size-11 cursor-pointer p-1"
		/>
		<Input
			value={hexEdit ?? color ?? ''}
			aria-label={m.watermark_color_hex()}
			aria-invalid={invalid || undefined}
			aria-describedby={invalid ? errorId : undefined}
			placeholder={onclear ? m.signature_color_auto() : undefined}
			oninput={(event) => inputHex(event.currentTarget.value)}
			autocomplete="off"
			spellcheck={false}
			class="w-24 px-2 font-mono"
		/>
	</div>
	{#if invalid}
		<p id={errorId} class="mt-1 text-sm text-danger">{m.watermark_color_invalid()}</p>
	{/if}
</fieldset>
