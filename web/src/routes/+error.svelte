<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { buttonVariants } from '#lib/components/ui/Button/Button.svelte';
	import PageShell from '#lib/components/ui/PageShell/PageShell.svelte';
	import { m } from '#lib/paraglide/messages.js';

	let heading: HTMLHeadingElement;

	const notFound = $derived(page.status === 404);
	const serverFault = $derived(page.status >= 500);
	const path = $derived(page.url.pathname);
	const onClientLink = $derived(path.startsWith('/g/'));
	const exit = $derived(
		path === '/dashboard' || path.startsWith('/dashboard/')
			? { label: m.error_page_back_to_galleries(), href: '/dashboard' }
			: onClientLink
				? serverFault
					? { label: m.error_page_try_again(), href: path }
					: null
				: { label: m.error_page_go_to_sign_in(), href: '/login' }
	);
	const title = $derived(
		notFound
			? m.error_page_title_not_found()
			: serverFault
				? m.error_page_title_server()
				: m.error_page_title_client()
	);
	const detail = $derived(
		notFound
			? onClientLink
				? m.error_page_detail_not_found_client()
				: m.error_page_detail_not_found()
			: serverFault
				? m.error_page_detail_server()
				: m.error_page_detail_client()
	);

	onMount(() => heading.focus());
</script>

<svelte:head>
	<title
		>{notFound ? m.error_page_head_not_found() : m.error_page_head_error()} — PhotoProofing</title
	>
</svelte:head>

<PageShell width="narrow">
	<h1 bind:this={heading} tabindex="-1" class="text-3xl">{title}</h1>
	<p>{detail}</p>
	{#if exit}
		<a href={exit.href} class={buttonVariants({ variant: 'outline' })}>{exit.label}</a>
	{/if}
</PageShell>
