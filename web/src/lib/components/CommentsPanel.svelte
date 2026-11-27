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
		openThread,
		oncount
	}: {
		galleryId: string;
		active?: boolean;
		/** A photo whose thread opens once its comments are listed, as a link from Activity asks. */
		openThread?: { photoId: string };
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

	let handled: object | undefined;

	$effect(() => {
		const request = openThread;

		if (request && request !== handled && commented.some((photo) => photo.id === request.photoId)) {
			handled = request;
			opened.add(request.photoId);
			expanded = request.photoId;
		}
	});

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
	<EmptyState
		variant="plain"
		icon={MessageSquare}
		title={m.comments_tab_empty()}
		description={m.comments_tab_empty_body()}
	/>
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
						<span class="truncate text-sm text-muted-foreground">
							{comments.latestBody(photo.id) ??
								m.notification_comments({ count: comments.count(photo.id) })}
						</span>
					</span>
					<ChevronDown
						size={16}
						aria-hidden="true"
						class="transition-transform duration-200 motion-reduce:transition-none {expanded ===
						photo.id
							? 'rotate-180'
							: ''}"
					/>
				</button>
				<div
					id="{panelId}-{photo.id}"
					inert={expanded !== photo.id}
					aria-hidden={expanded !== photo.id}
					class="grid transition-[grid-template-rows,opacity] duration-250 ease-out motion-reduce:transition-none {expanded ===
					photo.id
						? 'grid-rows-[1fr] opacity-100'
						: 'grid-rows-[0fr] opacity-0'}"
				>
					<div class="min-h-0 overflow-hidden">
						<div class="border-t border-border px-3 py-4 sm:px-5">
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
					</div>
				</div>
			</li>
		{/each}
	</ul>
{/if}
