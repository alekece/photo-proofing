<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import * as m from '#lib/paraglide/messages.js';

	let { open, onclick }: { open: boolean; onclick: () => void } = $props();
</script>

<button
	type="button"
	aria-expanded={open}
	aria-label={open ? m.contact_notes_hide() : m.contact_notes_show()}
	{onclick}
	class="icon-action relative grid size-9 shrink-0 place-items-center rounded-full text-muted-foreground @xl:hidden"
>
	<ChevronDown
		size={18}
		aria-hidden="true"
		class="transition-transform motion-reduce:transition-none {open ? 'rotate-180' : ''}"
	/>
</button>
