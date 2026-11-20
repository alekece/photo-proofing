<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { SIGNATURES, type Signature, type WorkspaceSettings } from '#lib/settings/settings.js';
	import Button from './ui/Button/Button.svelte';
	import MarkLogo from './MarkLogo.svelte';
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
</script>

<fieldset class="mb-4 min-w-0">
	<legend class="mb-2 text-sm font-semibold">{m.signature_label()}</legend>
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
<p class="mb-1 text-sm font-semibold">{m.signature_size()}</p>
<SizeSlider bind:value={settings.signatureSize} label={m.signature_size()} />
