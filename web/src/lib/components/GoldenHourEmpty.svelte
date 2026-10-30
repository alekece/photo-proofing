<section
	class="empty grid justify-items-center gap-3 rounded-xl px-6 py-16 text-center"
	aria-labelledby="golden-hour-title"
>
	<svg width="150" height="80" viewBox="0 0 130 70" aria-hidden="true" class="sun">
		<circle cx="65" cy="62" r="34" fill="#f2a33a" opacity=".85" />
		<path d="M0 62h130" stroke="#2b2420" stroke-width="1.5" />
	</svg>
	<h2 id="golden-hour-title" class="text-2xl">Golden hour, no clients.</h2>
	<p class="text-muted-foreground">
		The light is perfect and the dashboard is empty.<br />
		Quick, make a gallery before the sun moves.
	</p>
	<p class="text-sm text-muted-foreground">Press “New gallery” above to get started.</p>
</section>

<style>
	.empty {
		background: linear-gradient(180deg, #f7e3c4, var(--background) 75%);
	}
	.sun {
		animation: rise 0.9s ease-out;
	}
	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(12px);
		}
		to {
			opacity: 1;
			transform: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.sun {
			animation: none;
		}
	}
</style>
