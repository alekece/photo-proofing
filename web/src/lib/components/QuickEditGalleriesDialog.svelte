<script module lang="ts">
	import type { GalleryChanges } from '#lib/gallery/api.js';

	export interface QuickGalleryEdit {
		id: string;
		changes: GalleryChanges;
	}
</script>

<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { m } from '#lib/paraglide/messages.js';
	import ActionButton from '#lib/components/ui/ActionButton/ActionButton.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';
	import { TITLE_MAX } from '#lib/gallery/format.js';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import DatePicker from '#lib/components/ui/DatePicker/DatePicker.svelte';
	import Dialog from '#lib/components/ui/Dialog/Dialog.svelte';
	import Input from '#lib/components/ui/Input/Input.svelte';
	import type { GalleryOverview } from '#lib/gallery/api.js';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { DEFAULT_FOCAL_POINT } from '#lib/gallery/schemas.js';

	let {
		galleries,
		onsave,
		onclose
	}: {
		galleries: GalleryOverview[];
		/** Saves modified rows and returns any error message (`null` or empty when all succeeded). */
		onsave: (edits: QuickGalleryEdit[]) => Promise<string | null | void>;
		onclose: () => void;
	} = $props();

	const id = $props.id();

	type DraftRow = {
		id: string;
		heroId: string | null;
		focalX: number;
		focalY: number;
		initialTitle: string;
		title: string;
		initialShootDate: string | null;
		shootDate: string | null;
	};

	function toDraftRow(gallery: GalleryOverview): DraftRow {
		const focal = gallery.focal_point ?? DEFAULT_FOCAL_POINT;
		const initialTitle = gallery.title ?? '';

		return {
			id: gallery.id,
			heroId: gallery.hero_id,
			focalX: focal.x,
			focalY: focal.y,
			initialTitle,
			title: initialTitle,
			initialShootDate: gallery.shoot_date,
			shootDate: gallery.shoot_date
		};
	}

	// svelte-ignore state_referenced_locally
	let rows = $state<DraftRow[]>(galleries.map(toDraftRow));

	// When a batch save partially fails, the dashboard prunes succeeded IDs from `galleries`;
	// keep only remaining galleries while preserving any unsaved draft edits on them.
	$effect(() => {
		const incoming = galleries;

		untrack(() => {
			const reconciled: DraftRow[] = [];

			for (const item of incoming) {
				const current = rows.find((row) => row.id === item.id);

				if (!current) {
					reconciled.push(toDraftRow(item));
					continue;
				}

				const serverTitle = item.title ?? '';
				const serverDate = item.shoot_date;
				const titleEdited = current.title !== current.initialTitle;
				const dateEdited = current.shootDate !== current.initialShootDate;

				reconciled.push({
					...current,
					initialTitle: serverTitle,
					title: titleEdited ? current.title : serverTitle,
					initialShootDate: serverDate,
					shootDate: dateEdited ? current.shootDate : serverDate
				});
			}

			if (
				reconciled.length !== rows.length ||
				reconciled.some(
					(row, index) =>
						row.id !== rows[index]?.id ||
						row.initialTitle !== rows[index]?.initialTitle ||
						row.initialShootDate !== rows[index]?.initialShootDate
				)
			) {
				rows = reconciled;
			}
		});
	});

	let firstInput = $state<HTMLInputElement | null>(null);
	let saving = $state(false);
	let error = $state('');

	onMount(() => firstInput?.focus());

	function buildEdits(): QuickGalleryEdit[] {
		const edits: QuickGalleryEdit[] = [];

		for (const row of rows) {
			const nextTitle = row.title.trim();
			const prevTitle = row.initialTitle.trim();
			const changes: GalleryChanges = {};

			if (nextTitle !== prevTitle) {
				changes.title = nextTitle === '' ? null : nextTitle;
			}

			if (row.shootDate !== row.initialShootDate) {
				changes.shoot_date = row.shootDate;
			}

			if (changes.title !== undefined || changes.shoot_date !== undefined) {
				edits.push({ id: row.id, changes });
			}
		}

		return edits;
	}

	let saveButton: ActionButton;
	const edits = $derived(buildEdits());
	const invalid = $derived(rows.some((row) => [...row.title.trim()].length > TITLE_MAX));

	async function submit(close: () => void) {
		if (saving || invalid || !edits.length) return false;
		saving = true;
		error = '';

		try {
			const failure = await onsave(edits);

			if (failure) {
				error = failure;

				return false;
			}

			close();
			return true;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : m.api_error_generic();
			return false;
		} finally {
			saving = false;
		}
	}
