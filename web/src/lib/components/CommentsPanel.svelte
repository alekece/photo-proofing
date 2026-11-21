<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { CommentsClient } from '#lib/gallery/comments.js';
	import { PhotoComments } from '#lib/gallery/photoComments.svelte.js';
	import CommentThread from './CommentThread.svelte';
	import { ChevronDown, MessageSquare } from '@lucide/svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import { getLiveEvents } from '#lib/events.js';
	import { listPhotos, thumbnailUrl, type Photo } from '#lib/gallery/photos.js';

	let {
		galleryId,
		active = true,
		oncount
	}: {
		galleryId: string;
		active?: boolean;
		/** How many photos have comments, for the tab. */
		oncount?: (count: number | null) => void;
	} = $props();

	const events = getLiveEvents();
	const panelId = $props.id();
	const comments = $derived(
		new PhotoComments(new CommentsClient(`/api/v1/galleries/${encodeURIComponent(galleryId)}`))
	);
	$effect(() => comments.start(events, galleryId));

	let photos = $state<Photo[]>([]);
	let loaded = $state(false);
	let failed = $state(false);
	let loads = 0;
	let expanded = $state<string>();
	const opened = new SvelteSet<string>();

	function toggle(id: string) {
		opened.add(id);
		expanded = expanded === id ? undefined : id;
	}

	async function load(id: string) {
		const current = ++loads;

		try {
			const all = await listPhotos(id);

			if (current !== loads) return;

			photos = all;
			failed = false;
		} catch {
			if (current === loads) failed = true;
		} finally {
			if (current === loads) loaded = true;
		}
	}

	$effect(() => {
		const id = galleryId;

		photos = [];
		loaded = false;
		failed = false;
		expanded = undefined;
		opened.clear();
		void load(id);

		const unsubscribe = [
			events.onPhoto(
				(event) =>
					event.gallery_id === id &&
					(event.status === 'deleted' || event.status === 'ready') &&
					void load(id)
			),
			events.onResync(() => void load(id))
		];

		return () => {
			loads++;

			for (const off of unsubscribe) off();
		};
	});

	const commented = $derived(photos.filter(({ id }) => comments.count(id) > 0));

	$effect(() => {
		oncount?.(loaded && !failed && !comments.error ? commented.length : null);
	});
</script>

{#if failed}
	<p role="alert" class="text-sm text-danger">{m.comments_tab_load_failed()}</p>
{:else if comments.error}
	<p role="alert" class="text-sm text-danger">{comments.error}</p>
{/if}

{#if loaded && !failed && !comments.error && commented.length === 0}
	<EmptyState variant="plain">
		<span class="grid size-14 place-items-center rounded-full bg-background"
			><MessageSquare size={26} aria-hidden="true" /></span
		>
		<h3>{m.comments_tab_empty()}</h3>
		<p class="text-sm">{m.comments_tab_empty_body()}</p>
	</EmptyState>
{:else if commented.length > 0}
	<ul
		aria-label={m.tab_comments()}
		class="divide-y divide-border rounded-card border border-border bg-panel"
	>
		{#each commented as photo (photo.id)}
			<li>
				<button
					type="button"
					aria-expanded={expanded === photo.id}
					aria-controls="{panelId}-{photo.id}"
					onclick={() => toggle(photo.id)}
					class="flex w-full cursor-pointer items-center gap-3 px-3 py-2.5 text-left hover:bg-background/70 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus sm:px-4"
				>
					<img
						src={thumbnailUrl(galleryId, photo.id, 'photos', photo.revision)}
						alt=""
						loading="lazy"
						class="size-12 shrink-0 rounded-control object-cover"
					/>
					<span class="grid min-w-0 flex-1 gap-0.5">
						<strong class="truncate text-sm">{photo.original_name}</strong>
						<span class="truncate text-sm text-muted-foreground"
							>{comments.latestBody(photo.id) ??
								m.notification_comments({ count: comments.count(photo.id) })}</span
						>
					</span>
					<ChevronDown
						size={16}
						aria-hidden="true"
						class={expanded === photo.id ? 'rotate-180' : ''}
					/>
				</button>
				<div
					id="{panelId}-{photo.id}"
					hidden={expanded !== photo.id}
					class="border-t border-border px-3 py-4 sm:px-5"
				>
					{#if opened.has(photo.id)}
						<CommentThread
							client={comments.client}
							photoId={photo.id}
							viewer="photographer"
							inline
							active={active && expanded === photo.id}
							refresh={comments.revision}
							onchange={() => void comments.refresh()}
						/>
					{/if}
				</div>
			</li>
		{/each}
	</ul>
{/if}
