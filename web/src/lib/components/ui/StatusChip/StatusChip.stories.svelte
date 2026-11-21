<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import StatusChip, { type GalleryStatus } from './StatusChip.svelte';

	const { Story } = defineMeta({ title: 'UI/StatusChip', component: StatusChip });

	const statuses: GalleryStatus[] = ['draft', 'shared', 'delivered', 'expired', 'archived'];
</script>

{#snippet chips(mode: 'default' | 'cover' | 'workspace')}
	<div class="flex flex-wrap gap-2">
		{#each statuses as status (status)}
			<StatusChip {status} dot cover={mode === 'cover'} workspace={mode === 'workspace'} />
		{/each}
	</div>
{/snippet}

<Story name="Statuses" asChild>{@render chips('default')}</Story>
<Story name="Cover" asChild>{@render chips('cover')}</Story>
<Story name="Workspace" asChild>{@render chips('workspace')}</Story>
