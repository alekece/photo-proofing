<script lang="ts">
	import type { Proof } from '../gallery/types';
	import ProofViewer from './ProofViewer.svelte';

	let { proofs }: { proofs: Proof[] } = $props();
	let index = $state<number | null>(null);
</script>

<button onclick={() => (index = 0)}>Open viewer</button>
{#if index !== null}
	<ProofViewer {proofs} bind:index onclose={() => (index = null)} />
{/if}
