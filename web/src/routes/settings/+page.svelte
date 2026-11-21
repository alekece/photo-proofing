<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { Undo2, Upload, X } from '@lucide/svelte';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import { getStudioNamePreview } from '#lib/components/WorkspaceLayout.svelte';
	import ButtonFace from '#lib/components/ButtonFace.svelte';
	import ClickRipple from '#lib/components/ClickRipple.svelte';
	import LanguageMenu from '#lib/components/LanguageMenu.svelte';
	import ProofQualityPicker from '#lib/components/ProofQualityPicker.svelte';
	import SignatureEditor from '#lib/components/SignatureEditor.svelte';
	import SaveButton, { type SaveStatus } from '#lib/components/SaveButton.svelte';
	import WatermarkEditor from '#lib/components/WatermarkEditor.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Tabs from '#lib/components/ui/Tabs/Tabs.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import { ApiError } from '#lib/http.js';
	import {
		LOGO_TYPES,
		SettingsApi,
		normalizeWatermarkHex,
		settingsChanged,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import type { PageData } from './$types';

	const REVERTED_MS = 1400;
	const SAVED_MS = 1600;

	let { data }: { data: PageData } = $props();
	const api = new SettingsApi();
	// svelte-ignore state_referenced_locally
	let saved = $state<WorkspaceSettings>({ ...data.settings });
	// svelte-ignore state_referenced_locally
	let draft = $state<WorkspaceSettings>({ ...data.settings });
	let hexEdit = $state<string | null>(null);
	let section = $state('general');
	const sections = [
		{ id: 'general', label: m.settings_general() },
		{ id: 'watermark', label: m.watermark_label() },
		{ id: 'signature', label: m.signature_label() }
	];
	let dirty = $derived(settingsChanged(draft, saved));
	let invalidHex = $derived(hexEdit !== null && normalizeWatermarkHex(hexEdit) === null);
	let hasEdits = $derived(dirty || hexEdit !== null);
	let feedback = $state<'reverted' | null>(null);
	let status = $state<SaveStatus>('idle');
	// A new edit ends the confirmation early so Save is offered again at once.
	let shown = $derived<SaveStatus>(status === 'saved' && hasEdits ? 'idle' : status);
	// Like Saved, Reverted gives way as soon as there is something to cancel again.
	let reverted = $derived(feedback === 'reverted' && !hasEdits);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let savedTimer: ReturnType<typeof setTimeout> | undefined;
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	const previewStudioName = getStudioNamePreview();

	$effect(() => {
		previewStudioName?.(draft.studioName);

		return () => previewStudioName?.(undefined);
	});

	afterNavigate(() => {
		void activity();
	});
	onMount(() => {
		const stop = listenForActivity(() => activity());

		return () => {
			stop();
			clearTimeout(timer);
			clearTimeout(savedTimer);
		};
	});

	function show(kind: 'reverted', ms: number) {
		feedback = kind;
		clearTimeout(timer);
		timer = setTimeout(() => (feedback = null), ms);
	}

	function confirmSaved() {
		status = 'saved';
		clearTimeout(savedTimer);
		savedTimer = setTimeout(() => (status = 'idle'), SAVED_MS);
	}

	async function save() {
		if (!dirty || invalidHex || shown !== 'idle') return;

		status = 'saving';
		clearTimeout(timer);
		clearTimeout(savedTimer);
		feedback = null;
		try {
			const submitted = { ...draft };
			const submittedHex = hexEdit;

			saved = await api.save(submitted, saved);
			for (const key of Object.keys(submitted) as (keyof WorkspaceSettings)[]) {
				if (draft[key] === submitted[key]) draft = { ...draft, [key]: saved[key] };
			}
			if (hexEdit === submittedHex) hexEdit = null;
			confirmSaved();
		} catch (error) {
			if (error instanceof ApiError && (error.status === 422 || error.status === 413)) {
				toasts.error(error.message);
			} else {
				toasts.error(m.studio_save_failed(), () => void save());
			}
			status = 'idle';
		}
	}

	function cancel() {
		if (!hasEdits || status === 'saving') return;

		draft = { ...saved };
		hexEdit = null;
		show('reverted', REVERTED_MS);
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
		'relative inline-grid h-11 min-h-0 px-[1.375rem] max-sm:flex-1 font-semibold! tracking-normal! transition-[background-color,opacity] duration-300 disabled:opacity-35';
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
					onclick={removeLogo}><X size={13} aria-hidden="true" /></Button
				>
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

<svelte:head><title>{m.header_settings()} — PhotoProofing</title></svelte:head>
<main class="min-w-0 flex-1 pb-12">
	<div class="workspace-inset pt-4 sm:pt-6">
		<h1 class="mb-3 text-4xl leading-[1.1] sm:text-[3.25rem]">{m.header_settings()}</h1>
	</div>
	<div
		class="sticky top-[env(safe-area-inset-top)] z-20 flex flex-wrap items-center justify-end gap-2 border-b border-border bg-background workspace-inset py-2"
	>
		{#if invalidHex && section !== 'watermark'}
			<p aria-live="polite" class="mr-auto text-sm text-danger max-sm:basis-full">
				{m.watermark_color_invalid()}
			</p>
		{/if}
		<Button
			variant="outline"
			disabled={!reverted && (!hasEdits || status === 'saving')}
			aria-disabled={reverted || undefined}
			onclick={cancel}
			class="{action} border-[1.5px]! border-border! bg-input! aria-disabled:cursor-default {reverted
				? 'border-[#d8cab7]! bg-[#efe7dc]!'
				: ''}"
		>
			<ButtonFace shown={!reverted}>{m.common_cancel()}</ButtonFace>
			<ButtonFace shown={reverted}>
				{#snippet icon()}
					<span class="relative inline-grid">
						<Undo2 size={16} strokeWidth={2.4} aria-hidden="true" />
						{#if reverted}<ClickRipple />{/if}
					</span>
				{/snippet}
				{m.studio_reverted()}
			</ButtonFace>
		</Button>
		<SaveButton
			status={shown}
			disabled={shown === 'idle' && (!dirty || invalidHex)}
			onclick={save}
			class={action}
		/>
	</div>
	<span role="status" class="sr-only">
		{feedback === 'reverted' ? m.studio_reverted() : ''}
	</span>

	<div class="workspace-inset pt-4 sm:pt-6">
		<Tabs
			bind:value={section}
			items={sections}
			label={m.studio_sections()}
			sidebar
			listClass="min-w-0 overflow-x-auto whitespace-nowrap [&>button]:shrink-0 [&>button]:focus-visible:outline-offset-[-2px]"
		>
			{#snippet panel(active)}
				{#if active === 'general'}
					<div class="grid max-w-[48rem] min-w-0 gap-5">
						<section class="grid gap-2">
							<h2 class="text-xl">{m.studio_group_studio()}</h2>
							{@render studio()}
						</section>
						<section class="grid justify-items-start gap-2 border-t border-border pt-4">
							<h2 class="text-xl">{m.language_menu_options()}</h2>
							<LanguageMenu
								disabled={hasEdits || status === 'saving'}
								align="start"
								class="min-h-11 rounded-control border control-choice px-4 disabled:opacity-50"
							/>
							{#if hasEdits}
								<p class="text-sm text-muted-foreground">{m.settings_language_pending()}</p>
							{/if}
						</section>
						<section class="grid gap-2 border-t border-border pt-4">
							<h2 class="text-xl">{m.quality_label()}</h2>
							<p class="text-muted-foreground">{m.quality_hint()}</p>
							<ProofQualityPicker bind:value={draft.proofQuality} />
						</section>
					</div>
				{:else if active === 'watermark'}
					<WatermarkEditor bind:settings={draft} bind:hexEdit />
				{:else}
					<SignatureEditor bind:settings={draft} />
				{/if}
			{/snippet}
		</Tabs>
	</div>
</main>
