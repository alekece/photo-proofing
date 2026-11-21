<script lang="ts">
	import { tick } from 'svelte';
	import Radio from '#lib/components/ui/Radio/Radio.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { saveLanguage } from '#lib/i18n/language.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale, locales, setLocale, type Locale } from '#lib/paraglide/runtime.js';

	let { disabled = false, saving = $bindable(false) }: { disabled?: boolean; saving?: boolean } =
		$props();

	/** Each language is offered in its own name, so it can be found whatever is shown now. */
	const AUTONYMS: Record<Locale, string> = { en: 'English', fr: 'Français' };
	const current = getLocale();
	const group = $props.id();

	async function choose(event: MouseEvent & { currentTarget: HTMLInputElement }, language: Locale) {
		// Keep the checked choice tied to the saved locale, including on failure.
		event.preventDefault();

		if (language === current || saving || disabled) return;

		const picker = event.currentTarget.closest('[role="radiogroup"]');
		const hadFocus = picker?.contains(document.activeElement);

		saving = true;

		try {
			await saveLanguage(language);
		} catch {
			saving = false;
			toasts.error(m.language_save_failed());
			await tick();

			if (hadFocus && document.activeElement === document.body) {
				picker?.querySelector<HTMLInputElement>('input:checked')?.focus();
			}

			return;
		}

		// Reloads the page, now rendered in the saved language.
		setLocale(language);
	}
</script>

<div
	role="radiogroup"
	aria-label={m.settings_language()}
	class="grid max-w-[23.125rem] grid-cols-2 gap-3 sm:gap-[1.125rem]"
>
	{#each locales as language (language)}
		<label
			lang={language}
			class="relative cursor-pointer rounded-card border control-choice px-4 py-3 font-semibold has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-50 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2"
		>
			<Radio
				name={group}
				value={language}
				checked={language === current}
				disabled={saving || disabled}
				class="cursor-inherit absolute inset-0 m-0 size-full opacity-0"
				onclick={(event) => void choose(event, language)}
			/>
			{AUTONYMS[language]}
		</label>
	{/each}
</div>
