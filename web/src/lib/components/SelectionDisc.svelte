<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	let {
		label,
		selected,
		visible = false,
		touchLabel = false,
		onclick
	}: {
		label: string;
		selected: boolean;
		visible?: boolean;
		touchLabel?: boolean;
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	aria-label={label}
	aria-pressed={selected}
	{onclick}
	class="absolute top-1 left-1 z-10 inline-flex min-h-11 min-w-11 cursor-pointer items-center justify-center gap-1 rounded-full focus-visible:opacity-100 {visible ||
	selected
		? ''
		: 'opacity-0 group-hover:opacity-100 [@media(hover:none)]:opacity-100'}"
>
	<span
		class="relative grid size-[26px] place-items-center rounded-full text-white {selected
			? 'bg-dark/85'
			: 'bg-black/35'}"
	>
		<svg
			aria-hidden="true"
			viewBox="0 0 26 26"
			class="absolute inset-0 size-full fill-none stroke-white"
		>
			<circle cx="13" cy="13" r="12" stroke-width="1.25" />
		</svg>
		{#if selected}<Check size={16} aria-hidden="true" />{/if}
	</span>
	{#if touchLabel && !selected}<span
			aria-hidden="true"
			class="hidden rounded-full bg-dark/80 px-2 py-1 text-xs text-white [@media(hover:none)]:inline"
		>
			{m.action_select()}
		</span>{/if}
</button>
