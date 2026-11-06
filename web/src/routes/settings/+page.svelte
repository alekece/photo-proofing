<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { Check, ImagePlus, Undo2, X } from '@lucide/svelte';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import CoverSignaturePicker from '#lib/components/CoverSignaturePicker.svelte';
	import WatermarkEditor from '#lib/components/WatermarkEditor.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
	import {
		SettingsApi,
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

			saved = await api.save({ ...draft });
			show('saved', SAVED_MS);
			if (watermark) toasts.success('Watermarked proofs are being regenerated');
		} catch {
			toasts.error('Unable to save the settings.', () => void save());
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
		if (!file?.type.startsWith('image/')) return;

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
</script>

<svelte:head><title>Settings — PhotoProofing</title></svelte:head>
<AppHeader back={{ href: '/dashboard', label: 'All galleries' }} onsignout={logout} />
<PageShell>
	{#if message}<p role="alert" class="text-danger">{message}</p>{/if}
	<div class="flex flex-wrap items-center justify-between gap-3">
		<h1 class="text-3xl sm:text-4xl">Settings</h1>
		<div class="flex items-center gap-2">
			<Button variant="outline" disabled={!dirty} onclick={cancel}>
				<span
					class="mr-2 inline-flex {feedback === 'reverted' ? 'animate-revert-spin' : ''}"
					aria-hidden="true"><Undo2 size={16} /></span
				>
				{feedback === 'reverted' ? 'Reverted' : 'Cancel'}
			</Button>
			<Button
				disabled={(!dirty && feedback !== 'saved') || saving}
				onclick={save}
				class={feedback === 'saved' ? 'animate-save-pop !bg-ok !text-inverse' : ''}
			>
				{#if feedback === 'saved'}<Check
						size={16}
						class="mr-1"
						aria-hidden="true"
					/>Saved{:else}Save changes{/if}
			</Button>
		</div>
	</div>
	<span role="status" class="sr-only">
		{feedback === 'saved' ? 'Saved' : feedback === 'reverted' ? 'Reverted' : ''}
	</span>

	<section class="grid gap-3" aria-labelledby="studio-heading">
		<h2 id="studio-heading" class="text-xl">Studio</h2>
		<div class="flex flex-wrap items-end gap-4">
			<div class="grid gap-1">
				<span class="font-medium">Logo</span>
				{#if draft.logo}
					<div
						class="relative grid size-20 place-items-center rounded-card border border-border-strong bg-input"
					>
						<img src={draft.logo} alt="Studio logo" class="max-h-16 max-w-16" />
						<button
							type="button"
							aria-label="Remove logo"
							class="absolute -top-2 -right-2 grid size-6 cursor-pointer place-items-center rounded-full bg-foreground text-inverse"
							onclick={removeLogo}><X size={14} aria-hidden="true" /></button
						>
					</div>
				{:else}
					<label
						class="grid size-20 cursor-pointer place-items-center rounded-card border border-dashed border-border-strong text-muted-foreground focus-within:outline-3 focus-within:outline-offset-3"
					>
						<ImagePlus size={22} aria-hidden="true" />
						<span class="sr-only">Upload logo</span>
						<input type="file" accept="image/*" class="sr-only" onchange={pickLogo} />
					</label>
				{/if}
			</div>
			<div class="grid min-w-48 flex-1 gap-1">
				<Label for="studio-name">Studio name</Label>
				<Input id="studio-name" bind:value={draft.studioName} autocomplete="organization" />
			</div>
		</div>
	</section>

	<section class="grid gap-3" aria-labelledby="watermark-heading">
		<h2 id="watermark-heading" class="text-xl">Watermark on proofs</h2>
		<p class="text-sm text-muted-foreground">
			The same watermark is used by every gallery that has watermarking on.
		</p>
		<WatermarkEditor bind:settings={draft} />
	</section>

	<section class="grid gap-3" aria-labelledby="signature-heading">
		<h2 id="signature-heading" class="text-xl">Cover signature</h2>
		<CoverSignaturePicker
			bind:value={draft.signature}
			studioName={draft.studioName}
			logo={draft.logo}
		/>
	</section>
</PageShell>
