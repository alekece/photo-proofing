<script lang="ts">
	import CalendarDays from '@lucide/svelte/icons/calendar-days';
	import { parseDate, type DateValue } from '@internationalized/date';
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
		onchange,
		class: className,
		'aria-describedby': ariaDescribedby
	}: {
		id: string;
		value: string | null;
		min?: string;
		placeholder?: string;
		clearable?: boolean;
		onchange: (value: string | null) => void;
		class?: string;
		'aria-describedby'?: string;
	} = $props();

	let open = $state(false);

	const selected = $derived(value ? parseDate(value) : undefined);
	const minValue = $derived(min ? parseDate(min) : undefined);

	function pick(day: DateValue | undefined) {
		open = false;

		if (day && day.toString() !== value) onchange(day.toString());
	}

	function clear() {
		open = false;
		onchange(null);
	}
</script>

<Popover
	{id}
	bind:open
	aria-describedby={ariaDescribedby}
	class={cn(
		'cn-input inline-flex items-center gap-2 text-left focus-visible:border-foreground',
		className
	)}
	panelClass="min-w-0"
>
	{#snippet trigger()}
		<CalendarDays size={16} class="shrink-0 text-muted-foreground" aria-hidden="true" />
		<span class={cn('truncate', !value && 'text-muted-foreground')}>
			{value ? formatFullDate(value, 'UTC') : placeholder}
		</span>
	{/snippet}
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
</Popover>
