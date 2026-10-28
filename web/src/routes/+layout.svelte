<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

<style>
	:global(:root) {
		color-scheme: light dark;
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		--focus: #1a73e8;
		--accent: #1a73e8;
	}
	:global(body) {
		margin: 0;
		line-height: 1.5;
	}
	@media (prefers-color-scheme: dark) {
		:global(:root) {
			--focus: #8ab4f8;
			--accent: #8ab4f8;
		}
	}
</style>
