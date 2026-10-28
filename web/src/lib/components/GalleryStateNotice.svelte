<script lang="ts">
	import type { GrantedGalleryView } from '../gallery/types';

	let { state }: { state: GrantedGalleryView['state'] } = $props();

	const MESSAGES: Record<GrantedGalleryView['state'], string> = {
		proofing: 'Browse the previews below and mark your favourites. Your choices are saved automatically.',
		selection_received:
			'Your selection has been received. The photographer is reviewing it; previews stay available for now.',
		retouching:
			'Your selected photos are being retouched. Previews are shown for reference; final files are not ready yet.',
		delivery: 'Your final photos are being prepared for delivery. Previews are no longer shown.'
	};
</script>

<p class="notice" role="status">{MESSAGES[state]}</p>

<style>
	.notice {
		margin: 0;
		padding: 0.75rem 1rem;
		border-left: 4px solid var(--accent, #1a73e8);
		background: var(--surface, rgb(128 128 128 / 0.12));
	}
</style>
