<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { WorkspaceSettings } from '#lib/settings/settings.js';

	let { settings }: { settings: WorkspaceSettings } = $props();

	const SCALES = [1.4, 1.6, 1.9, 2.2, 2.5, 2.85, 3.2, 3.6, 4, 4.8];
	const ALIGN = ['start', 'center', 'end'];
	const angle = -24;
	const radians = (angle * Math.PI) / 180;
	const sin = Math.sin(radians);
	const cos = Math.cos(radians);
	const showLogo = $derived(settings.mark !== 'name' && settings.logo !== null);
	const showName = $derived(settings.mark !== 'logo' || settings.logo === null);
	const name = $derived(settings.studioName.trim() || m.watermark_your_studio());
	const scale = $derived(SCALES[Math.min(settings.size, SCALES.length - 1)]);
	const opacity = $derived(settings.watermarkOpacity / 100);
	let width = $state(0);
	let height = $state(0);
	let stampWidth = $state(0);
	let stampHeight = $state(0);

	const tiles = $derived.by(() => {
		if (!width || !height || !stampWidth || !stampHeight) return [];

		const stepX = stampWidth * 1.6;
		const stepY = stampHeight * 2.2;
		const paddingX = (stampWidth * cos + stampHeight * Math.abs(sin)) / 2;
		const paddingY = (stampWidth * Math.abs(sin) + stampHeight * cos) / 2;
		const halfWidth = width / 2 + paddingX;
		const halfHeight = height / 2 + paddingY;
		// Project the padded frame onto the rotated lattice axes.
		const columns = Math.ceil((halfWidth * cos + halfHeight * Math.abs(sin)) / stepX);
		const rows = Math.ceil((halfWidth * Math.abs(sin) + halfHeight * cos) / stepY);
		const visible: { id: string; x: number; y: number }[] = [];

		for (let row = -rows; row <= rows; row++) {
			const dy = row * stepY;
			const shift = row % 2 !== 0 ? 0.5 : 0;

			for (let column = -columns; column <= columns; column++) {
				const dx = (column + shift) * stepX;
				const x = dx * cos - dy * sin;
				const y = dx * sin + dy * cos;

				if (Math.abs(x) < halfWidth && Math.abs(y) < halfHeight) {
					visible.push({ id: `${row}:${column}`, x: x + width / 2, y: y + height / 2 });
				}
			}
		}

		return visible;
	});
</script>

{#snippet stamp()}
	<span class="inline-flex items-center gap-[0.45em] font-serif whitespace-nowrap italic">
		{#if showLogo}<img src={settings.logo} alt="" class="size-[1.6em] object-contain" />{/if}
		{#if showName}
			<span
				class="[text-shadow:0_1px_4px_rgb(0_0_0/0.35)]"
				style:color={settings.watermarkTextColor}
			>
				{name}
			</span>
		{/if}
	</span>
{/snippet}

{#if settings.pattern === 'repeat'}
	<div
		class="pointer-events-none absolute inset-0"
		style:font-size="{scale}cqw"
		style:opacity
		bind:clientWidth={width}
		bind:clientHeight={height}
	>
		<span
			class="invisible absolute inline-flex w-max"
			bind:offsetWidth={stampWidth}
			bind:offsetHeight={stampHeight}
		>
			{@render stamp()}
		</span>
		{#each tiles as { id, x, y } (id)}
			<span
				class="absolute inline-flex w-max"
				style:left="{x}px"
				style:top="{y}px"
				style:transform="translate(-50%, -50%) rotate({angle}deg)"
			>
				{@render stamp()}
			</span>
		{/each}
	</div>
{:else}
	<div class="absolute inset-0 grid p-[5cqw]" style:font-size="{scale}cqw" style:opacity>
		<div
			style:align-self={ALIGN[Math.floor(settings.position / 3)]}
			style:justify-self={ALIGN[settings.position % 3]}
		>
			{@render stamp()}
		</div>
	</div>
{/if}
