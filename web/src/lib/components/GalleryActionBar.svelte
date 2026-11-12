<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ArchiveRestore, Eye, Send } from '@lucide/svelte';
	import JourneySteps from '#lib/components/JourneySteps.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import { galleryStatus } from '#lib/gallery/format.js';

	/**
	 * Top studio bar over the gallery workspace: the 4-step actionable Journey Wizard
	 * (`Upload → Share → Collect → Publish`, hidden when archived) and the stage actions.
	 */
	let {
		gallery,
		photoCount = null,
		favoriteCount = null,
		onpreview,
		onshare,
		onpublish,
		onrestore
	}: {
		gallery: Gallery;
		photoCount?: number | null;
		favoriteCount?: number | null;
		onpreview: () => void;
		onshare: () => void;
		onpublish: () => void;
		onrestore: () => void;
	} = $props();

	const status = $derived(galleryStatus(gallery.states));
	const shared = $derived(gallery.share_token !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');
	const photos = $derived(photoCount ?? 0);
	const favorites = $derived(favoriteCount ?? 0);
	const steps = $derived([
		{
			id: 'upload',
			label: m.action_upload(),
			done: photos > 0,
			fact: photos > 0 ? m.summary_photos({ count: photos }) : m.step_add_proofs()
		},
		{
			id: 'share',
			label: m.action_share(),
			done: shared,
			fact: shared ? undefined : m.step_copy_link()
		},
		{
			id: 'collect',
			label: m.step_collect(),
			done: delivered,
			fact: shared ? m.notification_selected({ count: favorites }) : m.step_client_picks(),
			accent: gallery.included_photos !== null && favorites > gallery.included_photos
		},
		{
			id: 'publish',
			label: m.action_deliver(),
			done: delivered,
			fact: delivered ? undefined : m.step_upload_finals()
		}
	]);
</script>

<div
	data-slot="gallery-action-bar"
	class="@container flex flex-wrap items-center justify-between gap-3 border-b border-border bg-panel px-3.5 py-2.5 sm:gap-4 sm:px-7 sm:py-3"
>
	{#if !archived}
		<JourneySteps {steps} />
	{:else}
		<span class="flex-1"></span>
	{/if}

	<div class="ml-auto flex shrink-0 items-center gap-2">
		<Button variant="outline" size="sm" class="min-h-9! gap-2 px-3!" onclick={onpreview}>
			<Eye size={16} aria-hidden="true" />
			<span class="max-sm:sr-only">{m.action_preview()}</span>
		</Button>
		{#if archived}
			<Button size="sm" class="min-h-9! gap-2" onclick={onrestore}>
				<ArchiveRestore size={16} aria-hidden="true" />{m.action_restore()}
			</Button>
		{:else if !shared}
			<Button size="sm" class="min-h-9! gap-2" onclick={onshare}>
				<Send size={16} aria-hidden="true" />{m.action_share()}
			</Button>
		{:else if !delivered}
			<Button size="sm" class="min-h-9! gap-2" onclick={onpublish}>
				<Send size={16} aria-hidden="true" />{m.action_deliver()}
			</Button>
		{/if}
	</div>
</div>
