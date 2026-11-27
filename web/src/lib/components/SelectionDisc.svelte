<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	let {
		label,
		selected,
		visible = false,
		touchLabel = false,
		side = 'left',
		onclick
	}: {
		label: string;
		selected: boolean;
		visible?: boolean;
		touchLabel?: boolean;
		side?: 'left' | 'right';
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	aria-label={label}
	aria-pressed={selected}
	{onclick}
	class="absolute top-1 z-10 inline-flex min-h-11 min-w-11 cursor-pointer items-center justify-center gap-1 rounded-full focus-visible:opacity-100 {side ===
	'right'
		? 'right-1'
		: 'left-1'} {visible || selected
		? ''
		: 'opacity-0 group-hover:opacity-100 [@media(hover:none)]:opacity-100'}"
>
	<span
		class="relative grid size-[26px] place-items-center rounded-full border-[1.5px] text-panel shadow-[0_1px_0_rgb(255_255_255/0.1)_inset,0_5px_16px_rgb(22_18_15/0.14)] backdrop-blur-[10px] backdrop-saturate-[115%] transition-colors motion-reduce:transition-none {selected
			? 'border-[rgb(22_18_15/0.78)] bg-[rgb(22_18_15/0.78)]'
			: 'border-[rgb(22_18_15/0.3)] bg-[rgb(22_18_15/0.4)]'}"
	>
		{#if selected}<Check size={16} strokeWidth={2.5} aria-hidden="true" />{/if}
	</span>
	{#if touchLabel && !selected}<span
			aria-hidden="true"
			class="hidden pill bg-dark/80 text-xs text-white [@media(hover:none)]:inline"
		>
			{m.action_select()}
		</span>{/if}
</button>
