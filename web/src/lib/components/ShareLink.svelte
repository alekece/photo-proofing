<script lang="ts">
	import CopyLink from '#lib/components/CopyLink.svelte';

	let { token, origin }: { token: string; origin: string } = $props();
</script>

<CopyLink link={`${origin}/g/${token}`} />
