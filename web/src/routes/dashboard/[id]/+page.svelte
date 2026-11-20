<script lang="ts">
	import GalleryEditor from './GalleryEditor.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

{#key data.gallery.id}
	<GalleryEditor {data} />
{/key}
