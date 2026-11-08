<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import { page } from '$app/state';
	import SiteFooter from '#lib/components/SiteFooter.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	// Clients only see the version; copying it is for the photographer.
	const client = $derived(page.route.id?.startsWith('/g/') ?? false);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="flex min-h-dvh flex-col">
	<div class="flex flex-1 flex-col">
		{@render children()}
	</div>
	<SiteFooter copyable={!client} />
</div>
<Toaster />
