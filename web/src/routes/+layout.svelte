<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import SiteFooter from '#lib/components/SiteFooter.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';
	import { page } from '$app/state';
	import { cn } from '#lib/utils.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const workspaceRoutes = new Set([
		'/dashboard',
		'/dashboard/[id]',
		'/photos',
		'/contacts',
		'/activity',
		'/settings'
	]);
	const workspaceActive = $derived(workspaceRoutes.has(page.route.id ?? ''));
	const workspaceClearance = 'sm:ml-[clamp(15rem,16vw,17.5rem)]';
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="flex min-h-dvh flex-col">
	<div class={cn('flex flex-1 flex-col', workspaceActive && workspaceClearance)}>
		{@render children()}
	</div>
	<SiteFooter class={workspaceActive ? workspaceClearance : undefined} />
</div>
<Toaster />
