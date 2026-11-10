<script lang="ts">
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { DateValue } from '@internationalized/date';
	import { Calendar as CalendarPrimitive } from 'bits-ui';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	/**
	 * A single-month calendar from shadcn-svelte, on Bits UI, restyled with the app's tokens. The
	 * caller owns the language: `locale` names the months and days, `weekStartsOn` sets the first
	 * column.
	 */
	let {
		value,
		onValueChange,
		minValue,
		locale,
		weekStartsOn,
		class: className
	}: {
		value: DateValue | undefined;
		onValueChange: (value: DateValue | undefined) => void;
		minValue?: DateValue;
		locale: string;
		weekStartsOn: 0 | 1;
		class?: string;
	} = $props();

	const navButton =
		'inline-flex size-8 items-center justify-center rounded-control text-muted-foreground hover:bg-foreground/5 hover:text-foreground focus-visible:outline-2 focus-visible:outline-focus data-disabled:pointer-events-none data-disabled:opacity-40';
</script>

<CalendarPrimitive.Root
	type="single"
	{value}
	{onValueChange}
	{minValue}
	{locale}
	{weekStartsOn}
	weekdayFormat="short"
	fixedWeeks
	class={cn('p-2', className)}
>
	{#snippet children({ months, weekdays })}
		<CalendarPrimitive.Header>
			{#snippet child({ props })}
				<!-- A div, not Bits' <header>, which would add a banner landmark inside the page. -->
				<div {...props} class="flex items-center justify-between gap-2 pb-2">
					{@render nav()}
				</div>
			{/snippet}
		</CalendarPrimitive.Header>
		{#snippet nav()}
			<CalendarPrimitive.PrevButton class={navButton} aria-label={m.date_previous_month()}>
				<ChevronLeft size={16} aria-hidden="true" />
			</CalendarPrimitive.PrevButton>
			<CalendarPrimitive.Heading class="text-sm font-semibold capitalize" />
			<CalendarPrimitive.NextButton class={navButton} aria-label={m.date_next_month()}>
				<ChevronRight size={16} aria-hidden="true" />
			</CalendarPrimitive.NextButton>
		{/snippet}
		{#each months as month (month.value)}
			<CalendarPrimitive.Grid class="w-full border-collapse select-none">
				<CalendarPrimitive.GridHead>
					<CalendarPrimitive.GridRow class="flex">
						{#each weekdays as weekday, index (index)}
							<CalendarPrimitive.HeadCell
								class="w-9 text-center text-xs font-normal text-muted-foreground"
							>
								{weekday.slice(0, 2)}
							</CalendarPrimitive.HeadCell>
						{/each}
					</CalendarPrimitive.GridRow>
				</CalendarPrimitive.GridHead>
				<CalendarPrimitive.GridBody>
					{#each month.weeks as week (week)}
						<CalendarPrimitive.GridRow class="mt-1 flex">
							{#each week as date (date)}
								<CalendarPrimitive.Cell {date} month={month.value} class="size-9 p-0 text-center">
									<CalendarPrimitive.Day
										class="inline-flex size-9 items-center justify-center rounded-control text-sm text-foreground tabular-nums hover:bg-foreground/5 focus-visible:outline-2 focus-visible:outline-focus data-disabled:pointer-events-none data-disabled:opacity-35 data-outside-month:text-muted-foreground data-outside-month:opacity-60 data-selected:bg-foreground data-selected:font-semibold data-selected:text-background data-today:not-data-selected:font-semibold data-today:not-data-selected:underline data-today:not-data-selected:underline-offset-4"
									/>
								</CalendarPrimitive.Cell>
							{/each}
						</CalendarPrimitive.GridRow>
					{/each}
				</CalendarPrimitive.GridBody>
			</CalendarPrimitive.Grid>
		{/each}
	{/snippet}
</CalendarPrimitive.Root>
