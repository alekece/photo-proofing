<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { OwnerExport } from '#lib/gallery/export.svelte.js';
	import Button from './ui/Button/Button.svelte';

	let { zip, individualError }: { zip: OwnerExport; individualError?: string | null } = $props();
</script>

{#if zip.statusError}
	<div class="flex flex-wrap items-center justify-end gap-2 text-sm text-danger">
		<p role="alert">{zip.statusError}</p>
		<Button
			variant="outline"
			size="sm"
			class="min-h-9!"
			disabled={zip.refreshing}
			aria-busy={zip.refreshing}
			onclick={() => void zip.retryStatus()}>{m.owner_zip_retry_status()}</Button
		>
	</div>
{:else if individualError || zip.error}
	<p role="alert" class="text-right text-sm text-danger">
		{individualError ?? zip.error}
		{m.favorites_try_again()}
	</p>
{/if}
