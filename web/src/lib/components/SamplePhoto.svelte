<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * The static sample both Settings previews draw on, light to dark so a mark is judged on both.
	 * No photo is loaded, so every workspace sees the same preview.
	 */
	let { label, children }: { label: string; children: Snippet } = $props();
</script>

<div
	role="img"
	aria-label={label}
	class="[container-type:inline-size] relative mb-14 aspect-[3/2] w-full max-w-[48rem] overflow-hidden rounded-xl bg-linear-to-br from-[#ecd3b6] via-[#b5aaa0] to-[#3d4852]"
>
	{@render children()}
</div>
