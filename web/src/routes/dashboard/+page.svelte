<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { authRequest, createActivityRenewal } from '#lib/auth.js';
	import Button from '#lib/components/ui/button/button.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state(false);
	let message = $state('');
	let activity = $derived(
		createActivityRenewal(
			data.idleTimeoutMs,
			() => authRequest('renew'),
			() => {
				void goto('/login?expired=1', { refreshAll: true });
			}
		)
	);
	afterNavigate(() => {
		void activity();
	});
	onMount(() => {
		const renew = () => {
			void activity();
		};
		window.addEventListener('pointerdown', renew);
		window.addEventListener('keydown', renew);
		return () => {
			window.removeEventListener('pointerdown', renew);
			window.removeEventListener('keydown', renew);
		};
	});
	async function logout() {
		if (pending) return;
		pending = true;
		message = '';
		try {
			await authRequest('logout');
			await goto('/login', { refreshAll: true });
		} catch (error) {
			message = error instanceof Error ? error.message : 'Unable to sign out. Please try again.';
		} finally {
			pending = false;
		}
	}
</script>

<svelte:head><title>Dashboard — PhotoProofing</title></svelte:head>
<main class="mx-auto grid max-w-4xl gap-6 p-6">
	<header class="flex flex-wrap items-center justify-between gap-4">
		<h1 class="text-2xl font-semibold">Dashboard</h1>
		<Button onclick={logout} disabled={pending}>{pending ? 'Signing out…' : 'Sign out'}</Button>
	</header>
	<p>Your photographer workspace is ready. Gallery management is coming next.</p>
	{#if message}<p role="alert">{message}</p>{/if}
</main>
