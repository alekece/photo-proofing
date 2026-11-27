<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate, invalidateAll } from '$app/navigation';
	import { SlidersHorizontal, Signature, Stamp, Upload, X } from '@lucide/svelte';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import LanguagePicker from '#lib/components/LanguagePicker.svelte';
	import ProofQualityPicker from '#lib/components/ProofQualityPicker.svelte';
	import SignatureEditor from '#lib/components/SignatureEditor.svelte';
	import SaveButton, { type SaveStatus } from '#lib/components/SaveButton.svelte';
	import WatermarkEditor from '#lib/components/WatermarkEditor.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import ChoiceCards from '#lib/components/ui/ChoiceCards/ChoiceCards.svelte';
	import Tabs from '#lib/components/ui/Tabs/Tabs.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import WorkspaceHeading from '#lib/components/ui/WorkspaceHeading/WorkspaceHeading.svelte';
	import { ApiError } from '#lib/http.js';
	import { setLocale } from '#lib/paraglide/runtime.js';
	import {
		LOGO_TYPES,
		SettingsApi,
		normalizeWatermarkHex,
		settingsChanged,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const api = new SettingsApi();
	// svelte-ignore state_referenced_locally
	let saved = $state<WorkspaceSettings>({ ...data.settings });
	// svelte-ignore state_referenced_locally
	let draft = $state<WorkspaceSettings>({ ...data.settings });
	let hexEdit = $state<string | null>(null);
	let signatureHexEdit = $state<string | null>(null);
	let section = $state('general');
	let savingLanguage = $state(false);
	let headerHeight = $state(0);
	const sections = [
		{ id: 'general', label: m.settings_general(), icon: SlidersHorizontal },
		{ id: 'watermark', label: m.watermark_label(), icon: Stamp },
		{ id: 'signature', label: m.signature_label(), icon: Signature }
	];
	let dirty = $derived(settingsChanged(draft, saved));
	let invalidWatermarkHex = $derived(hexEdit !== null && normalizeWatermarkHex(hexEdit) === null);
	let invalidSignatureHex = $derived(
		signatureHexEdit !== null && normalizeWatermarkHex(signatureHexEdit) === null
	);
	let invalidHex = $derived(invalidWatermarkHex || invalidSignatureHex);
	let hasEdits = $derived(dirty || hexEdit !== null || signatureHexEdit !== null);
	let feedback = $state<'saved' | 'reverted' | null>(null);
	let status = $state<SaveStatus>('idle');
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));

	afterNavigate(() => {
		void activity();
	});
	onMount(() => listenForActivity(() => activity()));

	async function save() {
		if (!dirty || invalidHex || status !== 'idle') return;

		status = 'saving';
		savingLanguage = draft.language !== saved.language;
		feedback = null;
		try {
			const submitted = { ...draft };
			const submittedHex = hexEdit;
			const submittedSignatureHex = signatureHexEdit;
			const workspaceChanged =
				saved.studioName !== submitted.studioName ||
				saved.studioMode !== submitted.studioMode ||
				saved.studioPalette !== submitted.studioPalette;

			saved = await api.save(submitted, saved);
			for (const key of Object.keys(submitted) as (keyof WorkspaceSettings)[]) {
				if (draft[key] === submitted[key]) draft = { ...draft, [key]: saved[key] };
			}
			if (hexEdit === submittedHex) hexEdit = null;
			if (signatureHexEdit === submittedSignatureHex) signatureHexEdit = null;
			if (savingLanguage) {
				setLocale(saved.language);
			} else if (workspaceChanged) {
				await invalidateAll();
			}
			feedback = 'saved';
		} catch (error) {
			if (error instanceof ApiError && (error.status === 422 || error.status === 413)) {
				toasts.error(error.message);
			} else {
				toasts.error(m.studio_save_failed(), () => void save());
			}
		} finally {
			status = 'idle';
			savingLanguage = false;
		}
	}

	function cancel() {
		if (!hasEdits || status === 'saving') return;

		draft = { ...saved };
		hexEdit = null;
		signatureHexEdit = null;
		feedback = 'reverted';
	}

	function pickLogo(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];

		input.value = '';
		if (!file) return;
		if (!LOGO_TYPES.includes(file.type)) return void toasts.error(m.studio_logo_type());

		const reader = new FileReader();

		reader.onload = () => (draft.logo = reader.result as string);
		reader.readAsDataURL(file);
	}

	function removeLogo() {
		draft.logo = null;
		if (draft.mark !== 'name') draft.mark = 'name';
		if (draft.signature === 'logo' || draft.signature === 'both') draft.signature = 'name';
	}

	const action =
		'text-sm font-semibold! tracking-normal! transition-[background-color,opacity] duration-300 disabled:opacity-35';
</script>

