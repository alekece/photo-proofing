<script lang="ts">
	import { onMount } from 'svelte';
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { AuthError, authRequest } from '#lib/auth.js';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import PageShell from '#lib/components/ui/PageShell/PageShell.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let email = $state('');
	let password = $state('');
	let confirmation = $state('');
	let ready = $state(false);
	onMount(() => {
		ready = true;
	});
	let pending = $state(false);
	let message = $state('');
	const title = $derived(data.setupAvailable ? m.login_title_setup() : m.login_title());

	/** Sign-in refusals get their own wording; anything else reads like any other API error. */
	function refusal(error: AuthError) {
		if (error.status === 401) return m.login_invalid_credentials();
		if (error.status === 409) return m.login_setup_closed();
		if (error.status === 429) return m.login_too_many_attempts();

		return error.message;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		message = '';
		if (data.setupAvailable && password !== confirmation) {
			message = m.login_passwords_differ();

			return;
		}
		pending = true;
		try {
			await authRequest(data.setupAvailable ? 'setup' : 'login', {
				email,
				password
			});
			password = '';
			confirmation = '';
			await goto('/dashboard', { refreshAll: true });
		} catch (error) {
			message = error instanceof AuthError ? refusal(error) : m.login_failed();
			password = '';
			confirmation = '';
			await refreshAll();
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>{title} — PhotoProofing</title></svelte:head>
<PageShell width="narrow" class="flex w-full flex-1 flex-col justify-center px-6 py-12">
	<div class="text-center">
		<p class="brand mb-6 text-2xl">PhotoProofing</p>
		<h1 class="text-2xl">{title}</h1>
	</div>
	{#if data.setupAvailable}<p>
			{m.login_setup_intro()}
		</p>{/if}
	{#if page.url.searchParams.has('expired')}<p role="status">
			{m.login_session_expired()}
		</p>{/if}
	<Card as="form" onsubmit={submit} class="grid gap-5 p-6" aria-busy={pending}>
		<div class="grid gap-2">
			<Label for="email" class="font-normal">{m.login_email()}</Label><Input
				disabled={pending || !ready}
				id="email"
				type="email"
				autocomplete="username"
				bind:value={email}
				required
				maxlength={254}
			/>
		</div>
		<div class="grid gap-2">
			<Label for="password" class="font-normal">{m.login_password()}</Label><Input
				disabled={pending || !ready}
				id="password"
				aria-describedby="password-help"
				type="password"
				autocomplete={data.setupAvailable ? 'new-password' : 'current-password'}
				bind:value={password}
				required
			/>
			<p id="password-help" class="text-sm text-muted-foreground">
				{m.login_password_help()}
			</p>
		</div>
		{#if data.setupAvailable}
			<div class="grid gap-2">
				<Label for="confirmation" class="font-normal">{m.login_confirm_password()}</Label><Input
					disabled={pending || !ready}
					id="confirmation"
					type="password"
					autocomplete="new-password"
					bind:value={confirmation}
					required
				/>
			</div>
		{/if}
		{#if message}<p role="alert" class="text-sm text-danger">{message}</p>{/if}
		<Button type="submit" disabled={pending || !ready}
			>{pending
				? m.login_please_wait()
				: data.setupAvailable
					? m.login_create_account()
					: m.login_submit()}</Button
		>
	</Card>
</PageShell>
