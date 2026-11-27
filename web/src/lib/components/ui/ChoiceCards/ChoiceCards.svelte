<script lang="ts" generics="T extends string">
	import Radio from '#lib/components/ui/Radio/Radio.svelte';

	let {
		value = $bindable(),
		label,
		describedBy,
		options
	}: {
		value: T;
		label: string;
		describedBy?: string;
		options: readonly { value: T; label: string; detail?: string; lang?: string }[];
	} = $props();

	const group = $props.id();
</script>

<div
	role="radiogroup"
	aria-label={label}
	aria-describedby={describedBy}
	class="grid gap-3 sm:gap-[1.125rem] {options.length === 2
		? 'max-w-[23.125rem] grid-cols-2'
		: 'grid-cols-[repeat(auto-fit,minmax(min(100%,11rem),1fr))]'}"
>
	{#each options as option (option.value)}
		<label
			lang={option.lang}
			class="relative grid min-w-0 cursor-pointer gap-1 rounded-card border control-choice px-4 py-3 has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-50 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2"
		>
			<Radio
				name={group}
				value={option.value}
				checked={option.value === value}
				class="cursor-inherit absolute inset-0 m-0 size-full opacity-0"
				onchange={() => (value = option.value)}
			/>
			<span class="font-semibold">{option.label}</span>
			{#if option.detail}
				<span class="text-sm opacity-75">{option.detail}</span>
			{/if}
		</label>
	{/each}
</div>
