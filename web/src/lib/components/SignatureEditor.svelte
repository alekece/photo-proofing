<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { SIGNATURES, type Signature, type WorkspaceSettings } from '#lib/settings/settings.js';
	import MarkLogo from './MarkLogo.svelte';
	import SamplePhoto from './SamplePhoto.svelte';
	import SizeSlider from './SizeSlider.svelte';
	import StudioSignature from './StudioSignature.svelte';
	import { TOOLBAR, TOOLBAR_GLYPH, TOOLBAR_ITEM } from './settings-toolbar.js';

	let { settings = $bindable() }: { settings: WorkspaceSettings } = $props();

	const LABELS: Record<Signature, () => string> = {
		name: m.common_name,
		logo: m.common_logo,
		both: m.signature_both,
		none: m.signature_none
	};
	const noLogo = $derived(settings.logo === null);
</script>

<div class={TOOLBAR} role="toolbar" aria-label={m.signature_label()}>
	<div
		role="group"
		aria-label={m.signature_label()}
		class="flex items-center gap-0.5 px-1 sm:gap-1 sm:px-2"
	>
		{#each SIGNATURES as signature (signature)}
			<button
				type="button"
				title={LABELS[signature]()}
				aria-label={LABELS[signature]()}
				aria-pressed={settings.signature === signature}
				disabled={noLogo && (signature === 'logo' || signature === 'both')}
				class={TOOLBAR_ITEM}
				onclick={() => (settings.signature = signature)}
			>
				<span class={TOOLBAR_GLYPH}>
					{#if signature === 'logo' || signature === 'both'}
						<MarkLogo logo={settings.logo} class="size-6" />
					{/if}
					{#if signature === 'name' || signature === 'both'}Aa{/if}
					{#if signature === 'none'}<span class="font-sans text-sm not-italic"
							>{m.signature_none()}</span
						>{/if}
				</span>
			</button>
		{/each}
	</div>
	<SizeSlider bind:value={settings.signatureSize} label={m.signature_size()} />
</div>
<SamplePhoto label={m.signature_preview()}>
	<div
		class="absolute inset-0 bg-[linear-gradient(rgb(0_0_0/0.25),transparent_30%,transparent_50%,rgb(0_0_0/0.55))]"
	></div>
	<StudioSignature
		studio={settings.studioName.trim() || m.watermark_your_studio()}
		logo={settings.logo}
		signature={settings.signature}
		size={settings.signatureSize}
		class="absolute inset-x-0 top-[4cqw] z-10 font-serif text-[2.8cqw] text-inverse italic"
	/>
	<span
		class="absolute bottom-[4cqw] left-[4cqw] z-10 font-serif text-[5cqw] leading-none text-inverse"
		>{m.gallery_title_label()}</span
	>
</SamplePhoto>
