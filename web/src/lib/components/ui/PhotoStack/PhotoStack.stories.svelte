<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import PhotoStack from './PhotoStack.svelte';

	const { Story } = defineMeta({ title: 'UI/PhotoStack', component: PhotoStack });

	const photoIds = ['first', 'second', 'third'];
</script>

<Story name="Stacked" args={{ galleryId: 'storybook', photoIds }} />
<Story name="Completed" args={{ galleryId: 'storybook', photoIds, done: true }} />
