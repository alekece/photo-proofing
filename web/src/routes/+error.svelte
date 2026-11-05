<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { buttonVariants } from '#lib/components/ui/button/button.svelte';
	import PageShell from '#lib/components/ui/page-shell/page-shell.svelte';

	let heading: HTMLHeadingElement;

	const notFound = $derived(page.status === 404);
	const path = $derived(page.url.pathname);
	const exit = $derived(
		path === '/dashboard' || path.startsWith('/dashboard/')
			? { label: 'Back to your galleries', href: '/dashboard' }
			: path.startsWith('/g/')
				? { label: 'Try again', href: path }
				: { label: 'Go to sign in', href: '/login' }
	);

	onMount(() => heading.focus());
</script>

<svelte:head>
	<title>{notFound ? 'Page not found' : 'Something went wrong'} — PhotoProofing</title>
</svelte:head>

<PageShell width="narrow">
	<h1 bind:this={heading} tabindex="-1" class="text-3xl">
		{notFound ? "We couldn't find this page." : 'Something went wrong on our side.'}
	</h1>
	<p>
		{notFound
			? 'The link may be old or mistyped.'
			: 'Please try again in a moment. If it keeps happening, let your photographer know.'}
	</p>
	<a href={exit.href} class={buttonVariants({ variant: 'outline' })}>{exit.label}</a>
</PageShell>
