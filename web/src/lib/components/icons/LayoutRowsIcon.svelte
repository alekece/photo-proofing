<script lang="ts">
	let { size = 18 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
	<rect x="3" y="4" width="11" height="7" rx="1" />
	<rect x="16" y="4" width="5" height="7" rx="1" />
	<rect x="3" y="13" width="6" height="7" rx="1" />
	<rect x="11" y="13" width="10" height="7" rx="1" />
</svg>
