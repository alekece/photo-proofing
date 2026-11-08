<script lang="ts">
	import { Check } from '@lucide/svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { saveLanguage } from '#lib/i18n/language.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale, locales, setLocale, type Locale } from '#lib/paraglide/runtime.js';

	let { class: className = '' }: { class?: string } = $props();

	/** Each language is offered in its own name, so it can be found whatever is shown now. */
	const AUTONYMS: Record<Locale, string> = { en: 'English', fr: 'Français' };
	const NAMES: Record<Locale, () => string> = { en: m.language_name_en, fr: m.language_name_fr };

	const current = getLocale();

	let open = $state(false);
	let saving = $state(false);

	async function choose(language: Locale) {
		open = false;
		if (language === current || saving) return;

		saving = true;

		try {
			await saveLanguage(language);
		} catch {
			saving = false;
			toasts.error(m.language_save_failed());

			return;
		}

		// Reloads the page, now rendered in the saved language.
		setLocale(language);
	}
</script>

<Popover
	bind:open
	label={m.language_menu_label({ language: NAMES[current]() })}
	disabled={saving}
	align="end"
	class="{className} text-xs font-semibold tracking-wide"
	panelClass="w-44 text-foreground"
>
	{#snippet trigger()}{current.toUpperCase()}{/snippet}
	<div role="radiogroup" aria-label={m.language_menu_options()} class="grid">
		{#each locales as language (language)}
			<button
				type="button"
				role="radio"
				lang={language}
				aria-checked={language === current}
				class="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-control px-3 text-left text-[0.8125rem] hover:bg-border/60"
				onclick={() => void choose(language)}
			>
				<span class="grid w-4 place-items-center {language === current ? '' : 'invisible'}">
					<Check size={14} aria-hidden="true" />
				</span>
				{AUTONYMS[language]}
			</button>
		{/each}
	</div>
</Popover>
