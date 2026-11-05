<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';
	import Card from '#lib/components/ui/card/card.svelte';
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import Label from '#lib/components/ui/label/label.svelte';
	import Radio from '#lib/components/ui/radio/radio.svelte';
	import type { Gallery, GalleryChanges, GalleryLayout } from '#lib/gallery/api.js';
	import { endOfLocalDay, toLocalDate } from '#lib/gallery/format.js';
	import { goingToRetry } from '#lib/gallery/focus.js';

	/**
	 * `gallery` is the last saved state: draft values start from it once and are owned here
	 * afterwards. `failed` is bindable because a field reverting after a failed save clears its entry.
	 * `onsave` sends a patch; `schedule`/`cancel` are the page's keyed debouncer, so its flush on
	 * navigation also covers the debounced "included photos" save.
	 */
	let {
		gallery,
		failed = $bindable(),
		onsave,
		schedule,
		cancel
	}: {
		gallery: Gallery;
		failed: GalleryChanges;
		onsave: (changes: GalleryChanges) => void;
		schedule: (field: string, run: () => void) => void;
		cancel: (field: string) => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let expiryDay = $state(gallery.expires_at ? toLocalDate(gallery.expires_at) : '');
	// svelte-ignore state_referenced_locally
	let included = $state<number | null>(gallery.included_photos);
	// svelte-ignore state_referenced_locally
	let layout = $state<GalleryLayout>(gallery.layout);
	// svelte-ignore state_referenced_locally
	let watermark = $state(gallery.watermark_enabled);
	let includedInvalid = $state(false);

	let expired = $derived(
		expiryDay !== '' && new Date(endOfLocalDay(expiryDay)).getTime() < Date.now()
	);

	function setLayout(value: GalleryLayout) {
		layout = value;
		onsave({ layout });
	}
	function setExpiry() {
		onsave({ expires_at: expiryDay ? endOfLocalDay(expiryDay) : null });
	}
	function clearExpiry() {
		expiryDay = '';
		setExpiry();
	}
	function saveIncluded() {
		const count = included;
		includedInvalid = count !== null && (!Number.isInteger(count) || count < 1);
		if (includedInvalid || count === gallery.included_photos) return;
		onsave({ included_photos: count });
	}
	function editIncluded() {
		schedule('included_photos', saveIncluded);
	}
	function leaveIncluded(event: FocusEvent) {
		cancel('included_photos');
		if (failed.included_photos !== undefined && failed.included_photos === included) {
			if (goingToRetry(event)) return;
			delete failed.included_photos;
			included = gallery.included_photos;
			includedInvalid = false;
		} else saveIncluded();
	}
</script>

<Card as="aside" class="grid content-start gap-5 p-4" aria-label="Gallery settings">
	<h2 class="text-lg">Settings</h2>

	<fieldset class="grid gap-2">
		<legend class="mb-1 text-sm font-medium">Layout</legend>
		<div class="flex gap-4 text-sm">
			<Label class="flex items-center gap-2 font-normal">
				<Radio
					name="layout"
					value="grid"
					checked={layout === 'grid'}
					onchange={() => setLayout('grid')}
				/>
				Grid
			</Label>
			<Label class="flex items-center gap-2 font-normal">
				<Radio
					name="layout"
					value="masonry"
					checked={layout === 'masonry'}
					onchange={() => setLayout('masonry')}
				/>
				Masonry
			</Label>
		</div>
	</fieldset>

	<p class="text-sm text-muted-foreground">
		Star a photo in the Photos tab to make it the hero photo.
	</p>

	<div class="grid gap-2">
		<Label for="expiry" class="text-sm">Expiry</Label>
		<div class="flex gap-2">
			<Input
				id="expiry"
				type="date"
				class="min-h-0 flex-1 px-2 py-1 text-sm"
				value={expiryDay}
				onchange={(event) => {
					expiryDay = event.currentTarget.value;
					setExpiry();
				}}
			/>
			<Button variant="outline" size="sm" onclick={clearExpiry} disabled={expiryDay === ''}>
				Clear
			</Button>
		</div>
		<p class="text-xs text-muted-foreground">
			{expiryDay ? 'Ends at the end of this day.' : 'No expiry.'}
		</p>
		{#if expired}
			<p class="text-xs text-warning">
				This date is in the past, so the gallery is already expired.
			</p>
		{/if}
	</div>

	<div class="grid gap-2">
		<Label for="included" class="text-sm">Included photos</Label>
		<Input
			id="included"
			type="number"
			min="1"
			step="1"
			placeholder="Unlimited"
			class="min-h-0 w-auto px-2 py-1 text-sm"
			bind:value={included}
			oninput={editIncluded}
			onblur={leaveIncluded}
			aria-invalid={includedInvalid}
			aria-describedby="included-help"
		/>
		<p
			id="included-help"
			class="text-xs {includedInvalid ? 'text-danger' : 'text-muted-foreground'}"
		>
			{includedInvalid ? 'Enter a whole number of at least 1.' : 'Leave empty for unlimited.'}
		</p>
	</div>

	<Label class="flex items-center gap-2 border-t border-border pt-4 text-sm">
		<Checkbox bind:checked={watermark} onchange={() => onsave({ watermark_enabled: watermark })} />
		Watermark
	</Label>
</Card>
