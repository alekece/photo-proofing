<script lang="ts">
	import { onMount } from 'svelte';
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import Button from '#lib/components/ui/button/button.svelte';
	import Card from '#lib/components/ui/card/card.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
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
	const title = $derived(data.setupAvailable ? 'Create your photographer account' : 'Sign in');

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (pending) return;
		message = '';
		if (data.setupAvailable && password !== confirmation) {
			message = "Passwords don't match.";

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
			message = error instanceof Error ? error.message : 'Sign-in failed. Please try again.';
			password = '';
			confirmation = '';
			await refreshAll();
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>{title} — PhotoProofing</title></svelte:head>
<PageShell width="narrow" class="flex min-h-screen flex-col justify-center px-6 py-12">
	<div class="text-center">
		<p class="brand mb-6 text-2xl">PhotoProofing</p>
		<h1 class="text-2xl">{title}</h1>
	</div>
	{#if data.setupAvailable}<p>
			Create the single account for this installation. Your galleries will belong to its workspace.
		</p>{/if}
	{#if page.url.searchParams.has('expired')}<p role="status">
			Your session expired. Please sign in again.
		</p>{/if}
	<Card as="form" onsubmit={submit} class="grid gap-5 p-6" aria-busy={pending}>
		<div class="grid gap-2">
			<Label for="email">Email</Label><Input
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
			<Label for="password">Password</Label><Input
				disabled={pending || !ready}
				id="password"
				aria-describedby="password-help"
				type="password"
				autocomplete={data.setupAvailable ? 'new-password' : 'current-password'}
				bind:value={password}
				required
			/>
			<p id="password-help" class="text-sm text-muted-foreground">
				Use 12–128 characters. Spaces are allowed.
			</p>
		</div>
		{#if data.setupAvailable}
			<div class="grid gap-2">
				<Label for="confirmation">Confirm password</Label><Input
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
			>{pending ? 'Please wait…' : data.setupAvailable ? 'Create account' : 'Sign in'}</Button
		>
	</Card>
</PageShell>
