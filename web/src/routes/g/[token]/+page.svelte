<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { invalidateAll } from '$app/navigation';
	import AccessUnavailable from '#lib/components/AccessUnavailable.svelte';
	import ClientAccessGate from '#lib/components/ClientAccessGate.svelte';
	import ClientGallery from '#lib/components/ClientGallery.svelte';
	import { recordOpened } from '#lib/gallery/opened.js';
	import type { LockedSharedGallery, Proof, SharedGallery } from '#lib/gallery/types.js';

	let {
		data,
		params
	}: {
		data: {
			gallery: SharedGallery | null;
			lockedGallery?: LockedSharedGallery | null;
			proofs: Proof[];
			favorites: string[];
			coverUrl?: string;
		};
		params: { token: string };
	} = $props();

	let reportedOpen = false;

	// Only a browser showing an unlocked available gallery counts as an open, whether initially
	// unlocked or unlocked through the 6-digit gate on this page.
	$effect(() => {
		if (data.gallery && !reportedOpen) {
			reportedOpen = true;
			void recordOpened(params.token);
		}
	});

	const pageTitle = $derived(
		data.gallery
			? (data.gallery.title ?? m.gallery_untitled())
			: data.lockedGallery
				? (data.lockedGallery.title ?? m.gallery_untitled())
				: m.client_unavailable()
	);

	function oncontextmenu(event: MouseEvent) {
		const target = event.target;

		if (
			target instanceof Element &&
			(target.closest('img') ||
				target.closest('[data-image-context-menu-overlay]') ||
				target.closest('button')?.querySelector('img'))
		)
			event.preventDefault();
	}
</script>

<svelte:head>
	<title>{pageTitle}</title>
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex" />
</svelte:head>

<svelte:window {oncontextmenu} />

{#if data.gallery}
	<div class="client-gallery-protected">
		<ClientGallery
			token={params.token}
			gallery={data.gallery}
			proofs={data.proofs}
			coverUrl={data.coverUrl}
			favorites={data.favorites}
			studio={data.gallery.studio_name}
			studioLogo={data.gallery.has_logo
				? `/api/v1/galleries/shared/${encodeURIComponent(params.token)}/logo`
				: null}
			signature={data.gallery.signature}
			signatureSize={data.gallery.signature_size}
		/>
	</div>
{:else if data.lockedGallery}
	<div class="client-gallery-protected">
		<ClientAccessGate
			token={params.token}
			gallery={data.lockedGallery}
			coverUrl={data.coverUrl}
			onunlock={() => invalidateAll()}
		/>
	</div>
{:else}
	<AccessUnavailable />
{/if}

<style>
	.client-gallery-protected :global(img),
	.client-gallery-protected :global([data-image-context-menu-overlay]) {
		-webkit-touch-callout: none;
	}
</style>
