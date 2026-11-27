<script lang="ts">
	import ChoiceCards from '#lib/components/ui/ChoiceCards/ChoiceCards.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { locales, type Locale } from '#lib/paraglide/runtime.js';

	let { value = $bindable(), describedBy }: { value: Locale; describedBy?: string } = $props();

	/** Each language is offered in its own name, so it can be found whatever is shown now. */
	const AUTONYMS: Record<Locale, string> = { en: 'English', fr: 'Français' };
	const options = locales.map((locale) => ({
		value: locale,
		label: AUTONYMS[locale],
		lang: locale
	}));
</script>

<ChoiceCards bind:value label={m.settings_language()} {describedBy} {options} />
