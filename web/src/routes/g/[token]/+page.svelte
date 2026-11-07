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
		data: {
			gallery: SharedGallery | null;
			proofs: Proof[];
			favorites: string[];
			coverUrl?: string;
		};
		params: { token: string };
	} = $props();

	// Only a browser showing an available gallery counts as an open, not link previews or the owner's preview.
	onMount(() => {
		if (data.gallery) void recordOpened(params.token);
	});

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
	<title>{data.gallery ? (data.gallery.title ?? UNTITLED_GALLERY) : 'Gallery unavailable'}</title>
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
