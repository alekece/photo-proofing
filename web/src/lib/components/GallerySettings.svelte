<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Archive, ArchiveRestore, Calendar, Link, Trash2 } from '@lucide/svelte';
	import type { Component } from 'svelte';
	import LayoutRowsIcon from '#lib/components/icons/LayoutRowsIcon.svelte';
	import LayoutStaggeredIcon from '#lib/components/icons/LayoutStaggeredIcon.svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import DatePicker from '#lib/components/ui/DatePicker/DatePicker.svelte';
	import IconButtonGroup from '#lib/components/ui/IconButtonGroup/IconButtonGroup.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import Switch from '#lib/components/ui/Switch/Switch.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
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
	let popped = $state<string | null>(null);

	const archived = $derived(gallery.states.includes('archived'));
	const delivered = $derived(gallery.states.includes('delivered'));
	const cards: { id: string; label: string; icon: Component<{ size?: number }> }[] = [
		{ id: 'link', label: m.copy_link_label(), icon: Link },
		{ id: 'access', label: m.settings_access(), icon: Calendar },
		{ id: 'look', label: m.settings_look(), icon: LayoutStaggeredIcon },
		{ id: 'gallery', label: m.settings_gallery(), icon: Archive }
	];
	const layouts = [
		{ id: 'masonry', label: m.settings_layout_masonry(), icon: LayoutStaggeredIcon },
		{ id: 'grid', label: m.settings_layout_grid(), icon: LayoutRowsIcon }
	];
	const card = 'rounded-[0.875rem] bg-white p-4';
	// Rows are ruled apart; the first sits right under the card's heading.
	const row =
		'flex items-center justify-between gap-2.5 border-t border-[#efe7dc] py-2.5 text-sm last:pb-0 [h2+&]:border-t-0 [h2+&]:pt-0.5';
	// Phones show only the popped card; wide screens show all of them.
	const show = (id: string) =>
		popped === id
			? 'absolute left-[3.875rem] z-20 grid w-[min(18.75rem,calc(100vw-5rem))] shadow-lg md:static md:w-auto md:shadow-none'
			: 'hidden md:grid';

	async function commit(changes: GalleryChanges, revert: () => void) {
		if (!(await onsave(changes))) revert();
	}

	function setLayout(value: string) {
		const previous = layout;

		layout = value as GalleryLayout;
		void commit({ layout }, () => (layout = previous));
	}
	function setExpiry() {
		void commit(
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
				toasts.error(m.settings_included_invalid());
				revert();
			}

			return;
		}
		if (count === gallery.included_photos) return;

		void commit({ included_photos: count }, revert);
	}
	function setWatermark(value: boolean) {
		void commit({ watermark_enabled: value }, () => (watermark = !value));
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') popped = null;
	}}
/>
<aside
	aria-label={m.settings_panel()}
	class="relative grid content-start gap-3.5 border-r border-border bg-panel py-3 max-md:place-items-center md:px-5 md:pt-[1.375rem] md:pb-7"
>
	<div class="grid gap-2 md:hidden" role="group" aria-label={m.settings_sections()}>
		{#each cards as { id, label, icon: Icon } (id)}
			<button
				type="button"
				aria-label={m.settings_show({ label })}
				aria-expanded={popped === id}
				title={label}
				class="grid size-10 place-items-center rounded-xl border {popped === id
					? 'border-foreground bg-foreground text-inverse'
					: 'border-transparent text-muted-foreground'}"
				onclick={() => (popped = popped === id ? null : id)}
			>
				<Icon size={18} />
			</button>
		{/each}
	</div>

	<Card as="section" class="{show('link')} content-start {card} max-md:top-3">
		<h2
			class="mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
		>
			{m.copy_link_label()}
		</h2>
		{#if shareToken}
			<ShareLink token={shareToken} {origin} />
		{:else}
			<p class="mt-1.5 text-sm text-muted-foreground">{m.settings_not_shared()}</p>
		{/if}
	</Card>

	<Card as="section" class="{show('access')} content-start {card} max-md:top-14">
		<h2
			class="mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
		>
			{m.settings_access()}
		</h2>
		<div class={row}>
			<Label for="expiry" class="text-muted-foreground">{m.expiry_open_until()}</Label>
			<DatePicker
				id="expiry"
				class="min-h-0 w-auto rounded-lg px-2 py-[0.3125rem] text-sm font-semibold"
				value={expiryDay || null}
				placeholder="∞"
				clearable
				onchange={(day) => {
					expiryDay = day ?? '';
					setExpiry();
				}}
			/>
		</div>
		<div class={row}>
			<Label for="included" class="text-muted-foreground">{m.settings_included()}</Label>
			<Input
				id="included"
				type="number"
				min="1"
				step="1"
				placeholder="∞"
				class="min-h-0 w-[4.5rem] rounded-lg px-2 py-[0.3125rem] text-right text-sm font-semibold"
				bind:value={included}
				disabled={delivered}
				oninput={() => schedule('included_photos', () => saveIncluded(false))}
				onblur={() => {
					cancel('included_photos');
					saveIncluded(true);
				}}
			/>
		</div>
	</Card>

	<Card as="section" class="{show('look')} content-start {card} max-md:top-24">
		<h2
			class="mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
		>
			{m.settings_look()}
		</h2>
		<div class={row}>
			<span class="text-muted-foreground">{m.settings_layout()}</span>
			<IconButtonGroup
				value={layout}
				items={layouts}
				label={m.settings_layout()}
				onchange={setLayout}
			/>
		</div>
		<div class={row}>
			<Label for="watermark" class="text-muted-foreground">{m.settings_watermark()}</Label>
			<Switch id="watermark" bind:checked={watermark} onchange={setWatermark} />
		</div>
	</Card>

	<Card as="section" class="{show('gallery')} content-start {card} max-md:top-36">
		<h2
			class="mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
		>
			{m.settings_gallery()}
		</h2>
		<div class="flex gap-2 [&>*]:min-h-9! [&>*]:flex-1">
			{#if archived}
				<Button variant="outline" size="sm" onclick={onrestore}>
					<ArchiveRestore size={16} aria-hidden="true" class="mr-1.5" />{m.action_restore()}
				</Button>
			{:else}
				<Button variant="outline" size="sm" onclick={onarchive}>
					<Archive size={16} aria-hidden="true" class="mr-1.5" />{m.action_archive()}
				</Button>
			{/if}
			<Button variant="outline" size="sm" class="border-danger/35! text-danger!" onclick={ondelete}>
				<Trash2 size={16} aria-hidden="true" class="mr-1.5" />{m.action_delete()}
			</Button>
		</div>
	</Card>
</aside>
