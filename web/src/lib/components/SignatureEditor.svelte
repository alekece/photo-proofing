<script lang="ts">
	import { Check, RotateCcw } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import {
		normalizeWatermarkHex,
		SIGNATURES,
		type Signature,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import Button from './ui/Button/Button.svelte';
	import Input from './ui/Input/Input.svelte';
	import MarkLogo from './MarkLogo.svelte';
	import SettingsEditor from './SettingsEditor.svelte';
	import SignaturePreview from './SignaturePreview.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import { CHOICE, GLYPH } from './settings-toolbar.js';

	let { settings = $bindable() }: { settings: WorkspaceSettings } = $props();

	const LABELS: Record<Signature, () => string> = {
		name: m.common_name,
		logo: m.common_logo,
		both: m.signature_both,
		none: m.signature_none
	};
	const noLogo = $derived(settings.logo === null);
	const hasName = $derived(settings.signature === 'name' || settings.signature === 'both');
</script>

<SettingsEditor label={m.signature_preview()}>
	{#snippet toolbar()}
		<fieldset class="min-w-0">
			<legend class="mb-1 text-sm font-semibold">{m.signature_label()}</legend>
			<div class="flex flex-wrap gap-1.5">
				{#each SIGNATURES as signature (signature)}
					<Button
						variant="outline"
						title={LABELS[signature]()}
						aria-label={LABELS[signature]()}
						aria-pressed={settings.signature === signature}
						disabled={noLogo && (signature === 'logo' || signature === 'both')}
						class={CHOICE}
						onclick={() => (settings.signature = signature)}
					>
						<span class={GLYPH} aria-hidden="true">
							{#if signature === 'logo' || signature === 'both'}
								<MarkLogo logo={settings.logo} class="size-6" />
							{/if}
							{#if signature === 'name' || signature === 'both'}Aa{/if}
							{#if signature === 'none'}<span class="font-sans text-sm not-italic"
									>{m.signature_none()}</span
								>{/if}
						</span>
						{#if settings.signature === signature}<Check
								class="absolute top-0.5 right-0.5 size-3"
								aria-hidden="true"
							/>{/if}
					</Button>
				{/each}
			</div>
		</fieldset>
		<SizeSlider bind:value={settings.signatureSize} label={m.signature_size()} />
		<fieldset disabled={!hasName} class="min-w-0 disabled:opacity-45">
			<legend class="mb-1 text-sm font-semibold">{m.watermark_text_color()}</legend>
			<div class="flex flex-wrap items-center gap-2">
				<Input
					type="color"
					value={settings.signatureTextColor ?? '#FFFFFF'}
					aria-label={m.watermark_text_color()}
					oninput={(event) =>
						(settings.signatureTextColor = normalizeWatermarkHex(event.currentTarget.value))}
					class="size-11 cursor-pointer p-1"
				/>
				<Button
					variant="ghost"
					class="size-11 p-0"
					disabled={settings.signatureTextColor === null}
					title={m.signature_reset_color()}
					aria-label={m.signature_reset_color()}
					onclick={() => (settings.signatureTextColor = null)}
				>
					<RotateCcw size={16} aria-hidden="true" />
				</Button>
			</div>
		</fieldset>
	{/snippet}
	{#snippet preview()}<SignaturePreview {settings} />{/snippet}
</SettingsEditor>
