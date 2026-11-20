<script lang="ts">
	import { ArrowUpDown } from '@lucide/svelte';
	import ToolbarMenu from './ToolbarMenu.svelte';

	let {
		activeCount = 0,
		disabled = false,
		onchange
	}: {
		activeCount?: number;
		disabled?: boolean;
		onchange?: (value: string) => void;
	} = $props();

	let value = $state('newest');
	const options = [
		{ value: 'newest', label: 'Newest' },
		{ value: 'oldest', label: 'Oldest' }
	];
</script>

<ToolbarMenu
	label="Sort by"
	summary={options.find((option) => option.value === value)?.label ?? ''}
	icon={ArrowUpDown}
	{activeCount}
	{disabled}
	{options}
	{value}
	onchange={(next) => {
		value = next;
		onchange?.(next);
	}}
/>
