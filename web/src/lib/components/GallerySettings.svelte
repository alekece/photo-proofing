<script lang="ts">
	import {
		Archive,
		ArchiveRestore,
		LayoutDashboard,
		LayoutGrid,
		Link,
		Palette,
		SlidersHorizontal,
		Trash2
	} from '@lucide/svelte';
	import type { Component } from 'svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import Card from '#lib/components/ui/card/card.svelte';
	import IconButtonGroup from '#lib/components/ui/icon-button-group/icon-button-group.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import Switch from '#lib/components/ui/switch/switch.svelte';
	import { toasts } from '#lib/components/ui/toast/toasts.svelte.js';
	import type { Gallery, GalleryChanges, GalleryLayout } from '#lib/gallery/api.js';
	import { endOfLocalDay, toLocalDate } from '#lib/gallery/format.js';

	/**
	 * The side nav: four cards on wide screens, an icon rail on phones whose buttons pop the same
	 * cards out. `gallery` is the last saved state. Inputs save in place: `onsave` sends a patch
	 * and resolves whether it stuck, so a refused value reverts. `schedule`/`cancel` are the page's
	 * keyed debouncer, so its flush on navigation also covers the debounced "included photos" save.
	 */
	let {
		gallery,
		shareToken,
		origin,
		onsave,
		schedule,
		cancel,
		onarchive,
		onrestore,
		ondelete
	}: {
		gallery: Gallery;
		shareToken: string | null;
		origin: string;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
		schedule: (field: string, run: () => void) => void;
		cancel: (field: string) => void;
		onarchive: () => void;
		onrestore: () => void;
		ondelete: () => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let expiryDay = $state(gallery.expires_at ? toLocalDate(gallery.expires_at) : '');
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(gallery.included_photos);
	// svelte-ignore state_referenced_locally
	let layout = $state<GalleryLayout>(gallery.layout);
	// svelte-ignore state_referenced_locally
	let watermark = $state(gallery.watermark_enabled);
	let flash = $state('');
	let popped = $state<string | null>(null);

	const archived = $derived(gallery.states.includes('archived'));
	const cards: { id: string; label: string; icon: Component<{ size?: number }> }[] = [
		{ id: 'link', label: 'Client link', icon: Link },
		{ id: 'access', label: 'Client access', icon: SlidersHorizontal },
		{ id: 'look', label: 'Look', icon: Palette },
		{ id: 'gallery', label: 'Gallery', icon: Archive }
	];
	const layouts = [
		{ id: 'masonry', label: 'Staggered columns', icon: LayoutDashboard },
		{ id: 'grid', label: 'Even rows', icon: LayoutGrid }
	];
	// Phones show only the popped card; wide screens show all of them.
	const show = (id: string) => (popped === id ? 'grid' : 'hidden md:grid');

	async function commit(field: string, changes: GalleryChanges, revert: () => void) {
		if (await onsave(changes)) {
			flash = field;
			setTimeout(() => flash === field && (flash = ''), 1200);
		} else revert();
	}
	const ring = (field: string) => (flash === field ? 'ring-2 ring-ok' : '');

	function setLayout(value: string) {
		const previous = layout;

		layout = value as GalleryLayout;
		void commit('layout', { layout }, () => (layout = previous));
	}
	function setExpiry() {
		void commit(
			'expiry',
			{ expires_at: expiryDay ? endOfLocalDay(expiryDay) : null },
			() => (expiryDay = gallery.expires_at ? toLocalDate(gallery.expires_at) : '')
		);
	}
	/** While typing, a half-typed value is left alone; leaving the field is where it is judged. */
	function saveIncluded(leaving: boolean) {
		const count = included;
		const revert = () => {
			if (leaving) included = gallery.included_photos;
		};

		if (count !== null && (!Number.isInteger(count) || count < 1)) {
			if (leaving) {
				toasts.error('Included photos must be a whole number of at least 1.');
				revert();
			}

			return;
		}
		if (count === gallery.included_photos) return;

		void commit('included', { included_photos: count }, revert);
	}
	function setWatermark(value: boolean) {
		void commit('watermark', { watermark_enabled: value }, () => (watermark = !value));
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') popped = null;
	}}
