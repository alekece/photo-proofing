<script lang="ts">
	import { onMount } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { navigating } from '$app/state';
	import { authRequest } from '#lib/auth.js';
	import { createSessionActivity, listenForActivity } from '#lib/activity.js';
	import Placeholder from '#lib/components/Placeholder.svelte';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import EmptyWorkspace from '#lib/components/EmptyWorkspace.svelte';
	import GalleryCard from '#lib/components/GalleryCard.svelte';
	import NewGalleryDialog from '#lib/components/NewGalleryDialog.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import { Plus, Search } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import Select from '#lib/components/ui/select/select.svelte';
	import { parseGallerySort, type GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, DEFAULT_STATES } from '#lib/gallery/states.js';
	import { galleryStatus, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state(false);
	let message = $state('');
	let dialogOpen = $state(false);
	let query = $state('');
	// Only a same-page navigation (changing the sort) shows the loading cards.
	let loading = $derived(navigating.to?.route.id === '/dashboard');
	let selected = $state<GalleryState[]>([...DEFAULT_STATES]);
	let all = $derived(data.galleries ?? []);
	let counts = $derived(
		Object.fromEntries(
			ALL_STATES.map((state) => [
				state,
				all.filter((g) => galleryStatus(g.states) === state).length
			])
		) as Record<GalleryState, number>
	);
	// Plain substring match on the displayed title; never a regular expression.
	let matching = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return all.filter((gallery) =>
			(gallery.title ?? UNTITLED_GALLERY).toLowerCase().includes(needle)
		);
	});
	let visible = $derived(
		matching.filter((gallery) => selected.includes(galleryStatus(gallery.states)))
	);
	let hiddenMatches = $derived(matching.length - visible.length);
	let greeting = $derived.by(() => {
		const hour = new Date().getHours();

		return hour < 12 ? 'Good morning.' : hour < 18 ? 'Good afternoon.' : 'Good evening.';
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
	function changeSort(event: Event) {
		const sort = parseGallerySort((event.currentTarget as HTMLSelectElement).value);

		void goto(`?sort=${sort}`, { reset: false });
	}
	function clearAll() {
		query = '';
		selected = [...DEFAULT_STATES];
	}
</script>

<svelte:head><title>Dashboard — PhotoProofing</title></svelte:head>
<!-- The bell joins AppHeader's `actions` once #72 provides unread activity. -->
<AppHeader onsignout={logout} />
<PageShell width="wide">
	<div class="flex flex-wrap items-end justify-between gap-4">
		<div class="grid gap-1">
			<h1 class="text-3xl sm:text-4xl">{greeting}</h1>
			<!-- News chips ("13 selected", "4 new downloads") come with #72. -->
			<p class="text-muted-foreground">
				{all.length === 0
					? 'Your galleries will live here.'
					: 'Here is where your galleries stand.'}
			</p>
		</div>
		<Button onclick={() => (dialogOpen = true)}
			><Plus size={16} aria-hidden="true" />New gallery</Button
		>
	</div>
	{#if message}<p role="alert" class="text-danger">{message}</p>{/if}

	{#if data.galleries !== null}
		{@const none = all.length === 0}
		<div class="flex flex-wrap items-center gap-3">
			<div class="relative max-w-72 grow">
				<Search
					size={16}
					aria-hidden="true"
					class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted-foreground"
				/>
				<Input
					type="search"
					aria-label="Search galleries"
					placeholder="Search title"
					bind:value={query}
					disabled={none}
					class="w-full rounded-full pl-9"
				/>
			</div>
			<GalleryStateFilter
				{selected}
				{counts}
				disabled={none}
				onchange={(next) => (selected = next)}
			/>
			<div class="flex items-center gap-2">
				<Label for="sort" class="text-sm font-normal">Sort by</Label>
				<Select id="sort" value={data.sort} onchange={changeSort} disabled={none}>
					<option value="updated">Last updated</option>
					<option value="created">Newest created</option>
					<option value="title">Title A–Z</option>
				</Select>
			</div>
		</div>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">Loading galleries…</p>
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-busy="true">
			{#each [1, 2, 3] as n (n)}
				<li class="aspect-[4/5] overflow-hidden rounded-card border border-border">
					<Placeholder background="sheet" loading />
				</li>
			{/each}
		</ul>
	{:else if data.galleries === null}
		<p role="alert">Couldn’t load your galleries. Please reload the page to try again.</p>
	{:else if all.length === 0}
		<EmptyWorkspace />
	{:else if visible.length === 0}
		<EmptyState class="rounded-dialog">
			<h2 class="text-xl">
				{query.trim() !== '' && matching.length === 0
					? `No gallery called “${query.trim()}”`
					: 'No galleries with these statuses'}
			</h2>
			{#if hiddenMatches > 0}
				<p class="text-muted-foreground">
					{hiddenMatches === 1 ? '1 match is' : `${hiddenMatches} matches are`} hidden in unselected statuses.
				</p>
			{/if}
			<Button variant="outline" onclick={clearAll}>Clear search and status</Button>
		</EmptyState>
	{:else}
		<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
			{#each visible as gallery (gallery.id)}
				<GalleryCard {gallery} {query} />
			{/each}
			<li class="aspect-[4/5]">
				<button
					type="button"
					class="grid size-full cursor-pointer place-items-center content-center gap-2 rounded-card border-2 border-dashed border-border-strong p-4 text-center text-muted-foreground hover:border-foreground hover:text-foreground"
					onclick={() => (dialogOpen = true)}
				>
					<Plus size={28} aria-hidden="true" />
					<span class="brand text-xl">Who’s in front of your lens next?</span>
				</button>
			</li>
		</ul>
	{/if}
	{#if data.galleries !== null && all.length > 0}
		<footer class="text-sm text-muted-foreground">
			{visible.length === 1 ? '1 gallery' : `${visible.length} galleries`}
		</footer>
	{/if}
</PageShell>
{#if dialogOpen}
	<NewGalleryDialog onclose={() => (dialogOpen = false)} />
{/if}
