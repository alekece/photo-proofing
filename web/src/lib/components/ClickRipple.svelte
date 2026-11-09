<script lang="ts">
	let {
		onend,
		class: className = 'rounded-full'
	}: {
		/** Called when the ripple finishes, so the owner can remove or re-arm it. */
		onend?: () => void;
		/** Shape of the ring; it fills the nearest positioned parent. */
		class?: string;
	} = $props();
</script>

<span
	onanimationend={() => onend?.()}
	aria-hidden="true"
	data-testid="click-ripple"
	class="pointer-events-none absolute inset-0 animate-click-ripple border-2 border-current {className}"
></span>