/>
<aside aria-label="Gallery settings" class="grid content-start gap-4">
	<div class="flex gap-2 md:hidden" role="group" aria-label="Gallery settings sections">
		{#each cards as { id, label, icon: Icon } (id)}
			<button
				type="button"
				aria-label="Show {label}"
				aria-expanded={popped === id}
				title={label}
				class="grid size-11 place-items-center rounded-control border {popped === id
					? 'border-foreground bg-foreground text-inverse'
					: 'border-border-strong'}"
				onclick={() => (popped = popped === id ? null : id)}
			>
				<Icon size={18} />
			</button>
		{/each}
	</div>

	<Card as="section" class="{show('link')} content-start gap-2 p-4">
		<h2 class="text-sm font-medium">Client link</h2>
		{#if shareToken}
			<ShareLink token={shareToken} {origin} />
		{:else}
			<p class="text-sm text-muted-foreground">Not shared yet</p>
		{/if}
	</Card>

	<Card as="section" class="{show('access')} content-start gap-4 p-4">
		<h2 class="text-sm font-medium">Client access</h2>
		<div class="grid gap-1.5">
			<Label for="expiry" class="text-sm">Open until</Label>
			<Input
				id="expiry"
				type="date"
				class="min-h-0 px-2 py-1 text-sm {ring('expiry')}"
				value={expiryDay}
				onchange={(event) => {
					expiryDay = event.currentTarget.value;
					setExpiry();
				}}
			/>
			<p class="text-xs text-muted-foreground">Leave empty for no expiry.</p>
		</div>
		<div class="grid gap-1.5">
			<Label for="included" class="text-sm">Included photos</Label>
			<Input
				id="included"
				type="number"
				min="1"
				step="1"
				placeholder="Unlimited"
				class="min-h-0 w-auto px-2 py-1 text-sm {ring('included')}"
				bind:value={included}
				oninput={() => schedule('included_photos', () => saveIncluded(false))}
				onblur={() => {
					cancel('included_photos');
					saveIncluded(true);
				}}
			/>
			<p class="text-xs text-muted-foreground">Leave empty for no limit.</p>
		</div>
	</Card>

	<Card as="section" class="{show('look')} content-start gap-4 p-4">
		<h2 class="text-sm font-medium">Look</h2>
		<div class="grid gap-1.5 {ring('layout') ? 'rounded-control ring-2 ring-ok' : ''}">
			<span class="text-sm">Layout</span>
			<IconButtonGroup value={layout} items={layouts} label="Layout" onchange={setLayout} />
		</div>
		<div class="flex items-center justify-between gap-2">
			<Label for="watermark" class="text-sm">Watermark proofs</Label>
			<Switch
				id="watermark"
				bind:checked={watermark}
				class={ring('watermark')}
				onchange={setWatermark}
			/>
		</div>
	</Card>

	<Card as="section" class="{show('gallery')} content-start gap-2 p-4">
		<h2 class="text-sm font-medium">Gallery</h2>
		<div class="flex flex-wrap gap-2">
			{#if archived}
				<Button variant="outline" size="sm" onclick={onrestore}>
					<ArchiveRestore size={16} aria-hidden="true" class="mr-1.5" />Restore
				</Button>
			{:else}
				<Button variant="outline" size="sm" onclick={onarchive}>
					<Archive size={16} aria-hidden="true" class="mr-1.5" />Archive
				</Button>
			{/if}
			<Button variant="destructive" size="sm" onclick={ondelete}>
				<Trash2 size={16} aria-hidden="true" class="mr-1.5" />Delete
			</Button>
		</div>
	</Card>
</aside>
