<script lang="ts">
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';

	let { onconfirm, onclose }: { onconfirm: () => void | Promise<void>; onclose: () => void } =
		$props();
</script>

<ConfirmDialog title="Archive this gallery?" confirmLabel="Archive" {onconfirm} {onclose}>
	{#snippet description()}
		<p class="text-sm text-muted-foreground">
			The client link stops working. You can restore it any time.
		</p>
	{/snippet}
</ConfirmDialog>
