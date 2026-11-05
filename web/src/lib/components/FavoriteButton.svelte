<script lang="ts">
	let {
		selected,
		label,
		onclick,
		class: className = ''
	}: { selected: boolean; label: string; onclick: () => void; class?: string } = $props();
</script>

<button type="button" class={className} aria-pressed={selected} aria-label={label} {onclick}>
	<svg
		aria-hidden="true"
		viewBox="0 0 24 24"
		class="size-6"
		fill={selected ? 'currentColor' : 'none'}
		stroke="currentColor"
		stroke-width="2"
		stroke-linejoin="round"
	>
		<path
			d="M12 21s-7.5-4.6-9.6-9.2C.9 8.3 3 4.5 6.8 4.5c2.1 0 3.9 1.2 5.2 3 1.3-1.8 3.1-3 5.2-3 3.8 0 5.9 3.8 4.4 7.3C19.5 16.4 12 21 12 21z"
		/>
	</svg>
</button>
