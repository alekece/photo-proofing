<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ArchiveRestore, Send } from '@lucide/svelte';
	import JourneySteps, { type Step } from '#lib/components/JourneySteps.svelte';
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
		sharing = false,
		onupload,
		onshare,
		oncollect,
		ondelivery,
		onrestore
	}: {
		gallery: Gallery;
		photoCount?: number | null;
		favoriteCount?: number | null;
		sharing?: boolean;
		onupload: () => void;
		onshare: () => void;
		oncollect?: () => void;
		ondelivery: () => void;
		onrestore: () => void;
	} = $props();

	const status = $derived(galleryStatus(gallery.states));
	const shared = $derived(gallery.share_token !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');
	const photos = $derived(photoCount ?? 0);
	const favorites = $derived(favoriteCount ?? 0);
	const steps = $derived<Step[]>([
		{
			id: 'upload',
			label: m.action_upload(),
			onactivate: onupload,
			done: photos > 0,
			fact: photos > 0 ? m.summary_photos({ count: photos }) : m.step_add_proofs()
		},
		{
			id: 'share',
			label: m.action_share(),
			onactivate: onshare,
			done: shared,
			fact: shared ? undefined : m.step_copy_link()
		},
		{
			id: 'collect',
			label: m.step_collect(),
			onactivate: shared ? oncollect : undefined,
			done: delivered,
			fact: shared ? m.notification_selected({ count: favorites }) : m.step_client_picks(),
			accent: gallery.included_photos !== null && favorites > gallery.included_photos
		},
		{
			id: 'publish',
			label: m.action_deliver(),
			onactivate: shared ? ondelivery : undefined,
			done: delivered,
			fact: delivered ? undefined : m.step_upload_finals()
		}
	]);
</script>

<div
	data-slot="gallery-action-bar"
	class="@container border-b border-border bg-panel px-3.5 py-2.5 sm:px-7 sm:py-3"
>
	<div class="relative grid justify-items-center gap-1">
		{#if !archived}
			<JourneySteps {steps} />
		{/if}

		{#if archived}
			<Button size="sm" class="min-h-9! gap-2 justify-self-end" onclick={onrestore}>
				<ArchiveRestore size={16} aria-hidden="true" />{m.action_restore()}
			</Button>
		{:else if !shared && !sharing}
			<Button
				data-share-trigger
				size="sm"
				class="min-h-9! gap-2 justify-self-end @min-[56rem]:absolute @min-[56rem]:right-0 @min-[56rem]:self-center"
				onclick={onshare}
			>
				<Send size={16} aria-hidden="true" />{m.action_share()}
			</Button>
		{/if}
	</div>
</div>
