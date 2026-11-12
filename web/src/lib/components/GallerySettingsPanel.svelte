<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Archive, ArchiveRestore, Trash2 } from '@lucide/svelte';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import DatePicker from '#lib/components/ui/DatePicker/DatePicker.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import Label from '#lib/components/ui/Label/Label.svelte';
	import Switch from '#lib/components/ui/Switch/Switch.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import type { Gallery, GalleryChanges } from '#lib/gallery/api.js';
	import { endOfLocalDay, toLocalDate } from '#lib/gallery/format.js';

	/**
	 * Studio `Settings` view, split into three Warm Paper cards:
	 * 1. General (`Shoot date`, `Included photos`, `Open until`)
	 * 2. Privacy & Protection (`Client link`, `Enable watermarks`)
	 * 3. Actions (`Archive` / `Restore`, `Delete`)
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

	const savedExpiry = $derived(gallery.expires_at ? toLocalDate(gallery.expires_at) : '');
	const savedShootDate = $derived(gallery.shoot_date);
	const savedIncluded = $derived(gallery.included_photos);
	const savedWatermark = $derived(gallery.watermark_enabled);

	let expiryDay = $derived(savedExpiry);
	let shootDate = $derived(savedShootDate);
	let watermark = $derived(savedWatermark);
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(savedIncluded);
	let includedDirty = false;
	// svelte-ignore state_referenced_locally
	let lastSavedIncluded = savedIncluded;

	$effect.pre(() => {
		if (savedIncluded !== lastSavedIncluded) {
			lastSavedIncluded = savedIncluded;
			if (!includedDirty) included = savedIncluded;
		}
	});

	const archived = $derived(gallery.states.includes('archived'));
	const delivered = $derived(gallery.states.includes('delivered'));

	const card = 'rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5';
	const heading =
		'mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase';
	const row =
		'flex flex-wrap items-center justify-between gap-2.5 border-t border-[#ece3d6] py-3 text-sm last:pb-0 [h3+&]:border-t-0 [h3+&]:pt-0.5';

	async function commit(changes: GalleryChanges, revert: () => void) {
		if (!(await onsave(changes))) revert();
	}

	function setShootDate(day: string | null) {
		const previous = shootDate;

		shootDate = day;
		void commit({ shoot_date: day }, () => (shootDate = previous));
	}

	function setExpiry(day: string | null) {
		const next = day ?? '';

		expiryDay = next;
		void commit(
			{ expires_at: next ? endOfLocalDay(next) : null },
			() => (expiryDay = gallery.expires_at ? toLocalDate(gallery.expires_at) : '')
		);
	}

	/** While typing, a half-typed value is left alone; leaving the field is where it is judged. */
	function saveIncluded(leaving: boolean) {
		const count = included;
		const revert = () => {
			if (leaving || included === count) {
				includedDirty = false;
				included = gallery.included_photos;
			}
		};

		if (count !== null && (!Number.isInteger(count) || count < 1)) {
			if (leaving) {
				toasts.error(m.settings_included_invalid());
				revert();
			}

			return;
		}
		if (count === gallery.included_photos) {
			if (leaving) includedDirty = false;

			return;
		}

		void (async () => {
			if (await onsave({ included_photos: count })) {
				if (included === count) includedDirty = false;
			} else {
				revert();
			}
		})();
	}

	function setWatermark(value: boolean) {
		void commit({ watermark_enabled: value }, () => (watermark = !value));
	}
</script>

<div class="grid max-w-2xl gap-4">
	<h2 class="text-2xl leading-tight">{m.tab_settings()}</h2>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_general()}</h3>
		<div class={row}>
			<Label for="shoot-date" class="shrink-0 text-foreground">{m.shoot_date_label()}</Label>
			<DatePicker
				id="shoot-date"
				placeholder="–"
				rootClass="w-44 min-w-0"
				class="min-h-0 w-full rounded-lg bg-white px-2.5 py-1.5 text-sm font-semibold [&>span]:flex-1 [&>span]:text-center"
				value={shootDate}
				clearable
				onchange={setShootDate}
			/>
		</div>
		<div class={row}>
			<Label for="included" class="text-foreground">{m.settings_included()}</Label>
			<Input
				id="included"
				placeholder="–"
				type="number"
				min="1"
				step="1"
				class="min-h-0 w-[4.75rem] [appearance:textfield] rounded-lg bg-white px-2 py-1.5 text-center text-sm font-semibold placeholder:text-muted-foreground/40 [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
				bind:value={included}
				disabled={delivered}
				oninput={() => {
					includedDirty = true;
					schedule('included_photos', () => saveIncluded(false));
				}}
				onblur={() => {
					cancel('included_photos');
					saveIncluded(true);
				}}
			/>
		</div>
		<div class={row}>
			<Label for="expiry" class="shrink-0 text-foreground">{m.expiry_open_until()}</Label>
			<DatePicker
				id="expiry"
				placeholder="–"
				rootClass="w-44 min-w-0"
				class="min-h-0 w-full rounded-lg bg-white px-2.5 py-1.5 text-sm font-semibold [&>span]:flex-1 [&>span]:text-center"
				value={expiryDay || null}
				clearable
				onchange={setExpiry}
			/>
		</div>
	</Card>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_privacy_protection()}</h3>
		<div class={row}>
			<span class="text-foreground">{m.copy_link_label()}</span>
			{#if shareToken}
				<div class="w-full sm:w-auto sm:min-w-64">
					<ShareLink token={shareToken} {origin} />
				</div>
			{:else}
				<span class="text-sm text-muted-foreground">{m.settings_not_shared()}</span>
			{/if}
		</div>
		<div class={row}>
			<Label for="watermark" class="text-foreground">{m.settings_watermark()}</Label>
			<Switch id="watermark" bind:checked={watermark} onchange={setWatermark} />
		</div>
	</Card>

	<Card as="section" class={card}>
		<h3 class={heading}>{m.settings_actions()}</h3>
		<div class={row}>
			<div class="grid gap-0.5">
				<span class="font-medium text-foreground"
					>{archived ? m.action_restore() : m.action_archive()}</span
				>
				<span class="text-xs text-muted-foreground"
					>{archived ? m.settings_restore_hint() : m.settings_archive_hint()}</span
				>
			</div>
			{#if archived}
				<Button variant="outline" size="sm" class="min-h-9! shrink-0 bg-white!" onclick={onrestore}>
					<ArchiveRestore size={16} aria-hidden="true" class="mr-1.5" />{m.action_restore()}
				</Button>
			{:else}
				<Button variant="outline" size="sm" class="min-h-9! shrink-0 bg-white!" onclick={onarchive}>
					<Archive size={16} aria-hidden="true" class="mr-1.5" />{m.action_archive()}
				</Button>
			{/if}
		</div>
		<div class={row}>
			<div class="grid gap-0.5">
				<span class="font-medium text-foreground">{m.action_delete()}</span>
				<span class="text-xs text-muted-foreground">{m.settings_delete_hint()}</span>
			</div>
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! shrink-0 border-danger/35! bg-white! text-danger!"
				onclick={ondelete}
			>
				<Trash2 size={16} aria-hidden="true" class="mr-1.5" />{m.action_delete()}
			</Button>
		</div>
	</Card>
</div>
