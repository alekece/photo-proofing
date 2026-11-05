<script lang="ts">
	import Checkbox from '#lib/components/ui/checkbox/checkbox.svelte';
	import type { GalleryState } from '#lib/gallery/schemas.js';
	import { ALL_STATES, STATE_LABELS } from '#lib/gallery/states.js';

	let {
		selected,
		onchange
	}: { selected: GalleryState[]; onchange: (next: GalleryState[]) => void } = $props();

	function toggle(state: GalleryState, checked: boolean) {
		onchange(checked ? [...selected, state] : selected.filter((chosen) => chosen !== state));
	}
</script>

<fieldset class="flex flex-wrap items-center gap-x-4 gap-y-2">
	<legend class="sr-only">State</legend>
	<span class="text-sm" aria-hidden="true">State</span>
	{#each ALL_STATES as state (state)}
		<label class="flex items-center gap-1.5 text-sm">
			<Checkbox
				checked={selected.includes(state)}
				onchange={(event) => toggle(state, event.currentTarget.checked)}
			/>
			{STATE_LABELS[state]}
		</label>
	{/each}
</fieldset>
