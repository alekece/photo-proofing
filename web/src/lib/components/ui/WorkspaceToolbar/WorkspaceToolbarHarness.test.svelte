<script lang="ts">
	import WorkspaceToolbar from './WorkspaceToolbar.svelte';

	let {
		height = 0,
		searchable = true,
		disabled = false
	}: {
		height?: number;
		searchable?: boolean;
		disabled?: boolean;
	} = $props();

	let query = $state('');
</script>

<WorkspaceToolbar
	search={searchable
		? { label: 'Search', value: query, disabled, oninput: (value) => (query = value) }
		: undefined}
	resultsHeight={height}
>
	{#snippet actions()}<button type="button">Action</button>{/snippet}
</WorkspaceToolbar>

<output>{query}</output>