</script>

<Dialog
	{onclose}
	dismissible={!saving}
	labelledby="{id}-title"
	class="mx-auto mt-6 mb-auto {rows.length === 1
		? 'w-[min(30.625rem,calc(100vw-1.5rem))]'
		: 'w-[min(38rem,calc(100vw-1.5rem))]'} max-h-[calc(100dvh-2rem)] overflow-hidden rounded-dialog border border-border bg-background text-foreground shadow-xl sm:my-auto"
>
	{#snippet header()}
		<div class="grid gap-1 border-b border-border pb-3">
			<h2 id="{id}-title" class="pr-10 text-2xl leading-tight sm:text-3xl">
				{m.quick_edit_title()}
			</h2>
			{#if rows.length > 1}<p class="text-sm text-muted-foreground">
					{m.quick_edit_subtitle({ count: rows.length })}
				</p>{/if}
		</div>
	{/snippet}

	{#snippet content()}
		<form
			id="{id}-form"
			onsubmit={(event) => {
				event.preventDefault();
				saveButton.activate();
			}}
			class="grid min-h-0 gap-4"
			aria-busy={saving}
		>
			{#if error}
				<Toaster studio localError={{ message: error, dismiss: () => (error = '') }} />
			{/if}

			<div data-slot="quick-edit-rows" class="grid min-h-0 gap-2.5 overflow-y-auto pr-1">
				{#each rows as row, index (row.id)}
					<div
						class="grid items-end gap-2.5 {rows.length === 1
							? 'sm:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]'
							: 'grid-cols-[2.75rem_1fr] rounded-control border border-border p-2.5 sm:grid-cols-[3rem_1fr_10.5rem]'}"
					>
						{#if rows.length > 1}<div
								class="relative aspect-[4/3] w-full overflow-hidden rounded-[0.4375rem] bg-dark"
							>
								{#if row.heroId}
									<img
										src={previewUrl(row.id, row.heroId)}
										alt=""
										style:object-position="{row.focalX}% {row.focalY}%"
										class="size-full object-cover"
									/>
								{:else}
									<CoverBackdrop title={row.title.trim() || null} compact />
								{/if}
							</div>{/if}

						<div class="grid min-w-0 gap-1">
							{#if rows.length === 1}<label for="{id}-name-{index}" class="text-sm font-medium">
									{m.new_gallery_field()}
								</label>{/if}
							{#if index === 0}
								<Input
									bind:ref={firstInput}
									id="{id}-name-{index}"
									aria-label={m.new_gallery_field()}
									placeholder={m.gallery_untitled()}
									bind:value={row.title}
									disabled={saving}
									class="min-h-9 w-full bg-white px-2.5 py-1.5 text-sm"
								/>
							{:else}
								<Input
									id="{id}-name-{index}"
									aria-label={m.new_gallery_field()}
									placeholder={m.gallery_untitled()}
									bind:value={row.title}
									disabled={saving}
									class="min-h-9 w-full bg-white px-2.5 py-1.5 text-sm"
								/>
							{/if}
						</div>
						<div class={rows.length === 1 ? 'grid min-w-0 gap-1' : 'col-span-2 sm:col-span-1'}>
							<label
								for="{id}-date-{index}"
								class={rows.length === 1 ? 'text-sm font-medium' : 'sr-only'}
							>
								{m.shoot_date_label()}
							</label>
							<DatePicker
								disabled={saving}
								id="{id}-date-{index}"
								placeholder={m.shoot_date_label()}
								align="end"
								clearable
								value={row.shootDate}
								onchange={(picked) => (row.shootDate = picked)}
								rootClass="w-full min-w-0"
								class="min-h-9 w-full bg-white px-2.5 py-1.5 text-sm"
							/>
						</div>
					</div>
				{/each}
			</div>
		</form>
	{/snippet}

	{#snippet actions(dismiss, close)}
		<div class="flex flex-wrap justify-end gap-2 border-t border-border pt-3">
			<Button variant="outline" type="button" disabled={saving} onclick={dismiss}>
				{m.common_cancel()}
			</Button>
			<ActionButton
				bind:this={saveButton}
				form="{id}-form"
				pendingDelayMs={150}
				disabled={saving || invalid || !edits.length}
				labels={{ idle: m.quick_edit_save(), pending: m.quick_edit_saving() }}
				onaction={() => submit(close)}
			/>
		</div>
		{#if invalid}<p role="alert" class="text-sm text-danger">
				{m.new_gallery_title_too_long({ max: TITLE_MAX })}
			</p>{/if}
	{/snippet}
</Dialog>
