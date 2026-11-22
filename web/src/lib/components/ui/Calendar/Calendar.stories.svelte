<script module lang="ts">
	import { CalendarDate } from '@internationalized/date';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { dateLocale } from '#lib/gallery/format.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import Calendar from './Calendar.svelte';

	const { Story } = defineMeta({ title: 'UI/Calendar', component: Calendar });
</script>

<script lang="ts">
	import type { DateValue } from '@internationalized/date';

	let value = $state<DateValue | undefined>(new CalendarDate(2026, 11, 21));
</script>

<Story name="With minimum date" asChild>
	<div class="inline-block">
		<Calendar
			{value}
			minValue={new CalendarDate(2026, 11, 15)}
			locale={dateLocale()}
			weekStartsOn={getLocale() === 'fr' ? 1 : 0}
			onValueChange={(next) => (value = next)}
		/>
	</div>
</Story>
