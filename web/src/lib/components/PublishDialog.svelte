<script lang="ts">
	import { onMount } from 'svelte';
	import ConfirmDialog from '#lib/components/ui/confirm-dialog/confirm-dialog.svelte';
	import Button from '#lib/components/ui/button/button.svelte';
	import PhotoStack from '#lib/components/ui/photo-stack/photo-stack.svelte';
	import { formatLongDate } from '#lib/gallery/format.js';
	import { listPhotos, type Photo } from '#lib/gallery/photos.js';

	/**
	 * Share and Deliver never refuse: when photos are missing, uploading or failed they warn and
	 * the button says "anyway". Deliver counts the proofs for now; finished photos arrive with #71.
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
	let copyStatus = $state('');

	let ready = $derived(photos?.filter(({ status }) => status === 'ready') ?? []);
	let uploading = $derived(
		uploadingFiles +
			(photos?.filter(({ status }) => status === 'waiting' || status === 'processing').length ?? 0)
	);
	let failed = $derived(photos?.filter(({ status }) => typeof status === 'object').length ?? 0);
	let empty = $derived(ready.length === 0);
	let warned = $derived(empty || uploading > 0 || failed > 0);
	let until = $derived(expiresAt ? formatLongDate(expiresAt) : '');

	let shareText = $derived(
		`This creates a private link. Anyone with it can see the proofs and pick favorites${until ? ` until ${until}` : ''}.`
	);
	const noun = $derived(kind === 'share' ? 'Share' : 'Deliver');
	const photoCount = (count: number) => `${count} ${count === 1 ? 'photo' : 'photos'}`;
	let issues = $derived(
		[
			uploading > 0 && `${uploading} ${uploading === 1 ? 'is' : 'are'} still uploading`,
			failed > 0 && `${failed} failed`
		]
			.filter(Boolean)
			.join(' and ')
	);

	onMount(() => {
		load(galleryId)
			.then((list) => (photos = list))
			.catch(() => {
				checkFailed = true;
				photos = [];
			});
	});

	async function copy() {
		try {
			await navigator.clipboard.writeText(link);
			copyStatus = 'Copied';
		} catch {
			copyStatus = 'Couldn’t copy automatically. Use the client link on the page.';
		}
	}
</script>

<ConfirmDialog
	title={!photos
		? `${noun}?`
		: empty
			? `${noun} with no photos?`
			: `${noun} ${photoCount(ready.length)}?`}
	confirmLabel={warned && photos ? `${noun} anyway` : noun}
	busy={photos === null}
	onconfirm={async () => {
		link = (await onconfirm()) ?? '';
	}}
	{onclose}
	result={{
		title: kind === 'share' ? 'Your gallery is shared' : 'Your gallery is delivered',
		body: resultBody
	}}
>
	{#snippet visual(done)}
		<PhotoStack {galleryId} photoIds={ready.map(({ id }) => id)} {done} />
	{/snippet}
	{#snippet description()}
		{#if photos === null}
			<p class="text-sm text-muted-foreground" role="status">Checking your photos…</p>
		{:else}
			<p class="text-sm text-muted-foreground">
				{#if kind === 'share'}
					{shareText}
				{:else}
					{title}
					{title.includes('&') ? 'see' : 'sees'} them at the same link and can download them. Favorites
					are locked from now on.
				{/if}
			</p>
			{#if empty}
				<p class="text-sm font-bold text-warning">
					{kind === 'share'
						? 'Clients see an empty gallery until you upload.'
						: 'Clients see nothing to download until you upload.'}
				</p>
			{/if}
			{#if issues}
				<p class="text-sm font-bold text-warning">{issues[0].toUpperCase() + issues.slice(1)}.</p>
			{/if}
			{#if checkFailed}
				<p class="text-sm font-bold text-warning">Couldn’t check the photos.</p>
			{/if}
		{/if}
	{/snippet}
</ConfirmDialog>

{#snippet resultBody(close: () => void)}
	<p class="text-center text-sm text-muted-foreground">
		{kind === 'share' ? 'Send the link to your client.' : 'Your client can now download them.'}
	</p>
	<p class="text-center text-xs" role="status">{copyStatus}</p>
	<div class="flex justify-center gap-2">
		<Button variant="ghost" onclick={close}>Done</Button>
		{#if link}<Button onclick={copy}>Copy link</Button>{/if}
	</div>
{/snippet}
