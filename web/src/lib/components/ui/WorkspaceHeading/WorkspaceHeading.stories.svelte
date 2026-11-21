<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import WorkspaceHeading from './WorkspaceHeading.svelte';

	const { Story } = defineMeta({ title: 'UI/WorkspaceHeading', component: WorkspaceHeading });
</script>

<Story name="With subtitle" asChild>
	<WorkspaceHeading title={m.activity_title()} subtitle={m.activity_subtitle()} />
</Story>

<Story name="With count" asChild>
	<WorkspaceHeading
		title={m.contacts_title()}
		count={new Intl.NumberFormat(getLocale()).format(12)}
	/>
</Story>
