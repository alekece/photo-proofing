<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';
	import { shareGallery, type Gallery } from '#lib/gallery/api.js';

	let {
		gallery,
		origin,
		share = shareGallery
	}: { gallery: Gallery; origin: string; share?: typeof shareGallery } = $props();

	// svelte-ignore state_referenced_locally
	let token = $state(gallery.share_token);
	let sharing = $state(false);
	let failed = $state(false);
	let copyStatus = $state('');
	let input = $state<HTMLInputElement | null>(null);

	const link = $derived(token ? `${origin}/g/${token}` : '');

	async function startSharing() {
		sharing = true;
		failed = false;
		try {
			token = (await share(gallery.id)).share_token;
		} catch {
			failed = true;
		} finally {
			sharing = false;
		}
	}

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(link);
			copyStatus = 'Copied';
		} catch {
			input?.focus();
			input?.select();
			copyStatus = 'Couldn’t copy automatically. Please copy it manually.';
		}
	}
</script>

<div class="grid gap-2">
	<h2 class="text-sm font-medium">Client link</h2>
	{#if token}
		<div class="flex gap-2">
			<Input
				bind:ref={input}
				readonly
				value={link}
				aria-label="Client link"
				class="rounded-md border bg-background px-2 py-1 text-sm"
			/>
			<Button variant="outline" size="sm" onclick={copyLink}>Copy</Button>
		</div>
		<p class="text-xs" role="status">{copyStatus}</p>
		<!-- Always shown until the photo count from #66 can tell an empty gallery from a filled one. -->
		<p class="text-xs text-amber-700">
			This gallery has no photos yet. Clients will see an empty gallery.
		</p>
	{:else}
		<Button variant="outline" size="sm" onclick={startSharing} disabled={sharing}>Share</Button>
		{#if failed}
			<p class="text-xs text-destructive" role="alert">Couldn’t share the gallery. Please try again.</p>
		{/if}
	{/if}
</div>
