<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import SiteFooter from '#lib/components/SiteFooter.svelte';
	import WorkspaceLayout from '#lib/components/WorkspaceLayout.svelte';
	import Toaster from '#lib/components/ui/Toast/Toaster.svelte';
	import { page } from '$app/state';
	import { DEFAULT_SETTINGS, type WorkspaceSettings } from '#lib/settings/settings.js';
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
	const settings = $derived((page.data?.settings as WorkspaceSettings | null) ?? DEFAULT_SETTINGS);
	const previewCounts =
		import.meta.env.DEV && import.meta.env.VITE_WORKSPACE_PREVIEW_COUNTS
			? (JSON.parse(import.meta.env.VITE_WORKSPACE_PREVIEW_COUNTS) as Record<
					'galleries' | 'photos' | 'contacts',
					number
				>)
			: undefined;
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="flex min-h-dvh flex-col">
	{#if workspaceActive}
		<div
			data-studio-mode={settings.studioMode}
			data-studio-palette={settings.studioPalette}
			class="flex flex-1 flex-col bg-background text-foreground"
		>
			<WorkspaceLayout studioName={settings.studioName} counts={previewCounts}>
				{@render children()}
			</WorkspaceLayout>
			<Toaster studio />
		</div>
	{:else}
		<div class="flex flex-1 flex-col">
			{@render children()}
		</div>
		<SiteFooter />
		<Toaster />
	{/if}
</div>
