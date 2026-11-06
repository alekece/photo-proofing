<script lang="ts">
	let { size = 18 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
	<rect x="3" y="3" width="8" height="11" rx="1" />
	<rect x="13" y="3" width="8" height="6" rx="1" />
	<rect x="3" y="16" width="8" height="5" rx="1" />
	<rect x="13" y="11" width="8" height="10" rx="1" />
</svg>
