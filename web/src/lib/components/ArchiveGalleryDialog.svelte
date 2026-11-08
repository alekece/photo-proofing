<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import ConfirmDialog from '#lib/components/ui/ConfirmDialog/ConfirmDialog.svelte';

	let { onconfirm, onclose }: { onconfirm: () => void | Promise<void>; onclose: () => void } =
		$props();
</script>

<ConfirmDialog title={m.archive_title()} confirmLabel={m.action_archive()} {onconfirm} {onclose}>
	{#snippet description()}
		<p class="text-sm text-muted-foreground">
			{m.archive_body()}
		</p>
	{/snippet}
</ConfirmDialog>
