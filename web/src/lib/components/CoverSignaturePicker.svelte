<script lang="ts">
	import StudioSignature from '#lib/components/StudioSignature.svelte';
	import { SIGNATURES, type Signature } from '#lib/settings/settings.js';

	let {
		value = $bindable(),
		studioName,
		logo,
		cover = null
	}: {
		value: Signature;
		studioName: string;
		logo: string | null;
		/** A real gallery cover to sign; plain tiles stand in without one. */
		cover?: { image: string | null; title: string } | null;
	} = $props();

	const LABELS: Record<Signature, string> = {
		name: 'Name',
		logo: 'Logo',
		both: 'Both',
		none: 'None'
	};

	function disabled(signature: Signature) {
		return logo === null && (signature === 'logo' || signature === 'both');
	}
</script>

<div
	role="radiogroup"
	aria-label="Cover signature"
	class="grid grid-cols-[repeat(auto-fit,minmax(9.375rem,1fr))] gap-[1.125rem]"
>
	{#each SIGNATURES as signature (signature)}
		<button
			type="button"
			role="radio"
			aria-checked={value === signature}
			disabled={disabled(signature)}
			class="group grid cursor-pointer gap-2.5 text-left disabled:cursor-not-allowed disabled:opacity-40"
			onclick={() => (value = signature)}
		>
			<span
				aria-hidden="true"
				class="[container-type:inline-size] relative block aspect-[4/3] overflow-hidden rounded-xl bg-dark text-inverse outline-2 outline-offset-[3px] transition-[outline-color] group-hover:outline-[#cdbda8] after:absolute after:inset-0 after:bg-[linear-gradient(rgb(0_0_0/0.25),transparent_30%,transparent_50%,rgb(0_0_0/0.55))] after:content-[''] {value ===
				signature
					? 'outline-3! outline-foreground!'
					: 'outline-transparent'}"
			>
				{#if cover?.image}
					<img src={cover.image} alt="" class="block size-full object-cover" />
				{/if}
				<StudioSignature
					studio={studioName}
					{logo}
					{signature}
					class="absolute inset-x-0 top-[6cqw] z-10 gap-[2cqw] text-[3cqw]"
				/>
				<span class="absolute bottom-[7cqw] left-[7cqw] z-10 font-serif text-[8cqw] leading-none"
					>{cover?.title ?? 'Gallery title'}</span
				>
			</span>
			<span
				class="w-max rounded-full px-3.5 py-1 font-semibold transition-colors {value === signature
					? 'bg-foreground text-inverse'
					: ''}">{LABELS[signature]}</span
			>
		</button>
	{/each}
</div>
