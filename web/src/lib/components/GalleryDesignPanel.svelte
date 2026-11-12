<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import LayoutRowsIcon from '#lib/components/icons/LayoutRowsIcon.svelte';
	import LayoutStaggeredIcon from '#lib/components/icons/LayoutStaggeredIcon.svelte';
	import Card from '#lib/components/ui/Card/Card.svelte';
	import IconButtonGroup from '#lib/components/ui/IconButtonGroup/IconButtonGroup.svelte';
	import type { GalleryChanges, GalleryLayout } from '#lib/gallery/api.js';

	/**
	 * Studio `Design` view: client-facing visual presentation settings (`Layout` today; cover
	 * templates, focal point, typography, grid density, and spacing plug in here next).
	 */
	let {
		layout,
		onsave
	}: {
		layout: GalleryLayout;
		onsave: (changes: GalleryChanges) => Promise<boolean>;
	} = $props();

	const savedLayout = $derived(layout);
	let current = $derived<GalleryLayout>(savedLayout);

	const layouts = [
		{ id: 'masonry', label: m.settings_layout_masonry(), icon: LayoutStaggeredIcon },
		{ id: 'grid', label: m.settings_layout_grid(), icon: LayoutRowsIcon }
	];

	async function setLayout(value: string) {
		const previous = current;
		const next = value as GalleryLayout;

		current = next;

		if (!(await onsave({ layout: next }))) {
			current = previous;
		}
	}
</script>

<div class="grid max-w-xl gap-4">
	<h2 class="text-2xl leading-tight">{m.tab_design()}</h2>

	<Card as="section" class="rounded-[0.875rem] border border-border bg-panel p-4 sm:p-5">
		<h3
			class="mb-2.5 font-sans! text-[0.6875rem] leading-none font-semibold! tracking-[0.08em] text-muted-foreground uppercase"
		>
			{m.settings_look()}
		</h3>
		<div class="flex items-center justify-between gap-2.5 pt-0.5 text-sm">
			<span class="text-foreground">{m.settings_layout()}</span>
			<IconButtonGroup
				value={current}
				items={layouts}
				label={m.settings_layout()}
				onchange={(value) => void setLayout(value)}
			/>
		</div>
	</Card>
</div>
