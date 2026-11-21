<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { WorkspaceSettings } from '#lib/settings/settings.js';
	import StudioSignature from './StudioSignature.svelte';

	let { settings }: { settings: WorkspaceSettings } = $props();
</script>

<div
	class="absolute inset-0 bg-[linear-gradient(rgb(0_0_0/0.25),transparent_30%,transparent_50%,rgb(0_0_0/0.55))]"
></div>
<StudioSignature
	studio={settings.studioName.trim() || m.watermark_your_studio()}
	logo={settings.logo}
	signature={settings.signature}
	size={settings.signatureSize}
	textColor={settings.signatureTextColor}
	class="absolute inset-x-0 top-[4cqw] z-10 font-serif text-[2.8cqw] text-inverse italic"
/>
<span
	class="absolute bottom-[4cqw] left-[4cqw] z-10 font-serif text-[5cqw] leading-none text-inverse"
	>{m.gallery_title_label()}</span
>
