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
	import Button from '#lib/components/ui/Button/Button.svelte';
	import { Plus, Search } from '@lucide/svelte';
	import GalleryStateFilter from '#lib/components/GalleryStateFilter.svelte';
	import GallerySortMenu from '#lib/components/GallerySortMenu.svelte';
	import type { GallerySort, GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, DEFAULT_STATES } from '#lib/gallery/states.js';
	import { galleryStatus, UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state(false);
	let message = $state('');

	const gridClass =
		'grid grid-cols-2 gap-2.5 px-2.5 sm:grid-cols-[repeat(auto-fill,minmax(230px,1fr))] sm:gap-5 sm:px-8';

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
	// Archived galleries stay out of the totals, as in the prototype.
	let totals = $derived.by(() => {
		const live = all.filter((gallery) => galleryStatus(gallery.states) !== 'archived');

		return {
			galleries: live.length,
			photos: live.reduce((sum, gallery) => sum + gallery.photo_count, 0)
		};
	});
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

		return hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';
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
	function changeSort(sort: GallerySort) {
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
<main class="flex flex-1 flex-col">
	<div
		class="grid items-end gap-3 px-3.5 pt-[1.125rem] pb-[1.125rem] sm:grid-cols-[1fr_auto] sm:gap-6 sm:px-8 sm:pt-6 sm:pb-[1.625rem]"
	>
		<div>
			<!-- The name and the news count ("Two galleries have news", the news chips) come with #72. -->
			<h1 class="text-[1.875rem] leading-[1.05] sm:text-[2.75rem]">
				{greeting}. <em class="text-danger">All quiet</em> for now.
			</h1>
		</div>
		<Button class="justify-self-start sm:justify-self-auto" onclick={() => (dialogOpen = true)}
			>+ New gallery</Button
		>
	</div>
	{#if message}<p role="alert" class="px-3.5 pb-3 text-danger sm:px-8">{message}</p>{/if}

	{#if data.galleries !== null}
		{@const none = all.length === 0}
		<div
			class="sticky top-2 z-[6] mx-2.5 mb-4 flex flex-wrap items-center gap-2 rounded-[0.875rem] border border-border bg-panel p-2 sm:mx-8 sm:mb-[1.375rem] sm:gap-3.5 sm:py-2.5 sm:pr-3 sm:pl-4 {none
				? 'opacity-55'
				: ''}"
		>
			<label
				class="flex min-w-40 flex-1 items-center gap-2 rounded-md has-focus-visible:outline-2 has-focus-visible:outline-offset-4 has-focus-visible:outline-focus/40 max-sm:basis-full"
			>
				<Search size={16} aria-hidden="true" class="shrink-0" />
				<input
					type="search"
					aria-label="Search galleries"
					placeholder="Search galleries"
					bind:value={query}
					disabled={none}
					class="w-full border-0 bg-transparent text-[0.9375rem] outline-none placeholder:text-muted-foreground focus-visible:shadow-none focus-visible:outline-none [&::-webkit-search-cancel-button]:hidden"
				/>
			</label>
			<GalleryStateFilter
				{selected}
				{counts}
				disabled={none}
				onchange={(next) => (selected = next)}
			/>
			<GallerySortMenu value={data.sort} disabled={none} onchange={changeSort} />
		</div>
	{/if}
	{#if loading}
		<p class="sr-only" role="status">Loading galleries…</p>
		<ul class={gridClass} aria-busy="true">
			{#each [1, 2, 3, 4] as n (n)}
				<li class="aspect-[3/4] overflow-hidden rounded-card border border-border sm:aspect-[4/5]">
					<Placeholder background="sheet" loading />
				</li>
			{/each}
		</ul>
	{:else if data.galleries === null}
		<p role="alert" class="px-3.5 sm:px-8">
			Couldn’t load your galleries. Please reload the page to try again.
		</p>
	{:else if all.length === 0}
		<EmptyWorkspace />
	{:else}
		<ul class="{gridClass} mb-12">
			{#each visible as gallery (gallery.id)}
				<GalleryCard {gallery} {query} />
			{:else}
				<li class="col-span-full px-0 pt-14 pb-10 text-center text-muted-foreground">
					<h2 class="mb-1.5 text-2xl leading-[1.2] text-foreground">
						{query.trim() !== '' && matching.length === 0
							? `No gallery called “${query.trim()}”`
							: 'No galleries with these statuses'}
					</h2>
					<p>
						{#if hiddenMatches > 0}
							{hiddenMatches === 1 ? '1 match is' : `${hiddenMatches} matches are`} hidden in unselected
							statuses.
						{:else}
							Check the spelling, or try part of the client’s name.
						{/if}
					</p>
					<Button variant="outline" class="mt-3.5" onclick={clearAll}
						>Clear search and status</Button
					>
				</li>
			{/each}
			{#if visible.length > 0}
				<li class="aspect-[3/4] sm:aspect-[4/5]">
					<button
						type="button"
						class="group grid size-full cursor-pointer content-center justify-items-center gap-2.5 rounded-card border-[1.5px] border-dashed border-[#cdbda9] bg-[radial-gradient(120%_80%_at_50%_30%,#faf5ee,transparent_70%)] text-center text-muted-foreground transition-colors duration-250 hover:border-danger hover:bg-[#f8f1e8]"
						onclick={() => (dialogOpen = true)}
					>
						<span
							class="relative mb-2 grid h-16 w-[5.375rem] place-items-center text-[#a8977f] transition duration-300 group-hover:scale-[0.88] group-hover:text-danger"
						>
							<i class="absolute top-0 left-0 size-3.5 border-t-2 border-l-2"></i>
							<i class="absolute top-0 right-0 size-3.5 border-t-2 border-r-2"></i>
							<i class="absolute bottom-0 left-0 size-3.5 border-b-2 border-l-2"></i>
							<i class="absolute right-0 bottom-0 size-3.5 border-r-2 border-b-2"></i>
							<Plus size={30} strokeWidth={1.4} aria-hidden="true" />
						</span>
						<strong
							class="px-2 font-serif text-[1.0625rem] leading-[1.2] font-normal text-foreground italic sm:text-[1.375rem]"
							>Who’s in front of <br />your lens next?</strong
						>
					</button>
				</li>
			{/if}
		</ul>
		<p class="mt-auto px-3.5 pt-6 pb-6 text-[0.8125rem] text-muted-foreground sm:px-8">
			{totals.galleries === 1 ? '1 gallery' : `${totals.galleries} galleries`} · {totals.photos ===
			1
				? '1 photo'
				: `${totals.photos} photos`}
		</p>
	{/if}
</main>
{#if dialogOpen}
	<NewGalleryDialog onclose={() => (dialogOpen = false)} />
{/if}
