<main
	class="grid min-h-dvh content-center justify-items-center gap-2 bg-dark px-4 py-6 text-center text-inverse"
>
	<svg
		class="mb-4 h-auto max-w-full rounded-sm shadow-viewfinder"
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
			<g class="bokeh" fill="#e8c49a" opacity=".35">
				<circle cx="70" cy="60" r="22" />
				<circle cx="270" cy="50" r="28" />
				<circle cx="110" cy="130" r="30" />
				<circle cx="240" cy="125" r="18" />
			</g>
			<path
				d="M113.3 0v210M226.6 0v210M0 70h340M0 140h340"
				stroke="#fff"
				stroke-opacity=".18"
				stroke-width="1"
			/>
			<g class="top" fill="#f5f5f5">
				<text x="12" y="18"><tspan class="af">AF</tspan> ● ONE SHOT</text>
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
	<h1 class="font-serif text-[1.5rem] italic">Focus not locked</h1>
	<p class="max-w-80 text-inverse/70">
		This link no longer works. Ask your photographer for a new one.
	</p>
	<p class="mt-6 font-mono text-xs tracking-[0.25em] text-inverse/50 uppercase">PhotoProofing</p>
</main>

<style>
	/* Scoped child selectors for SVG <text>/<g> artwork and the keyframe animation. */
	text {
		font:
			11px ui-monospace,
			Menlo,
			monospace;
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
	.af {
		animation: blink 1.2s steps(2, jump-none) infinite;
	}
	@keyframes blink {
		50% {
			opacity: 0.25;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.focus,
		.af {
			animation: none;
		}
	}
</style>
