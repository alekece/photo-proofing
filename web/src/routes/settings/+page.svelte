<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { ArrowLeft, Undo2, Upload, X } from '@lucide/svelte';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import ButtonFace from '#lib/components/ButtonFace.svelte';
	import ClickRipple from '#lib/components/ClickRipple.svelte';
	import ProofQualityPicker from '#lib/components/ProofQualityPicker.svelte';
	import SignatureEditor from '#lib/components/SignatureEditor.svelte';
	import SaveButton, { type SaveStatus } from '#lib/components/SaveButton.svelte';
	import WatermarkEditor from '#lib/components/WatermarkEditor.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import PageShell from '#lib/components/ui/PageShell/PageShell.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import {
		LOGO_TYPES,
		SettingsApi,
		SettingsApiError,
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
	let dirty = $derived(settingsChanged(draft, saved));
	let feedback = $state<'reverted' | null>(null);
	let status = $state<SaveStatus>('idle');
	// A new edit ends the confirmation early so Save is offered again at once.
	let shown = $derived<SaveStatus>(status === 'saved' && dirty ? 'idle' : status);
	// Like Saved, Reverted gives way as soon as there is something to cancel again.
	let reverted = $derived(feedback === 'reverted' && !dirty);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let savedTimer: ReturnType<typeof setTimeout> | undefined;
	let signingOut = $state(false);
	let message = $state('');
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));

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
		if (!dirty || shown !== 'idle') return;

		status = 'saving';
		clearTimeout(timer);
		clearTimeout(savedTimer);
		feedback = null;
		try {
			const submitted = { ...draft };

			saved = await api.save(submitted, saved);
			for (const key of Object.keys(submitted) as (keyof WorkspaceSettings)[]) {
				if (draft[key] === submitted[key]) draft = { ...draft, [key]: saved[key] };
			}
			confirmSaved();
		} catch (error) {
			if (error instanceof SettingsApiError && (error.status === 422 || error.status === 413)) {
				toasts.error(error.message);
			} else {
				toasts.error(m.studio_save_failed(), () => void save());
			}
			status = 'idle';
		}
	}

	function cancel() {
		if (!dirty) return;

		draft = { ...saved };
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

	async function logout() {
		if (signingOut) return;

		signingOut = true;
		message = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			message = error instanceof Error ? error.message : m.sign_out_failed();
		} finally {
			signingOut = false;
		}
	}

	const action =
		'relative inline-grid h-11 min-h-0 px-[1.375rem] max-sm:flex-1 font-semibold! tracking-normal! transition-[background-color,opacity] duration-300 disabled:opacity-35';
</script>

<svelte:head><title>{m.header_settings()} — PhotoProofing</title></svelte:head>
<AppHeader onsignout={logout} />
<PageShell
	class="max-w-[75rem] gap-0 px-3.5 pt-5 pb-[3.75rem] sm:px-[clamp(1.25rem,5vw,3.5rem)] sm:pt-9"
>
	<a
		href="/dashboard"
		class="mb-4 inline-flex w-fit items-center gap-1.5 text-sm text-muted-foreground underline-offset-2 hover:underline"
		><ArrowLeft class="size-4" aria-hidden="true" />{m.header_back_to_galleries()}</a
	>
	{#if message}<p role="alert" class="mb-4 text-danger">{message}</p>{/if}
	<div class="mb-7 flex flex-wrap items-center gap-3">
		<h1 class="m-0 flex-1 text-4xl leading-[1.1] max-sm:basis-full sm:text-[3.25rem]">
			{m.header_settings()}
		</h1>
		<Button
			variant="outline"
			disabled={!reverted && (!dirty || status === 'saving')}
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
			disabled={shown === 'idle' && !dirty}
			onclick={save}
			class={action}
		/>
	</div>
	<span role="status" class="sr-only">
		{feedback === 'reverted' ? m.studio_reverted() : ''}
	</span>

	<div class="mb-10 flex items-center gap-5">
		{#if draft.logo}
			<div
				class="group/logo relative grid size-16 flex-none place-items-center rounded-[0.875rem] border-[1.5px] border-border bg-input"
			>
				<img src={draft.logo} alt={m.studio_logo_alt()} class="max-h-[2.375rem] max-w-[2.375rem]" />
				<button
					type="button"
					aria-label={m.studio_logo_remove()}
					class="absolute -top-2 -right-2 grid size-[1.625rem] cursor-pointer place-items-center rounded-full bg-foreground text-inverse opacity-0 transition-opacity group-hover/logo:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
					onclick={removeLogo}><X size={13} aria-hidden="true" /></button
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
			class="min-h-0 max-w-[32.5rem] min-w-0 flex-1 rounded-none border-0 border-b-[1.5px] border-border bg-transparent px-0 py-1.5 font-serif text-[1.625rem] leading-[normal] shadow-none focus:border-foreground"
		/>
	</div>

	<h2 class="mb-4 text-[1.875rem] leading-[1.15]">{m.studio_watermark()}</h2>
	<WatermarkEditor bind:settings={draft} />

	<h2 class="mb-4 text-[1.875rem] leading-[1.15]">{m.signature_label()}</h2>
	<SignatureEditor bind:settings={draft} />

	<h2 class="mt-10 mb-2 text-[1.875rem] leading-[1.15]">{m.quality_label()}</h2>
	<p class="mb-4 text-muted-foreground">{m.quality_hint()}</p>
	<ProofQualityPicker bind:value={draft.proofQuality} />
</PageShell>
