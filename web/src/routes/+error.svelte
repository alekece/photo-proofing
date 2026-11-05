<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { buttonVariants } from '#lib/components/ui/button/button.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';

	let heading: HTMLHeadingElement;

	const notFound = $derived(page.status === 404);
	const serverFault = $derived(page.status >= 500);
	const path = $derived(page.url.pathname);
	const onClientLink = $derived(path.startsWith('/g/'));
	const exit = $derived(
		path === '/dashboard' || path.startsWith('/dashboard/')
			? { label: 'Back to your galleries', href: '/dashboard' }
			: onClientLink
				? serverFault
					? { label: 'Try again', href: path }
					: null
				: { label: 'Go to sign in', href: '/login' }
	);
	const title = $derived(
		notFound
			? "We couldn't find this page."
			: serverFault
				? 'Something went wrong on our side.'
				: "We can't show this page."
	);
	const detail = $derived(
		notFound
			? onClientLink
				? 'The link may be old or mistyped. Ask your photographer for a new one.'
				: 'The link may be old or mistyped.'
			: serverFault
				? 'Please try again in a moment. If it keeps happening, let your photographer know.'
				: 'If you expected to see it, let your photographer know.'
	);

	onMount(() => heading.focus());
</script>

<svelte:head>
	<title>{notFound ? 'Page not found' : 'Something went wrong'} — PhotoProofing</title>
</svelte:head>

<PageShell width="narrow">
	<h1 bind:this={heading} tabindex="-1" class="text-3xl">{title}</h1>
	<p>{detail}</p>
	{#if exit}
		<a href={exit.href} class={buttonVariants({ variant: 'outline' })}>{exit.label}</a>
	{/if}
</PageShell>
