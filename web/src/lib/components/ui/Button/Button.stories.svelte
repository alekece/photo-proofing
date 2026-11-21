<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Button from './Button.svelte';

	const { Story } = defineMeta({ title: 'UI/Button', component: Button });
</script>

<Story name="Default" asChild>
	<Button>{m.comments_save()}</Button>
</Story>

<Story name="Destructive" asChild>
	<Button variant="destructive">{m.action_delete()}</Button>
</Story>

<Story name="Disabled" asChild>
	<Button disabled>{m.comments_save()}</Button>
</Story>
