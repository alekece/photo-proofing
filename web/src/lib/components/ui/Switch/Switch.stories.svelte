<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import Switch from './Switch.svelte';

	const { Story } = defineMeta({ title: 'UI/Switch', component: Switch });
</script>

{#snippet field(checked: boolean, disabled: boolean)}
	<div class="flex items-center gap-2">
		<Switch id="switch-watermark" {checked} {disabled} />
		<Label for="switch-watermark">{m.settings_watermark()}</Label>
	</div>
{/snippet}

<Story name="Off" asChild>{@render field(false, false)}</Story>
<Story name="On" asChild>{@render field(true, false)}</Story>
<Story name="Disabled" asChild>{@render field(false, true)}</Story>