{#snippet studio()}
	<div class="flex items-center gap-4">
		{#if draft.logo}
			<div
				class="group/logo relative grid size-16 flex-none place-items-center rounded-card border border-border bg-input"
			>
				<img src={draft.logo} alt={m.studio_logo_alt()} class="max-h-[2.375rem] max-w-[2.375rem]" />
				<Button
					variant="ghost"
					aria-label={m.studio_logo_remove()}
					class="absolute -top-2 -right-2 size-8 min-h-0 rounded-full bg-foreground p-0 text-inverse opacity-0 transition-opacity group-hover/logo:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
					onclick={removeLogo}
				>
					<X size={13} aria-hidden="true" />
				</Button>
			</div>
		{:else}
			<label
				class="grid size-16 flex-none cursor-pointer place-content-center justify-items-center gap-0.5 rounded-[0.875rem] border-[1.5px] border-dashed border-border text-xs font-semibold text-muted-foreground focus-within:outline-3 focus-within:outline-offset-3"
			>
				<Upload size={20} strokeWidth={1.8} aria-hidden="true" />
				{m.common_logo()}
				<input type="file" accept="image/png,image/jpeg" class="sr-only" onchange={pickLogo} />
			</label>
		{/if}
		<Input
			bind:value={draft.studioName}
			aria-label={m.studio_name()}
			placeholder={m.studio_name()}
			autocomplete="organization"
			class="max-w-[32.5rem] flex-1"
		/>
	</div>
{/snippet}

{#snippet settingsHeader()}
	<div
		bind:offsetHeight={headerHeight}
		class="sticky top-[env(safe-area-inset-top)] z-20 bg-background workspace-inset py-2.5 sm:py-3 md:col-start-2 md:row-start-1"
	>
		<div class="flex flex-wrap items-center justify-between gap-2">
			<WorkspaceHeading title={m.header_settings()} />
			<div class="ml-auto flex gap-2">
				<Button
					variant="outline"
					size="sm"
					disabled={!hasEdits || status === 'saving'}
					onclick={cancel}
					class="{action} border-[1.5px]! border-border! bg-input!"
				>
					{m.common_cancel()}
				</Button>
				<SaveButton
					{status}
					disabled={status === 'idle' && (!dirty || invalidHex)}
					onclick={save}
					class={action}
				/>
			</div>
		</div>
		{#if invalidHex && !(section === 'watermark' && invalidWatermarkHex) && !(section === 'signature' && invalidSignatureHex)}
			<p aria-live="polite" class="mt-2 text-sm text-danger">
				{m.settings_color_invalid_section({
					section: invalidWatermarkHex ? m.watermark_label() : m.signature_label()
				})}
			</p>
		{/if}
	</div>
{/snippet}

<svelte:head><title>{m.header_settings()} — PhotoProofing</title></svelte:head>
<main class="settings-page flex min-w-0 flex-1 flex-col">
	<span role="status" class="sr-only">
		{feedback === 'reverted' ? m.studio_reverted() : feedback === 'saved' ? m.studio_saved() : ''}
	</span>

	<div
		class="flex min-w-0 flex-1 flex-col [&_:focus-visible]:scroll-mt-[calc(var(--settings-header-height)_+_env(safe-area-inset-top)_+_0.25rem)]"
		style:--settings-header-height="{headerHeight}px"
	>
		<Tabs
			bind:value={section}
			items={sections}
			label={m.studio_sections()}
			sidebar
			listClass="md:sticky md:top-0 md:h-dvh md:self-start"
			header={settingsHeader}
		>
			{#snippet panel(active)}
				<fieldset disabled={savingLanguage} class="grid min-w-0 gap-4">
					{#if active === 'general'}
						<div class="grid max-w-[48rem] min-w-0 gap-5">
							<section class="grid gap-2">
								<h2 class="text-xl">{m.studio_group_studio()}</h2>
								{@render studio()}
							</section>
							<section class="grid gap-2 border-t border-border pt-4">
								<h2 class="text-xl">{m.studio_display_mode()}</h2>
								<p id="settings-photo-presentation-hint" class="mb-2 text-sm text-muted-foreground">
									{m.studio_display_mode_hint()}
								</p>
								<ChoiceCards
									describedBy="settings-photo-presentation-hint"
									label={m.studio_display_mode()}
									bind:value={draft.studioMode}
									options={[
										{ value: 'full_image', label: m.studio_full_image() },
										{ value: 'print', label: m.studio_print() }
									]}
								/>
							</section>
							<section class="grid gap-2 border-t border-border pt-4">
								<h2 class="text-xl">{m.studio_display_palette()}</h2>
								<p id="settings-studio-palette-hint" class="mb-2 text-sm text-muted-foreground">
									{m.studio_display_palette_hint()}
								</p>
								<ChoiceCards
									describedBy="settings-studio-palette-hint"
									label={m.studio_display_palette()}
									bind:value={draft.studioPalette}
									options={[
										{ value: 'warm', label: m.studio_warm() },
										{ value: 'neutral', label: m.studio_neutral() }
									]}
								/>
							</section>
							<section class="grid gap-2 border-t border-border pt-4">
								<h2 class="text-xl">{m.settings_language()}</h2>
								<p id="settings-language-hint" class="mb-2 text-sm text-muted-foreground">
									{m.settings_language_hint()}
								</p>
								<LanguagePicker bind:value={draft.language} describedBy="settings-language-hint" />
							</section>
							<section class="grid gap-2 border-t border-border pt-4">
								<h2 class="text-xl">{m.quality_label()}</h2>
								<p id="settings-quality-hint" class="mb-2 text-sm text-muted-foreground">
									{m.quality_hint()}
								</p>
								<ProofQualityPicker
									bind:value={draft.proofQuality}
									describedBy="settings-quality-hint"
								/>
							</section>
						</div>
					{:else if active === 'watermark'}
						<WatermarkEditor bind:settings={draft} bind:hexEdit />
					{:else}
						<SignatureEditor bind:settings={draft} bind:hexEdit={signatureHexEdit} />
					{/if}
				</fieldset>
			{/snippet}
		</Tabs>
	</div>
</main>

<style>
	:global(html:has(.settings-page)) {
		scrollbar-gutter: stable;
	}
</style>
