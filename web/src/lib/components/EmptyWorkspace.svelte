<section
	class="relative grid min-h-96 place-items-center overflow-hidden rounded-dialog bg-dark p-6 text-center text-inverse"
	aria-label="Empty workspace"
>
	<div data-slot="contact-sheet" aria-hidden="true" class="absolute inset-0 overflow-hidden">
		<div class="sheet absolute -inset-x-4 -top-[100%] grid grid-cols-4 gap-3 px-8 sm:grid-cols-6">
			{#each Array.from({ length: 96 }, (_, index) => index) as frame (frame)}
				<div class="frame aspect-[3/2] rounded-sm border border-inverse/15"></div>
			{/each}
		</div>
		<div
			class="leak absolute inset-y-0 w-1/2 bg-gradient-to-r from-transparent via-hero/25 to-transparent"
		></div>
		<div class="absolute inset-0 bg-gradient-to-b from-dark/60 via-transparent to-dark/80"></div>
	</div>

	<div class="relative grid justify-items-center gap-3">
		<div class="focus relative grid size-40 place-items-center">
			<span class="corner top-0 left-0 border-t-2 border-l-2"></span>
			<span class="corner top-0 right-0 border-t-2 border-r-2"></span>
			<span class="corner bottom-0 left-0 border-b-2 border-l-2"></span>
			<span class="corner right-0 bottom-0 border-r-2 border-b-2"></span>
			<span class="text-sm tracking-widest text-autofocus">[ 0 ]</span>
		</div>
		<p class="font-mono text-xs tracking-widest text-inverse/70">1/125 · F2.8 · ISO 400 · [ 0 ]</p>
		<p class="brand text-2xl">Nothing in frame yet.</p>
		<p class="text-sm text-inverse/70">Create your first gallery to start.</p>
	</div>
</section>

<style>
	.corner {
		position: absolute;
		width: 1.25rem;
		height: 1.25rem;
		border-color: var(--color-autofocus);
	}
	.sheet {
		animation: scroll 120s linear infinite;
	}
	.leak {
		animation: leak 14s ease-in-out infinite alternate;
	}
	.focus {
		animation: breathe 4s ease-in-out infinite;
	}
	@keyframes scroll {
		to {
			transform: translateY(50%);
		}
	}
	@keyframes breathe {
		50% {
			transform: scale(1.06);
			opacity: 0.8;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.sheet,
		.leak,
		.focus {
			animation: none;
		}
	}
</style>
