<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { navigating } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import { createGallery, type GallerySort } from '#lib/gallery/api.js';
	import { formatEdited, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state(false);
	let message = $state('');
	let creating = $state(false);
	let createError = $state('');
	// Only a same-page navigation (changing the sort) shows the loading cards.
	let loading = $derived(navigating.to?.route.id === '/dashboard');
	let activity = $derived(createSessionActivity(data.idleTimeoutMs));
	afterNavigate(() => {
		void activity();
	});
	onMount(() => listenForActivity(() => activity()));
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
	async function newGallery() {
		if (creating) return;
		creating = true;
		createError = '';
		try {
			const gallery = await createGallery();
			await goto(`/dashboard/${gallery.id}`);
		} catch (error) {
			if (error instanceof Error && 'status' in error && error.status === 401) {
				await goto('/login?expired=1', { refreshAll: true });
			} else {
				createError = 'Couldn’t create the gallery. Please try again.';
			}
		} finally {
			creating = false;
		}
	}
	function changeSort(event: Event) {
		const sort = (event.currentTarget as HTMLSelectElement).value as GallerySort;
		void goto(`?sort=${sort}`, { reset: false });
	}
</script>

<svelte:head><title>Dashboard — PhotoProofing</title></svelte:head>
<main class="mx-auto grid max-w-5xl gap-6 p-6">
	<header class="flex flex-wrap items-center justify-between gap-4">
		<h1 class="text-2xl font-semibold">Dashboard</h1>
		<div class="flex flex-wrap items-center gap-2">
			<Button onclick={newGallery} disabled={creating}>
				{creating ? 'Creating…' : 'New gallery'}
			</Button>
			<Button variant="outline" onclick={logout} disabled={pending}>
				{pending ? 'Signing out…' : 'Sign out'}
			</Button>
		</div>
	</header>
	{#if message}<p role="alert">{message}</p>{/if}
	{#if createError}<p role="alert">{createError}</p>{/if}

	{#if loading || (data.galleries && data.galleries.length > 0)}
		<div class="flex items-center gap-2">
			<label for="sort" class="text-sm">Sort by</label>
			<select
				id="sort"
				class="rounded-md border bg-background px-2 py-1 text-sm"
				value={data.sort}
				onchange={changeSort}
			>
				<option value="updated">Last edited</option>
				<option value="created">Date created</option>
				<option value="title">Title</option>
			</select>
		</div>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">Loading galleries…</p>
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-busy="true">
			{#each [1, 2, 3] as n (n)}
				<li class="aspect-[3/2] overflow-hidden rounded-xl border">
					<Placeholder background="sheet" loading />
				</li>
			{/each}
		</ul>
	{:else if data.galleries === null}
		<p role="alert">Couldn’t load your galleries. Please reload the page to try again.</p>
	{:else if data.galleries.length === 0}
		<section class="grid justify-items-center gap-3 rounded-xl border border-dashed p-12 text-center">
			<h2 class="text-lg font-medium">Create your first gallery</h2>
			<p class="text-muted-foreground">
				Galleries hold the photos you share with a client. Start one and set it up in a minute.
			</p>
		</section>
	{:else}
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
			{#each data.galleries as gallery (gallery.id)}
				<li class="overflow-hidden rounded-xl border bg-card focus-within:ring-2 focus-within:ring-ring">
					<a href={`/dashboard/${gallery.id}`} class="block outline-none">
						<div class="aspect-[3/2]">
							<Placeholder background="sheet" icon label="Your hero photo will appear here" />
						</div>
						<div class="grid gap-1 p-4">
							<h2 class="truncate font-medium" class:text-muted-foreground={gallery.title === null}>
								{gallery.title ?? UNTITLED_GALLERY}
							</h2>
							<p class="text-sm text-muted-foreground">
								{gallery.layout === 'masonry' ? 'Masonry' : 'Grid'} · Edited {formatEdited(
									gallery.updated_at
								)}
							</p>
						</div>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</main>
