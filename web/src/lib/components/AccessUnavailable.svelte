<main class="unavailable">
	<svg
		class="viewfinder"
		viewBox="0 0 340 210"
		width="340"
		height="210"
		role="presentation"
		aria-hidden="true"
	>
		<defs>
			<linearGradient id="vf-scene" x1="0" y1="0" x2="1" y2="1">
				<stop offset="0" stop-color="#c9b8a6" />
				<stop offset="1" stop-color="#6b7f8e" />
			</linearGradient>
			<clipPath id="vf-clip"><rect width="340" height="210" rx="4" /></clipPath>
		</defs>
		<g clip-path="url(#vf-clip)">
			<rect width="340" height="210" fill="#111" />
			<rect width="340" height="210" fill="url(#vf-scene)" opacity=".9" />
			<path
				d="M113.3 0v210M226.6 0v210M0 70h340M0 140h340"
				stroke="#fff"
				stroke-opacity=".18"
				stroke-width="1"
			/>
			<g class="top" fill="#f5f5f5">
				<text x="12" y="18">AF ● ONE SHOT</text>
				<text x="328" y="18" text-anchor="end">[ 000 ]</text>
			</g>
			<g class="focus">
				<path
					d="M139 76v-14h14M201 76v-14h-14M139 124v14h14M201 124v14h-14"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
				/>
			</g>
			<rect y="180" width="340" height="30" fill="#0a0a0a" opacity=".78" />
			<g class="info" fill="#f5f5f5">
				<text x="38" y="199">1/250</text>
				<text x="92" y="199">F2.8</text>
				<text x="136" y="199">ISO 100</text>
				<text x="204" y="199">·····0·····</text>
				<text x="278" y="199" fill="#ff9a2e">AF failed</text>
			</g>
		</g>
	</svg>
	<h1>Focus not locked</h1>
	<p>We couldn't lock onto this gallery. Please contact your photographer.</p>
</main>

<style>
	.unavailable {
		display: grid;
		justify-items: center;
		align-content: center;
		gap: 0.5rem;
		min-height: 100vh;
		box-sizing: border-box;
		padding: 1.5rem 1rem;
		text-align: center;
	}
	.viewfinder {
		max-width: 100%;
		height: auto;
		border-radius: 4px;
		box-shadow: var(--shadow-viewfinder);
		margin-bottom: 1rem;
	}
	text {
		font: 11px ui-monospace, Menlo, monospace;
	}
	.top text {
		font-size: 10px;
	}
	.focus {
		color: var(--color-autofocus);
		transform-box: fill-box;
		transform-origin: center;
		animation: hunt 2.4s ease-in-out 1 both;
	}
	@keyframes hunt {
		0% {
			transform: translate(-60px, 12px) scale(1.25);
			color: var(--color-inverse);
		}
		18% {
			transform: translate(40px, -14px) scale(0.9);
		}
		34% {
			transform: translate(-20px, -10px) scale(1.1);
			color: var(--color-inverse);
		}
		55%,
		100% {
			transform: none;
			color: var(--color-autofocus);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.focus {
			animation: none;
		}
	}
	h1 {
		margin: 0;
		font-size: 1.5rem;
	}
	p {
		margin: 0;
		max-width: 20rem;
		color: var(--muted-foreground);
	}
</style>
