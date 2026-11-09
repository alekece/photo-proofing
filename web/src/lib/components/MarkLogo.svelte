<script lang="ts">
	/** The studio logo, or a placeholder mark until one is uploaded. */
	let { logo, class: className = '' }: { logo: string | null; class?: string } = $props();
</script>

{#if logo}
	<img src={logo} alt="" class="{className} object-contain" />
{:else}
	<svg viewBox="0 0 48 48" aria-hidden="true" class={className}>
		<circle cx="24" cy="24" r="22" fill="none" stroke="currentColor" stroke-width="1.6" />
		<path
			d="M15 31c3-9 6-14 9-14s4 5 5 9 2 5 4 5"
			fill="none"
			stroke="currentColor"
			stroke-width="1.6"
			stroke-linecap="round"
		/>
		<circle cx="31" cy="15" r="1.8" fill="currentColor" />
	</svg>
{/if}
