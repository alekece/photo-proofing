<script lang="ts">
	import "../app.css";
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
		color-scheme: light;
		font-family: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		--focus: #2b2420;
	}
	:global(body) {
		margin: 0;
		line-height: 1.5;
	}
</style>
