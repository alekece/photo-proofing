<script lang="ts">
	import { onMount } from 'svelte';
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let email = $state('');
	let password = $state('');
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
		pending = true;
		message = '';
		try {
			await authRequest(data.setupAvailable ? 'setup' : 'login', {
				email,
				password
			});
			password = '';
			await goto('/dashboard', { refreshAll: true });
		} catch (error) {
			message = error instanceof Error ? error.message : 'Sign-in failed. Please try again.';
			await refreshAll();
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>{title} — PhotoProofing</title></svelte:head>
<main class="mx-auto flex min-h-screen max-w-md flex-col justify-center gap-6 px-6 py-12">
	<div>
		<p class="mb-2 text-sm text-muted-foreground">PhotoProofing</p>
		<h1 class="text-2xl font-semibold">{title}</h1>
	</div>
	{#if data.setupAvailable}<p>
			Create the single account for this installation. Your galleries will belong to its workspace.
		</p>{/if}
	{#if page.url.searchParams.has('expired')}<p role="status">
			Your session expired. Please sign in again.
		</p>{/if}
	<form onsubmit={submit} class="grid gap-5" aria-busy={pending}>
		<div class="grid gap-2">
			<label for="email">Email</label><Input
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
			<label for="password">Password</label><Input
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
		{#if message}<p role="alert">{message}</p>{/if}
		<Button type="submit" disabled={pending || !ready}
			>{pending ? 'Please wait…' : data.setupAvailable ? 'Create account' : 'Sign in'}</Button
		>
	</form>
</main>
