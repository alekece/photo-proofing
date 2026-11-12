<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { galleryInitials } from '#lib/gallery/format.js';

	/**
	 * The warm film-leader stand-in behind a cover without a hero photo. It fills the nearest
	 * positioned ancestor; `centered` is the client cover, where the title sits in the middle;
	 * `compact` scales initials for small studio thumbnails and omits the film-edge date strip.
	 */
	let {
		title,
		edge = null,
		centered = false,
		compact = false
	}: {
		title: string | null;
		/** The film edge print, e.g. the date; the line is left out without it or in `compact` mode. */
		edge?: string | null;
		centered?: boolean;
		compact?: boolean;
	} = $props();

	const initials = $derived(galleryInitials(compact ? (title ?? m.gallery_untitled()) : title));
</script>

<div
	data-testid="no-hero"
	class="absolute inset-0 {compact
		? 'z-0'
		: '-z-20'} bg-[radial-gradient(55%_80%_at_82%_10%,rgb(214_105_63/0.22),transparent_70%),radial-gradient(80%_120%_at_20%_100%,#3a2a20,transparent_70%),linear-gradient(160deg,#2b211b,#16120f_70%)]"
></div>
<div aria-hidden="true" class="absolute inset-0 {compact ? 'z-0' : '-z-10'} overflow-hidden">
	<span
		class="absolute top-1/2 font-serif leading-none tracking-tight whitespace-nowrap select-none {compact
			? 'right-1/2 translate-x-1/2 -translate-y-1/2 text-base text-panel/25 italic md:text-3xl'
			: centered
				? 'right-1/2 translate-x-1/2 -translate-y-[62%] text-[clamp(9.375rem,24vw,20.625rem)] text-panel/[0.07] italic'
				: 'right-[4%] -translate-y-1/2 text-[clamp(9.375rem,24vw,20.625rem)] text-panel/[0.07] italic'}"
		>{initials}</span
	>
	{#if edge && !compact}
		<span
			class="absolute truncate font-mono text-[0.5625rem] tracking-[0.2em] text-[#d9792e]/80 uppercase sm:text-[0.625rem] {centered
				? 'inset-x-[1.125rem] top-[5.25rem] text-center sm:inset-x-8 sm:top-[6.25rem]'
				: 'inset-x-[1.125rem] top-[4.125rem] sm:inset-x-8 sm:top-[4.875rem]'}"
		>
			<span
				class="mb-2 block h-1.5 w-[min(26rem,100%)] bg-[repeating-linear-gradient(90deg,rgb(251_248_243/0.14)_0_8px,transparent_8px_18px)] {centered
					? 'mx-auto'
					: ''}"
			></span>
			▸&nbsp;&nbsp;{edge}
		</span>
	{/if}
	<span
		class="absolute -inset-x-[40%] -inset-y-[20%] animate-leak bg-[linear-gradient(105deg,transparent_40%,rgb(232_110_50/0.16)_48%,rgb(255_206_140/0.12)_52%,transparent_60%)] motion-reduce:hidden"
	></span>
</div>
