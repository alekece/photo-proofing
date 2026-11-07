<script lang="ts">
	import { onMount } from 'svelte';
	import AccessUnavailable from '#lib/components/AccessUnavailable.svelte';
	import ClientGallery from '#lib/components/ClientGallery.svelte';
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import { recordOpened } from '#lib/gallery/opened.js';
	import type { Proof, SharedGallery } from '#lib/gallery/types.js';

	let {
		data,
		params
	}: {
		data: { gallery: SharedGallery | null; proofs: Proof[]; favorites: string[] };
		params: { token: string };
	} = $props();

	// Only a browser showing an available gallery counts as an open, not link previews or the owner's preview.
	onMount(() => {
		if (data.gallery) void recordOpened(params.token);
	});
</script>

<svelte:head>
	<title>{data.gallery ? (data.gallery.title ?? UNTITLED_GALLERY) : 'Gallery unavailable'}</title>
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex" />
</svelte:head>

{#if data.gallery}
	<ClientGallery
		token={params.token}
		gallery={data.gallery}
		proofs={data.proofs}
		favorites={data.favorites}
		studio={data.gallery.studio_name}
		studioLogo={data.gallery.has_logo
			? `/api/v1/galleries/shared/${encodeURIComponent(params.token)}/logo`
			: null}
		signature={data.gallery.signature}
	/>
{:else}
	<AccessUnavailable />
{/if}
