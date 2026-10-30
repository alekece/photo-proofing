<script lang="ts">
	import AccessUnavailable from '#lib/components/AccessUnavailable.svelte';
	import ClientGallery from '#lib/components/ClientGallery.svelte';
	import { UNTITLED_GALLERY } from '#lib/gallery/format.js';
	import type { SharedGallery } from '#lib/gallery/types.js';

	let { data }: { data: { gallery: SharedGallery | null } } = $props();
</script>

<svelte:head>
	<title>{data.gallery ? (data.gallery.title ?? UNTITLED_GALLERY) : 'Gallery unavailable'}</title>
	<meta name="referrer" content="no-referrer" />
	<meta name="robots" content="noindex" />
</svelte:head>

{#if data.gallery}
	<ClientGallery gallery={data.gallery} />
{:else}
	<AccessUnavailable />
{/if}
