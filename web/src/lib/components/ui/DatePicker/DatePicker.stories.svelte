<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import DatePicker from './DatePicker.svelte';

	const { Story } = defineMeta({ title: 'UI/DatePicker', component: DatePicker });
</script>

<script lang="ts">
	let shootDate = $state<string | null>(null);
	let scheduledDate = $state<string | null>('2026-11-21');
</script>

<Story name="Empty and clearable" asChild>
	<div class="grid max-w-xs gap-2">
		<Label for="shoot-date">{m.shoot_date_label()}</Label>
		<DatePicker
			id="shoot-date"
			value={shootDate}
			clearable
			onchange={(next) => (shootDate = next)}
		/>
	</div>
</Story>

<Story name="Selected with presets" asChild>
	<div class="grid max-w-xs gap-2">
		<Label for="scheduled-date">{m.shoot_date_label()}</Label>
		<DatePicker
			id="scheduled-date"
			value={scheduledDate}
			presets
			onchange={(next) => (scheduledDate = next)}
		/>
	</div>
</Story>
