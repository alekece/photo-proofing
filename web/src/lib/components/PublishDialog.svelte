<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { onMount } from 'svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import GallerySharePanel from '#lib/components/GallerySharePanel.svelte';
	import type { Gallery } from '#lib/gallery/api.js';
	import type { InvitationDraft } from '#lib/gallery/types.js';
	import type { WorkspaceSettings } from '#lib/settings/settings.js';
	import ShareLink from '#lib/components/ShareLink.svelte';
	import PhotoStack from '#lib/components/ui/PhotoStack/PhotoStack.svelte';
	import { formatLongDate } from '#lib/gallery/format.js';
	import { listPhotos, summarizePhotoReadiness, type Photo } from '#lib/gallery/photos.js';

	/**
	 * Share and Deliver never refuse: when photos are missing, uploading or failed they warn and
	 * the button says "anyway". Deliver checks only finished photos.
	 */
	type Props = {
		kind: 'share' | 'deliver';
		galleryId: string;
		/** ISO date the link works until; only Share mentions it. */
		expiresAt?: string | null;
		/** Files of this gallery still on their way to becoming photos. */
		uploading?: number;
		load?: typeof listPhotos;
		gallery?: Gallery;
		settings?: WorkspaceSettings;
		heroRevision?: number;
		/** Origin of the client link shown once the gallery is shared. */
		origin?: string;
		/** Resolves with what happened when the stage was applied, or nothing when it was only reconciled. */
		onconfirm: (email: InvitationDraft | undefined) => Promise<{ emailed: boolean } | void>;
		onclose: () => void;
	};

	let {
		kind,
		galleryId,
		expiresAt = null,
		uploading: uploadingFiles = 0,
		load = listPhotos,
		gallery,
		settings,
		heroRevision,
		origin = globalThis.location?.origin ?? '',
		onconfirm,
		onclose
	}: Props = $props();

	let photos = $state<Photo[] | null>(null);
	let checkFailed = $state(false);
	let expanded = $state(false);
	let blocked = $state(false);
	let editor = $state<GallerySharePanel>();
	let outcome = $state<{ emailed: boolean }>();
	const emailed = $derived(outcome?.emailed ?? false);

	const readiness = $derived(photos ? summarizePhotoReadiness(photos, uploadingFiles) : null);
	let ready = $derived(readiness?.ready ?? []);
	let uploading = $derived(readiness?.uploadingCount ?? uploadingFiles);
	let failed = $derived(readiness?.failedCount ?? 0);
	let empty = $derived(readiness?.empty ?? true);
	let until = $derived(expiresAt ? formatLongDate(expiresAt) : '');

	let shareText = $derived(until ? m.publish_share_text_until({ until }) : m.publish_share_text());
	const sharing = $derived(kind === 'share');
	let issues = $derived(
		new Intl.ListFormat(getLocale(), { type: 'conjunction' }).format(
			[
				uploading > 0 ? m.publish_uploading({ count: uploading }) : '',
				failed > 0 ? m.publish_failed({ count: failed }) : ''
			].filter(Boolean)
		)
	);

	onMount(() => {
		load(galleryId, fetch, kind === 'deliver' ? 'finals' : 'photos')
			.then((list) => (photos = list))
			.catch(() => {
				checkFailed = true;
				photos = [];
			});
	});
</script>

{#snippet sharedBody()}
	{#if emailed}
		<p class="text-sm text-muted-foreground">{m.share_sent_note()}</p>
	{:else if gallery?.share_token}
		<p class="text-sm text-muted-foreground">{m.share_link_note()}</p>
		<ShareLink token={gallery.share_token} {origin} />
	{/if}
{/snippet}

<ConfirmDialog
	title={!photos || checkFailed
		? sharing
			? m.publish_share_question()
			: m.publish_deliver_question()
		: empty
			? sharing
				? m.publish_share_empty()
				: m.publish_deliver_empty()
			: sharing
				? m.publish_share_count({ count: ready.length })
				: m.publish_deliver_count({ count: ready.length })}
	confirmLabel={sharing ? m.action_share() : m.action_deliver()}
	busy={photos === null || blocked}
	{expanded}
	onconfirm={async () => {
		const draft = editor?.draft();

		outcome = (await onconfirm(draft)) ?? undefined;
	}}
	result={sharing && outcome && gallery?.share_token
		? { title: m.publish_shared_title(), body: sharedBody }
		: undefined}
	{onclose}
>
	{#snippet visual(done)}
		<PhotoStack
			{galleryId}
			photoIds={ready.map(({ id }) => id)}
			set={sharing ? 'photos' : 'finals'}
			polaroid
			{done}
		/>
	{/snippet}
	{#snippet description()}
		{#if photos === null}
			<p class="text-sm text-muted-foreground" role="status">{m.publish_checking()}</p>
		{:else}
			{#if empty && !checkFailed}
				<p class="text-sm font-bold text-warning">
					{sharing ? m.publish_empty_share() : m.publish_empty_deliver()}
				</p>
			{/if}
			{#if issues}
				<p class="text-sm font-bold text-warning">{issues[0].toUpperCase() + issues.slice(1)}.</p>
			{/if}
			{#if checkFailed}
				<p class="text-sm font-bold text-warning">{m.publish_check_failed()}</p>
			{/if}
			{#if !expanded}
				{#if editor?.recipientNames()}
					{#if sharing && gallery?.share_token}
						<p class="text-sm font-semibold text-terracotta">{m.share_already_shared()}</p>
					{/if}
					<p class="text-sm wrap-anywhere text-foreground">
						{m.share_will_email({ names: editor.recipientNames() })}
					</p>
				{:else if sharing}
					<p class="text-sm text-foreground">{shareText}</p>
				{/if}
			{/if}
		{/if}
	{/snippet}
	{#snippet content(pending)}
		{#if gallery}<GallerySharePanel
				bind:this={editor}
				{gallery}
				{settings}
				{heroRevision}
				stage={sharing ? 'proofing' : 'finals'}
				visibleCount={checkFailed || !photos ? null : ready.length}
				sending={pending}
				bind:expanded
				bind:blocked
			/>{/if}
	{/snippet}
	{#snippet secondaryAction(pending)}
		{#if gallery && editor?.canEmail() && !expanded}<Button
				variant="outline"
				aria-expanded={expanded}
				disabled={pending || !editor?.canCustomize()}
				onclick={() => editor?.toggleCustomization()}
			>
				{m.stage_customize()}
			</Button>{/if}
	{/snippet}
</ConfirmDialog>
