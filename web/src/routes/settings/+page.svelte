<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { Check, Undo2, Upload, X } from '@lucide/svelte';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import CoverSignaturePicker from '#lib/components/CoverSignaturePicker.svelte';
	import WatermarkEditor from '#lib/components/WatermarkEditor.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
	import {
		LOGO_MAX_BYTES,
		LOGO_TYPES,
		SettingsApi,
		SettingsApiError,
		settingsChanged,
		watermarkChanged,
		type WorkspaceSettings
	} from '#lib/settings/settings.js';
	import type { PageData } from './$types';

	const SAVED_MS = 1600;
	const REVERTED_MS = 1400;

	let { data }: { data: PageData } = $props();
	const api = new SettingsApi();
	// svelte-ignore state_referenced_locally
	let saved = $state<WorkspaceSettings>({ ...data.settings });
	// svelte-ignore state_referenced_locally
	let draft = $state<WorkspaceSettings>({ ...data.settings });
	let dirty = $derived(settingsChanged(draft, saved));
	let feedback = $state<'saved' | 'reverted' | null>(null);
	let saving = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
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
		};
	});

	function show(kind: 'saved' | 'reverted', ms: number) {
		feedback = kind;
		clearTimeout(timer);
		timer = setTimeout(() => (feedback = null), ms);
	}

	async function save() {
		if (!dirty || saving) return;

		saving = true;
		try {
			const watermark = watermarkChanged(draft, saved);

			saved = await api.save({ ...draft }, saved);
			draft = { ...saved };
			show('saved', SAVED_MS);
			if (watermark) toasts.success('Watermarked proofs are being regenerated');
		} catch (error) {
			if (error instanceof SettingsApiError && error.status === 422) {
				toasts.error(error.message);
			} else {
				toasts.error('Unable to save the settings.', () => void save());
			}
		} finally {
			saving = false;
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
		if (!LOGO_TYPES.includes(file.type))
			return void toasts.error('The logo must be a PNG or JPEG image.');
		if (file.size > LOGO_MAX_BYTES) return void toasts.error('The logo must be under 1 MB.');

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
			message = error instanceof Error ? error.message : 'Unable to sign out. Please try again.';
		} finally {
			signingOut = false;
		}
	}

	const action =
		'relative inline-grid h-11 min-h-0 px-[1.375rem] max-sm:flex-1 font-semibold! tracking-normal! transition-[background-color,opacity] duration-300 disabled:opacity-35';
	const face =
		'col-start-1 row-start-1 inline-flex items-center justify-center gap-2 transition-[opacity,transform] duration-300';
</script>

<svelte:head><title>Settings — PhotoProofing</title></svelte:head>
<AppHeader crumb="Settings" onsignout={logout} />
<PageShell
	class="max-w-[75rem] gap-0 px-3.5 pt-5 pb-[3.75rem] sm:px-[clamp(1.25rem,5vw,3.5rem)] sm:pt-9"
>
	{#if message}<p role="alert" class="mb-4 text-danger">{message}</p>{/if}
	<div class="mb-7 flex flex-wrap items-center gap-3">
		<h1 class="m-0 flex-1 text-4xl leading-[1.1] max-sm:basis-full sm:text-[3.25rem]">Settings</h1>
		<Button
			variant="outline"
			disabled={!dirty}
			onclick={cancel}
			class="{action} border-[1.5px]! border-border! bg-input! {feedback === 'reverted'
				? 'border-[#d8cab7]! bg-[#efe7dc]! opacity-100!'
				: ''}"
		>
			<span
				class="{face} {feedback === 'reverted' ? '-translate-y-2 opacity-0' : ''}"
				aria-hidden={feedback === 'reverted'}>Cancel</span
			>
			<span
				class="{face} {feedback === 'reverted' ? '' : 'translate-y-2 opacity-0'}"
				aria-hidden={feedback !== 'reverted'}
			>
				<span class="inline-flex {feedback === 'reverted' ? 'animate-revert-spin' : ''}"
					><Undo2 size={16} strokeWidth={2.4} aria-hidden="true" /></span
				>Reverted
			</span>
		</Button>
		<Button
			disabled={(!dirty && feedback !== 'saved') || saving}
			onclick={save}
			class="{action} {feedback === 'saved' ? 'animate-save-pop bg-ok! opacity-100!' : ''}"
		>
			<span
				class="{face} {feedback === 'saved' ? '-translate-y-2 opacity-0' : ''}"
				aria-hidden={feedback === 'saved'}>Save changes</span
			>
			<span
				class="{face} {feedback === 'saved' ? '' : 'translate-y-2 opacity-0'}"
				aria-hidden={feedback !== 'saved'}
			>
				<Check size={16} strokeWidth={2.6} aria-hidden="true" />Saved
			</span>
		</Button>
	</div>
	<span role="status" class="sr-only">
		{feedback === 'saved' ? 'Saved' : feedback === 'reverted' ? 'Reverted' : ''}
	</span>

	<div class="mb-10 flex items-center gap-5">
		{#if draft.logo}
			<div
				class="group/logo relative grid size-16 flex-none place-items-center rounded-[0.875rem] border-[1.5px] border-border bg-input"
			>
				<img src={draft.logo} alt="Studio logo" class="max-h-[2.375rem] max-w-[2.375rem]" />
				<button
					type="button"
					aria-label="Remove logo"
					class="absolute -top-2 -right-2 grid size-[1.625rem] cursor-pointer place-items-center rounded-full bg-foreground text-inverse opacity-0 transition-opacity group-hover/logo:opacity-100 focus-visible:opacity-100 [@media(hover:none)]:opacity-100"
					onclick={removeLogo}><X size={13} aria-hidden="true" /></button
				>
			</div>
		{:else}
			<label
				class="grid size-16 flex-none cursor-pointer place-content-center justify-items-center gap-0.5 rounded-[0.875rem] border-[1.5px] border-dashed border-border text-xs font-semibold text-muted-foreground focus-within:outline-3 focus-within:outline-offset-3"
			>
				<Upload size={20} strokeWidth={1.8} aria-hidden="true" />
				Logo
				<input type="file" accept="image/png,image/jpeg" class="sr-only" onchange={pickLogo} />
			</label>
		{/if}
		<Input
			bind:value={draft.studioName}
			aria-label="Studio name"
			placeholder="Studio name"
			autocomplete="organization"
			class="min-h-0 max-w-[32.5rem] min-w-0 flex-1 rounded-none border-0 border-b-[1.5px] border-border bg-transparent px-0 py-1.5 font-serif text-[1.625rem] leading-[normal] shadow-none focus:border-foreground"
		/>
	</div>

	<h2 class="mb-4 text-[1.875rem] leading-[1.15]">Watermark on proofs</h2>
	<WatermarkEditor bind:settings={draft} samples={data.samples} />

	<h2 class="mb-4 text-[1.875rem] leading-[1.15]">Cover signature</h2>
	<CoverSignaturePicker
		bind:value={draft.signature}
		studioName={draft.studioName}
		logo={draft.logo}
		cover={data.cover}
	/>
</PageShell>
