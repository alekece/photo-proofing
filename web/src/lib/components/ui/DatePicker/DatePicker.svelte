<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import {
		getLocalTimeZone,
		parseDate,
		today,
		type DateDuration,
		type DateValue
	} from '@internationalized/date';
	import Calendar from '#lib/components/ui/Calendar/Calendar.svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { dateLocale, formatFullDate } from '#lib/gallery/format.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { cn } from '#lib/utils.js';

	/**
	 * A field-styled button that opens the app calendar. Days travel as `YYYY-MM-DD` strings and
	 * never through a JS `Date`, so the picked day is the stored day in every time zone.
	 */
	let {
		id,
		value,
		min,
		placeholder = '',
		clearable = false,
		presets = false,
		align,
		onchange,
		class: className,
		rootClass,
		'aria-describedby': ariaDescribedby
	}: {
		id: string;
		value: string | null;
		min?: string;
		placeholder?: string;
		clearable?: boolean;
		presets?: boolean;
		align?: 'start' | 'end';
		onchange: (value: string | null) => void;
		class?: string;
		rootClass?: string;
		'aria-describedby'?: string;
	} = $props();

	let open = $state(false);

	const selected = $derived(value ? parseDate(value) : undefined);
	const minValue = $derived(min ? parseDate(min) : undefined);

	const relativePresets: { label: () => string; duration: DateDuration }[] = [
		{ label: () => m.date_preset_1_week(), duration: { weeks: 1 } },
		{ label: () => m.date_preset_2_weeks(), duration: { weeks: 2 } },
		{ label: () => m.date_preset_1_month(), duration: { months: 1 } },
		{ label: () => m.date_preset_2_months(), duration: { months: 2 } },
		{ label: () => m.date_preset_6_months(), duration: { months: 6 } },
		{ label: () => m.date_preset_1_year(), duration: { years: 1 } }
	];

	function pick(day: DateValue | undefined) {
		open = false;

		if (day && day.toString() !== value) onchange(day.toString());
	}

	function pickPreset(duration: DateDuration) {
		pick(today(getLocalTimeZone()).add(duration));
	}

	function clear() {
		open = false;
		onchange(null);
	}
</script>

<Popover
	{id}
	bind:open
	{align}
	aria-describedby={ariaDescribedby}
	class={cn(
		'cn-input inline-flex items-center gap-2 text-left focus-visible:border-foreground',
		className
	)}
	{rootClass}
	panelClass="min-w-0 max-w-[calc(100vw-2rem)] max-h-[min(28rem,calc(100dvh-2rem))] overflow-y-auto bg-white"
>
	{#snippet trigger()}
		<CalendarDays size={16} class="shrink-0 text-muted-foreground" aria-hidden="true" />
		<span class={cn('min-w-0 truncate', !value && 'text-muted-foreground/40')}>
			{value ? formatFullDate(value, 'UTC', 'medium') : placeholder}
		</span>
	{/snippet}
	<div class={cn(presets && 'flex flex-col sm:flex-row')}>
		{#if presets}
			<div
				class="grid grid-cols-2 gap-1 border-b border-border p-1.5 sm:flex sm:w-36 sm:shrink-0 sm:flex-col sm:border-r sm:border-b-0"
			>
				{#each relativePresets as preset (preset.label())}
					<button
						type="button"
						class="rounded-control px-2.5 py-1.5 text-left text-xs font-medium whitespace-nowrap text-foreground hover:bg-foreground/5 focus-visible:outline-2 focus-visible:outline-focus sm:text-sm"
						onclick={() => pickPreset(preset.duration)}
					>
						{preset.label()}
					</button>
				{/each}
			</div>
		{/if}
		<div>
			<Calendar
				value={selected}
				onValueChange={pick}
				{minValue}
				locale={dateLocale()}
				weekStartsOn={getLocale() === 'fr' ? 1 : 0}
			/>
			{#if clearable && value}
				<div class="flex justify-end border-t border-border px-2 pt-1 pb-1">
					<button
						type="button"
						class="rounded-control px-2 py-1 text-sm text-muted-foreground hover:bg-foreground/5 hover:text-foreground"
						onclick={clear}
					>
						{m.date_clear()}
					</button>
				</div>
			{/if}
		</div>
	</div>
</Popover>
