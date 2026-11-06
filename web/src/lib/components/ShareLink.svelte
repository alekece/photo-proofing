<script lang="ts">
	import Button from '#lib/components/ui/button/button.svelte';
	import Input from '#lib/components/ui/input/input.svelte';

	let { token, origin }: { token: string; origin: string } = $props();

	let copyStatus = $state('');
	let input = $state<HTMLInputElement | null>(null);

	const link = $derived(`${origin}/g/${token}`);

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(link);
			copyStatus = 'Copied';
		} catch {
			input?.focus();
			input?.select();
			copyStatus = 'Couldn’t copy automatically. Please copy it manually.';
		}
	}
</script>

<div class="grid gap-2">
	<div class="flex gap-2">
		<Input
			bind:ref={input}
			readonly
			value={link}
			aria-label="Client link"
			class="min-w-0 flex-1 text-sm"
		/>
		<Button variant="outline" size="sm" onclick={copyLink}>Copy link</Button>
		<Button variant="outline" size="sm" href={link} target="_blank" rel="noopener noreferrer"
			>Open</Button
		>
	</div>
	<p class="text-xs" role="status">{copyStatus}</p>
</div>
