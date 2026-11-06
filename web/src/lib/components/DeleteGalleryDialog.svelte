<script lang="ts">
	import ConfirmDialog from '#lib/components/ui/confirm-dialog/confirm-dialog.svelte';

	let { onconfirm, onclose }: { onconfirm: () => void | Promise<void>; onclose: () => void } =
		$props();
</script>

<ConfirmDialog
	title="Delete this gallery?"
	confirmLabel="Delete"
	cancelLabel="Keep it"
	destructive
	{onconfirm}
	{onclose}
>
	{#snippet description()}
		<p class="text-sm text-muted-foreground">Its photos, favorites and client link go for good.</p>
	{/snippet}
</ConfirmDialog>
