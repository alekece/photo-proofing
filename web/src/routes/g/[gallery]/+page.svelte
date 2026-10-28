<script lang="ts">
	let { data }: { data: { gallery: string } } = $props();
</script>

<h1>Gallery</h1>
<p>{data.gallery}</p>
