<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { onMount } from 'svelte';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import CopyLink from '#lib/components/CopyLink.svelte';
	import PhotoStack from '#lib/components/ui/PhotoStack/PhotoStack.svelte';
	import { formatLongDate } from '#lib/gallery/format.js';
	import { listPhotos, type Photo } from '#lib/gallery/photos.js';

	/**
	 * Share and Deliver never refuse: when photos are missing, uploading or failed they warn and
	 * the button says "anyway". Deliver checks only finished photos.
	 * `onconfirm` may resolve with the client link to offer after sharing.
	 */
	type Props = {
		kind: 'share' | 'deliver';
		galleryId: string;
		/** Who the delivered photos are for; Deliver names them. */
		title: string;
		/** ISO date the link works until; only Share mentions it. */
		expiresAt?: string | null;
		/** Files of this gallery still on their way to becoming photos. */
		uploading?: number;
		load?: typeof listPhotos;
		onconfirm: () => Promise<string | void>;
		onclose: () => void;
	};

	let {
		kind,
		galleryId,
		title,
		expiresAt = null,
		uploading: uploadingFiles = 0,
		load = listPhotos,
		onconfirm,
		onclose
	}: Props = $props();

	let photos = $state<Photo[] | null>(null);
	let checkFailed = $state(false);
	let link = $state('');

	let ready = $derived(photos?.filter(({ status }) => status === 'ready') ?? []);
	let uploading = $derived(
		uploadingFiles +
			(photos?.filter(({ status }) => status === 'waiting' || status === 'processing').length ?? 0)
	);
	let failed = $derived(photos?.filter(({ status }) => typeof status === 'object').length ?? 0);
	let empty = $derived(ready.length === 0);
	let warned = $derived(empty || uploading > 0 || failed > 0);
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

<ConfirmDialog
	title={!photos
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
	confirmLabel={warned && photos
		? sharing
			? m.publish_share_anyway()
			: m.publish_deliver_anyway()
		: sharing
			? m.action_share()
			: m.action_deliver()}
	busy={photos === null}
	onconfirm={async () => {
		link = (await onconfirm()) ?? '';
	}}
	{onclose}
	result={{
		title: sharing ? m.publish_shared_title() : m.publish_delivered_title(),
		body: resultBody
	}}
>
	{#snippet visual(done)}
		<PhotoStack
			{galleryId}
			photoIds={ready.map(({ id }) => id)}
			set={kind === 'deliver' ? 'finals' : 'photos'}
			{done}
		/>
	{/snippet}
	{#snippet description()}
		{#if photos === null}
			<p class="text-sm text-muted-foreground" role="status">{m.publish_checking()}</p>
		{:else}
			<p class="text-sm text-muted-foreground">
				{#if kind === 'share'}
					{shareText}
				{:else}
					{title.includes('&')
						? m.publish_deliver_text_many({ title })
						: m.publish_deliver_text({ title })}
				{/if}
			</p>
			{#if empty}
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
		{/if}
	{/snippet}
</ConfirmDialog>

{#snippet resultBody(close: () => void)}
	<p class="text-center text-sm text-muted-foreground">
		{sharing ? m.publish_send_link() : m.publish_can_download()}
	</p>
	{#if link}<CopyLink {link} />{/if}
	<div class="flex justify-center">
		<Button variant="ghost" onclick={close}>{m.action_done()}</Button>
	</div>
{/snippet}
