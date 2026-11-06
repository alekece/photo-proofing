<script lang="ts">
	import { SIGNATURES, type Signature } from '#lib/settings/settings.js';

	let {
		value = $bindable(),
		studioName,
		logo
	}: { value: Signature; studioName: string; logo: string | null } = $props();

	const LABELS: Record<Signature, string> = {
		name: 'Name',
		logo: 'Logo',
		both: 'Both',
		none: 'None'
	};
	const name = $derived(studioName.trim() || 'Your studio');

	function disabled(signature: Signature) {
		return logo === null && (signature === 'logo' || signature === 'both');
	}
</script>

<div role="radiogroup" aria-label="Cover signature" class="grid grid-cols-2 gap-3 sm:grid-cols-4">
	{#each SIGNATURES as signature (signature)}
		<button
			type="button"
			role="radio"
			aria-checked={value === signature}
			disabled={disabled(signature)}
			class="grid cursor-pointer gap-2 rounded-card border p-2 text-left disabled:cursor-not-allowed disabled:opacity-50 {value ===
			signature
				? 'border-foreground'
				: 'border-border-strong'}"
			onclick={() => (value = signature)}
		>
			<span
				aria-hidden="true"
				class="grid aspect-[3/2] content-between justify-items-center bg-dark p-2 text-inverse"
			>
				<span class="inline-flex items-center gap-1 text-inverse/80">
					{#if (signature === 'logo' || signature === 'both') && logo}
						<img src={logo} alt="" class="h-3 w-auto" />
					{/if}
					{#if signature === 'name' || signature === 'both'}
						<span class="font-mono text-[8px] tracking-widest uppercase">{name}</span>
					{/if}
				</span>
				<span class="font-serif text-xs">Gallery title</span>
			</span>
			<span class="text-sm font-medium">{LABELS[signature]}</span>
		</button>
	{/each}
</div>
