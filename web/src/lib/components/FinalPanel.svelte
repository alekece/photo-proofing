<script lang="ts" module>
	/** A delivered or draft final photo; the shape the Final endpoints (#70) will fill in. */
	export type FinalPhoto = {
		id: string;
		name: string;
		thumbnailUrl: string;
		/** Whether the client has downloaded it, once delivered. */
		downloaded?: boolean;
	};
</script>

<script lang="ts">
	import { CircleCheck, ImagePlus, Trash2, Upload } from '@lucide/svelte';
	import PhotoTile, { GRID_CLASSES } from '#lib/components/PhotoTile.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import EmptyState from '#lib/components/ui/empty-state/empty-state.svelte';
	import SelectPill from '#lib/components/ui/select-pill/select-pill.svelte';
	import { SelectMode } from '#lib/gallery/select-mode.svelte.js';

	let {
		photos = [],
		oncount,
		onupload,
		ondelete
	}: {
		/** The finished photos; empty until the Final endpoints (#70, #71) exist. */
		photos?: FinalPhoto[];
		/** How many finished photos there are, for the tab. */
		oncount?: (count: number | null) => void;
		/** Uploads the picked or dropped files; without it, uploading is shown but unavailable. */
		onupload?: (files: File[]) => void;
		/** Deletes the photos, after the caller has confirmed. */
		ondelete?: (photos: FinalPhoto[]) => void;
	} = $props();

	const mode = new SelectMode();
	const UNAVAILABLE = 'Finished photos arrive with a later release';

	let picker = $state<HTMLInputElement | null>(null);
	let dragging = $state(false);

	$effect(() => {
		oncount?.(photos.length);
	});

	function addFiles(files: FileList | null | undefined) {
		if (onupload && files && files.length > 0) onupload([...files]);
	}

	function ondrop(event: DragEvent) {
		event.preventDefault();
		dragging = false;
		addFiles(event.dataTransfer?.files);
	}

	function ondragover(event: DragEvent) {
		event.preventDefault();
		dragging = true;
	}

	function onkeydown(event: KeyboardEvent) {
		if (mode.active && event.key === 'Escape' && !event.defaultPrevented) mode.leave();
	}
</script>

<svelte:window {onkeydown} />

<section
	aria-label="Final"
	class={['grid gap-3 rounded-sm', dragging && 'outline-2 outline-offset-4 outline-foreground/40']}
	{ondragover}
	ondragleave={() => (dragging = false)}
	{ondrop}
>
	<div class="flex flex-wrap items-center justify-end gap-2">
		{#if mode.active}
			<SelectPill
				count={mode.count}
				actionLabel="Delete"
				destructive
				actionDisabled={!ondelete || mode.count === 0}
				onaction={() => ondelete?.(photos.filter(({ id }) => mode.has(id)))}
				onselectall={() => mode.selectAll(photos.map(({ id }) => id))}
				onclose={() => mode.leave()}
			>
				{#snippet icon()}<Trash2 size={16} aria-hidden="true" />{/snippet}
			</SelectPill>
		{:else}
			<Button
				variant="outline"
				size="sm"
				class="min-h-9! gap-1.5"
				disabled={photos.length === 0}
				onclick={() => mode.enter()}><CircleCheck size={16} aria-hidden="true" />Select</Button
			>
		{/if}
		<Button
			size="sm"
			class="min-h-9! gap-1.5"
			disabled={!onupload}
			title={onupload ? undefined : UNAVAILABLE}
			onclick={() => picker?.click()}><Upload size={16} aria-hidden="true" />Upload</Button
		>
		<input
			bind:this={picker}
			type="file"
			multiple
			accept="image/jpeg,.jpg,.jpeg"
			class="hidden"
			aria-label="Choose finished photos to upload"
			onchange={(event) => {
				addFiles(event.currentTarget.files);
				event.currentTarget.value = '';
			}}
		/>
	</div>

	{#if photos.length === 0}
		<EmptyState>
			<ImagePlus size={32} aria-hidden="true" />
			<h3>Drop the finished photos here</h3>
			<Button
				size="sm"
				disabled={!onupload}
				title={onupload ? undefined : UNAVAILABLE}
				onclick={() => picker?.click()}>Upload</Button
			>
		</EmptyState>
	{:else}
		<ul aria-label="Final photos" class={GRID_CLASSES.grid}>
			{#each photos as photo (photo.id)}
				<li>
					<PhotoTile
						name={photo.name}
						state="ready"
						thumbnailUrl={photo.thumbnailUrl}
						downloaded={photo.downloaded}
						selected={mode.has(photo.id)}
						onselect={mode.active ? () => mode.toggle(photo.id) : undefined}
						onremove={ondelete && (() => ondelete([photo]))}
					/>
				</li>
			{/each}
		</ul>
	{/if}
</section>
