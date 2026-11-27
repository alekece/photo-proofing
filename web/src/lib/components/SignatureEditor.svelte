<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		normalizeWatermarkHex,
		SIGNATURES,
		type Signature,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import Button from './ui/Button/Button.svelte';
	import MarkLogo from './MarkLogo.svelte';
	import SettingsColorField from './SettingsColorField.svelte';
	import SettingsEditor from './SettingsEditor.svelte';
	import SignaturePreview from './SignaturePreview.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import { CHOICE, GLYPH } from './settings-toolbar.js';

	let {
		settings = $bindable(),
		hexEdit = $bindable()
	}: { settings: WorkspaceSettings; hexEdit: string | null } = $props();

	const LABELS: Record<Signature, () => string> = {
		name: m.common_name,
		logo: m.common_logo,
		both: m.signature_both,
		none: m.signature_none
	};
	const noLogo = $derived(settings.logo === null);
	const hasName = $derived(settings.signature === 'name' || settings.signature === 'both');

	function chooseSignature(signature: Signature) {
		if (
			(signature === 'logo' || signature === 'none') &&
			hexEdit !== null &&
			normalizeWatermarkHex(hexEdit) === null
		) {
			hexEdit = null;
		}
		settings.signature = signature;
	}
</script>

<SettingsEditor label={m.signature_preview()}>
	{#snippet toolbar()}
		<fieldset class="min-w-0">
			<legend class="mb-1 text-sm font-semibold">{m.settings_mark()}</legend>
			<div class="flex flex-wrap gap-1.5">
				{#each SIGNATURES as signature (signature)}
					<Button
						variant="outline"
						title={LABELS[signature]()}
						aria-label={LABELS[signature]()}
						aria-pressed={settings.signature === signature}
						disabled={noLogo && (signature === 'logo' || signature === 'both')}
						class={CHOICE}
						onclick={() => chooseSignature(signature)}
					>
						<span class={GLYPH} aria-hidden="true">
							{#if signature === 'logo' || signature === 'both'}
								<MarkLogo logo={settings.logo} />
							{/if}
							{#if signature === 'name' || signature === 'both'}Aa{/if}
							{#if signature === 'none'}<span class="font-sans text-sm not-italic">
									{m.signature_none()}
								</span>{/if}
						</span>
					</Button>
				{/each}
			</div>
		</fieldset>
		<SizeSlider bind:value={settings.signatureSize} label={m.settings_size()} />
		<SettingsColorField
			color={settings.signatureTextColor}
			bind:hexEdit
			disabled={!hasName}
			oncolor={(color) => (settings.signatureTextColor = color)}
			onclear={() => (settings.signatureTextColor = null)}
		/>
	{/snippet}
	{#snippet preview()}<SignaturePreview {settings} />{/snippet}
</SettingsEditor>
