<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { ArchiveRestore } from '@lucide/svelte';
	import JourneySteps, { type Step } from '#lib/components/JourneySteps.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import { formatLongDate, galleryStatus } from '#lib/gallery/format.js';

	/**
	 * Top studio bar over the gallery workspace: an actionable five-step Journey
	 * (`Upload → Share → Collect → Import finished → Deliver`, hidden when archived).
	 */
	let {
		gallery,
		photoCount = null,
		favoriteCount = null,
		finalCount = null,
		favoritesCollected = false,
		onupload,
		onshare,
		oncollect,
		onretouch,
		ondelivery,
		onrestore
	}: {
		gallery: Gallery;
		photoCount?: number | null;
		favoriteCount?: number | null;
		finalCount?: number | null;
		favoritesCollected?: boolean;
		onupload: () => void;
		onshare: () => void;
		oncollect?: () => void;
		onretouch?: () => void;
		ondelivery: () => void;
		onrestore?: () => void;
	} = $props();

	const status = $derived(galleryStatus(gallery.states));
	const shared = $derived(gallery.share_token !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');
	const photos = $derived(photoCount ?? 0);
	const favorites = $derived(favoriteCount ?? 0);
	const finals = $derived(finalCount ?? 0);
	const steps = $derived<Step[]>([
		{
			id: 'upload',
			label: m.step_upload(),
			onactivate: onupload,
			done: photos > 0 || shared,
			fact: photos > 0 ? m.summary_photos({ count: photos }) : undefined
		},
		{
			id: 'share',
			label: m.step_share(),
			onactivate: onshare,
			done: shared,
			fact: gallery.shared_at
				? m.step_shared_on({ date: formatLongDate(gallery.shared_at) })
				: undefined
		},
		{
			id: 'collect',
			label: m.step_collect(),
			onactivate: shared ? oncollect : undefined,
			done: shared && (favoritesCollected || finals > 0 || delivered),
			fact: shared ? m.notification_selected({ count: favorites }) : undefined,
			accent: gallery.included_photos !== null && favorites > gallery.included_photos
		},
		{
			id: 'retouch',
			label: m.step_upload_finals(),
			onactivate: shared ? onretouch : undefined,
			done: shared && (finals > 0 || delivered),
			fact: finals > 0 ? m.summary_photos({ count: finals }) : undefined
		},
		{
			id: 'deliver',
			label: m.step_deliver(),
			onactivate: shared && !delivered ? ondelivery : undefined,
			done: delivered,
			fact: gallery.delivered_at
				? m.step_delivered_on({ date: formatLongDate(gallery.delivered_at) })
				: undefined
		}
	]);
</script>

<div
	data-slot="gallery-action-bar"
	class="@container border-b border-border bg-panel workspace-inset py-2.5 sm:py-3"
>
	<div class="flex flex-wrap items-center justify-between gap-2">
		{#if !archived}
			<JourneySteps {steps} />
		{/if}

		{#if archived && onrestore}
			<div class="ml-auto flex shrink-0 items-center gap-2">
				<Button size="sm" variant="outline" onclick={onrestore}>
					<ArchiveRestore size={16} aria-hidden="true" />{m.action_restore()}
				</Button>
			</div>
		{/if}
	</div>
</div>
