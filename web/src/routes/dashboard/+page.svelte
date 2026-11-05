<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { navigating } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import GoldenHourEmpty from '#lib/components/GoldenHourEmpty.svelte';
	import NewGalleryDialog from '#lib/components/NewGalleryDialog.svelte';
	import Card from '#lib/components/ui/card/card.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import Select from '#lib/components/ui/select/select.svelte';
	import { parseGallerySort, type GalleryState } from '#lib/gallery/schemas.js';
	import { STATE_LABELS, splitStates } from '#lib/gallery/states.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { formatEdited, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state(false);
	let message = $state('');
	let dialogOpen = $state(false);
	let query = $state('');
	// Only a same-page navigation (changing the sort) shows the loading cards.
	let loading = $derived(navigating.to?.route.id === '/dashboard');
	// Plain substring match on the displayed title; never a regular expression.
	let visible = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return (data.galleries ?? []).filter((gallery) =>
			(gallery.title ?? UNTITLED_GALLERY).toLowerCase().includes(needle)
		);
	});
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
	function show(sort: string, states: GalleryState[]) {
		const chosen = states.map((state) => `&state=${state}`).join('');

		void goto(`?sort=${sort}${chosen}`, { reset: false });
	}
	function changeSort(event: Event) {
		show(parseGallerySort((event.currentTarget as HTMLSelectElement).value), data.states);
	}
	function clearAll() {
		query = '';
		show(data.sort, []);
	}
</script>

<svelte:head><title>Dashboard — PhotoProofing</title></svelte:head>
<AppHeader onsignout={logout} />
<PageShell>
	<div class="flex flex-wrap items-end justify-between gap-4">
		<h1 class="text-3xl">Dashboard</h1>
		<Button onclick={() => (dialogOpen = true)}>New gallery</Button>
	</div>
	{#if message}<p role="alert" class="text-danger">{message}</p>{/if}

	{#if loading || data.states.length > 0 || (data.galleries && data.galleries.length > 0)}
		<div class="flex flex-wrap items-center gap-4">
			<Input
				type="search"
				aria-label="Search galleries"
				placeholder="Search title"
				bind:value={query}
				class="max-w-64"
			/>
			<div class="flex items-center gap-2">
				<Label for="sort" class="text-sm font-normal">Sort by</Label>
				<Select id="sort" value={data.sort} onchange={changeSort}>
					<option value="updated">Last edited</option>
					<option value="created">Date created</option>
					<option value="title">Title</option>
				</Select>
			</div>
			<GalleryStateFilter selected={data.states} onchange={(next) => show(data.sort, next)} />
		</div>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">Loading galleries…</p>
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-busy="true">
			{#each [1, 2, 3] as n (n)}
				<li class="aspect-[3/2] overflow-hidden rounded-card border border-border">
					<Placeholder background="sheet" loading />
				</li>
			{/each}
		</ul>
	{:else if data.galleries === null}
		<p role="alert">Couldn’t load your galleries. Please reload the page to try again.</p>
	{:else if data.galleries.length === 0 && data.states.length === 0}
		<GoldenHourEmpty />
	{:else if visible.length === 0}
		<EmptyState class="rounded-dialog">
			<h2 class="text-xl">No galleries match</h2>
			<p class="text-muted-foreground">Try a different title or filter.</p>
			<Button variant="outline" onclick={clearAll}>Clear search and filter</Button>
		</EmptyState>
	{:else}
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
			{#each visible as gallery (gallery.id)}
				{@const { progress, flags } = splitStates(gallery.states)}
				<Card as="li" class="overflow-hidden focus-within:ring-2 focus-within:ring-foreground">
					<a
						href={`/dashboard/${gallery.id}`}
						class="block text-foreground no-underline outline-none"
					>
						<div class="aspect-[3/2]">
							{#if gallery.hero_photo_id}
								<img
									src={previewUrl(gallery.id, gallery.hero_photo_id)}
									alt=""
									loading="lazy"
									class="size-full object-cover"
								/>
							{:else}
								<Placeholder background="sheet" icon label="Your hero photo will appear here" />
							{/if}
						</div>
						<div class="grid gap-1 p-4">
							<h2 class="truncate text-base" class:text-muted-foreground={gallery.title === null}>
								{gallery.title ?? UNTITLED_GALLERY}
							</h2>
							<p class="flex flex-wrap gap-1.5 text-xs">
								<span class="rounded-full border border-border px-2 py-0.5 font-medium">
									{STATE_LABELS[progress]}
								</span>
								{#each flags as flag (flag)}
									<span class="rounded-full border border-border px-2 py-0.5 text-muted-foreground">
										{STATE_LABELS[flag]}
									</span>
								{/each}
							</p>
							<p class="text-sm text-muted-foreground">
								{gallery.layout === 'masonry' ? 'Masonry' : 'Grid'} · Edited {formatEdited(
									gallery.updated_at
								)}
							</p>
						</div>
					</a>
				</Card>
			{/each}
		</ul>
	{/if}
</PageShell>
{#if dialogOpen}
	<NewGalleryDialog onclose={() => (dialogOpen = false)} />
{/if}
