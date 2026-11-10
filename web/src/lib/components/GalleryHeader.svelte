<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { Snippet } from 'svelte';
	import { Eye, ArchiveRestore, Send } from '@lucide/svelte';
	import CoverBackdrop from '#lib/components/CoverBackdrop.svelte';
	import JourneySteps from '#lib/components/JourneySteps.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import StatusChip from '#lib/components/ui/StatusChip/StatusChip.svelte';
	import { previewUrl } from '#lib/gallery/photos.js';
	import { formatDateTime, formatLongDate, galleryStatus } from '#lib/gallery/format.js';
	import type { Gallery } from '#lib/gallery/api.js';

	/**
	 * The banner over a gallery: hero photo (or a warm stand-in), status, the editable `heading`
	 * field, a details card, the next main action and the journey steps. Photo and favorite counts
	 * come from the tabs; they stay out of the text until known.
	 */
	let {
		gallery,
		photoCount = null,
		favoriteCount = null,
		heroRevision,
		heading,
		onpreview,
		onshare,
		ondeliver,
		onrestore
	}: {
		gallery: Gallery;
		photoCount?: number | null;
		favoriteCount?: number | null;
		heroRevision?: number;
		heading: Snippet;
		onpreview: () => void;
		onshare: () => void;
		ondeliver: () => void;
		onrestore: () => void;
	} = $props();

	const status = $derived(galleryStatus(gallery.states));
	const shared = $derived(gallery.share_token !== null);
	const delivered = $derived(gallery.states.includes('delivered'));
	const archived = $derived(status === 'archived');
	// A shoot date is a calendar day: read at local noon so no time zone moves it.
	const day = $derived(
		gallery.shoot_date
			? formatLongDate(`${gallery.shoot_date}T12:00`)
			: formatLongDate(gallery.created_at)
	);
	const details = $derived(
		[
			{ term: m.common_photos(), value: photoCount === null ? '–' : String(photoCount) },
			{ term: gallery.shoot_date ? m.details_shot() : m.details_created(), value: day },
			shared && {
				term: m.details_last_opened(),
				value:
					gallery.last_opened_at === null
						? m.details_not_opened()
						: formatDateTime(gallery.last_opened_at)
			}
		].filter((detail) => detail !== false)
	);
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
			id: 'deliver',
			label: m.action_deliver(),
			done: delivered,
			fact: delivered ? undefined : m.step_upload_finals()
		}
	]);
</script>

<div
	data-slot="gallery-header"
	class="relative isolate flex h-[20.625rem] flex-col justify-end gap-0 overflow-hidden bg-dark px-[1.125rem] pb-[4.625rem] text-panel sm:h-[25rem] sm:flex-row sm:items-end sm:gap-4 sm:px-8 sm:pt-[1.625rem] sm:pb-[6.125rem]"
>
	{#if gallery.hero_id}
		<img
			src={previewUrl(gallery.id, gallery.hero_id, 'photos', heroRevision)}
			alt=""
			class="absolute inset-0 -z-20 size-full animate-hero-in object-cover {archived
				? 'brightness-[0.85] grayscale-[0.7]'
				: ''}"
		/>
	{:else}
		<CoverBackdrop title={gallery.title} edge={day} />
	{/if}
	<div
		aria-hidden="true"
		class="absolute inset-0 -z-10 bg-linear-to-b from-[rgb(43_36_32/0.45)] via-[rgb(43_36_32/0)] via-35% to-[rgb(43_36_32/0.78)]"
	></div>

	<div class="grid min-w-0 flex-none gap-0 sm:flex-1">
		<StatusChip {status} dot cover class="w-fit" />
		{@render heading()}
		<dl
			class="mt-2 mb-3 flex w-fit gap-5 rounded-lg border border-panel/40 bg-panel/15 px-3.5 py-2 backdrop-blur-[6px] sm:mb-0"
		>
			{#each details as { term, value } (term)}
				<div class="grid gap-0.5">
					<dt class="text-xs opacity-80">{term}</dt>
					<dd class="text-sm font-semibold">{value}</dd>
				</div>
			{/each}
		</dl>
	</div>
	<div class="flex gap-2.5 [&>*]:max-sm:flex-1">
		<Button
			variant="outline"
			size="sm"
			class="min-h-10! gap-2 border-panel/40! bg-panel/15! font-semibold text-panel! backdrop-blur-[6px] hover:bg-panel/25!"
			onclick={onpreview}><Eye size={16} aria-hidden="true" />{m.action_preview()}</Button
		>
		{#if archived}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={onrestore}
				><ArchiveRestore size={16} aria-hidden="true" />{m.action_restore()}</Button
			>
		{:else if !shared}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={onshare}><Send size={16} aria-hidden="true" />{m.action_share()}</Button
			>
		{:else if !delivered}
			<Button
				size="sm"
				class="min-h-10! gap-2 bg-panel! font-semibold text-foreground!"
				onclick={ondeliver}><Send size={16} aria-hidden="true" />{m.action_deliver()}</Button
			>
		{/if}
	</div>

	{#if !archived}<JourneySteps {steps} />{/if}
</div>
