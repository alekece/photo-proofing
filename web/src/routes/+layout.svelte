<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import SiteFooter from '#lib/components/SiteFooter.svelte';
	import WorkspaceLayout from '#lib/components/WorkspaceLayout.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';
	import { page } from '$app/state';
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
	const studioName = $derived((page.data?.settings as { studioName?: string } | null)?.studioName);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="flex min-h-dvh flex-col">
	{#if workspaceActive}
		<WorkspaceLayout {studioName}>{@render children()}</WorkspaceLayout>
	{:else}
		<div class="flex flex-1 flex-col">
			{@render children()}
		</div>
		<SiteFooter />
	{/if}
</div>
<Toaster />
