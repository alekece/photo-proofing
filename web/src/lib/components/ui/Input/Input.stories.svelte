<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { m } from '#lib/paraglide/messages.js';
	import Label from '../Label/Label.svelte';
	import Input from './Input.svelte';

	const { Story } = defineMeta({ title: 'UI/Input', component: Input });
</script>

<Story name="Text" asChild>
	<div class="grid max-w-xs gap-2">
		<Label for="gallery-title">{m.gallery_title_label()}</Label>
		<Input id="gallery-title" type="text" placeholder={m.new_gallery_placeholder()} />
	</div>
</Story>

<Story name="Disabled" asChild>
	<div class="grid max-w-xs gap-2">
		<Label for="gallery-title">{m.gallery_title_label()}</Label>
		<Input id="gallery-title" type="text" value={m.gallery_untitled()} disabled />
	</div>
</Story>

<Story name="File" asChild>
	<div class="grid max-w-xs gap-2">
		<Label for="photos">{m.photos_choose()}</Label>
		<Input id="photos" type="file" accept="image/*" multiple />
	</div>
</Story>
