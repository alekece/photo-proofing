<script lang="ts">
	import { page } from '$app/state';
	import { m } from '#lib/paraglide/messages.js';
	import { Bell, Inbox, LoaderCircle } from '@lucide/svelte';
	import Button from '#lib/components/ui/Button/Button.svelte';
	import CloseButton from '#lib/components/ui/CloseButton/CloseButton.svelte';
	import EmptyState from '#lib/components/ui/EmptyState/EmptyState.svelte';
	import { toasts } from '#lib/components/ui/Toast/toasts.svelte.js';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { previewUrl } from '#lib/gallery/photos.js';
	import {
		notificationText,
		timeAgo,
		type Notification,
		type NotificationCenter
	} from '#lib/gallery/notifications.svelte.js';

	let {
		center,
		class: className,
		row = false
	}: {
		center: NotificationCenter;
		class?: string;
		/** A labelled navigation row, anchored to the bottom of the workspace rail, instead of an icon. */
		row?: boolean;
	} = $props();

	let open = $state(false);
	let deleting = $state(false);

	async function deleteAll() {
		if (deleting) return;

		deleting = true;
		try {
			if (await center.deleteAll()) open = false;
			else toasts.error(m.notifications_delete_failed());
		} finally {
			deleting = false;
		}
	}

	const SECTIONS = { selection: 'favorites', download: 'final', comment: 'comments' } as const;

	/** The tab of the gallery that holds what the notification reports. */
	function destination(item: Notification): string {
		const gallery = `/dashboard/${encodeURIComponent(item.gallery_id)}`;
		const section = item.kind === 'open' ? undefined : SECTIONS[item.kind];

		return section ? `${gallery}?section=${section}` : gallery;
	}

	function visit(event: MouseEvent, galleryId: string) {
		open = false;
		if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
			return;

		// Other destinations mark on entry; this editor stays mounted.
		if (page.route.id === '/dashboard/[id]' && page.params.id === galleryId && !page.error) {
			void center.markGalleryRead(galleryId);
		}
	}
</script>

<Popover
	bind:open
	label={center.unread > 0
		? m.notifications_unread({ count: center.unread })
		: m.header_notifications()}
	align="end"
	placement="right"
	aria-haspopup={false}
	class={row ? `cursor-pointer ${className}` : `relative sm:icon-action ${className}`}
	rootClass={row ? 'block w-full' : undefined}
	panelClass="fixed top-14 right-3 left-3 ml-0 flex max-h-[calc(100dvh-4.25rem)] w-auto flex-col overflow-hidden p-0 text-foreground motion-reduce:animate-none sm:right-auto sm:ml-2 sm:w-[24rem] {row
		? 'sm:top-auto sm:bottom-3 sm:left-(--workspace-rail-width) sm:max-h-[calc(100dvh-1.5rem)]'
		: 'sm:absolute sm:top-0 sm:left-full sm:max-h-[calc(100dvh-2rem)]'}"
>
	{#snippet trigger()}
		{#if row}
			<span aria-hidden="true" class="relative grid shrink-0 place-items-center">
				<Bell size={16} />
				{#if center.unread > 0}
					<span
						class="absolute -top-1 -right-1 hidden size-2 rounded-full bg-danger workspace-collapsed:block"
					></span>
				{/if}
			</span>
			<span aria-hidden="true" class="workspace-collapsed:sr-only">{m.header_notifications()}</span>
			{#if center.unread > 0}
				<span
					aria-hidden="true"
					class="ms-auto grid min-w-5 place-items-center rounded-full bg-danger px-1.5 text-[0.6875rem] leading-5 font-bold text-inverse workspace-collapsed:hidden"
				>
					{center.unread}
				</span>
			{/if}
		{:else}
			<span aria-hidden="true" class="sm:relative">
				<Bell size={18} />
				{#if center.unread > 0}
					<span
						class="absolute -top-0.5 -right-0.5 grid min-w-4 place-items-center rounded-full bg-danger px-1 text-[0.625rem] leading-4 font-bold text-inverse sm:top-0 sm:right-auto sm:left-3 sm:-translate-y-1/2"
					>
						{center.unread}
					</span>
				{/if}
			</span>
		{/if}
	{/snippet}
	{#snippet children({ close })}
		<div class="flex shrink-0 items-center gap-2 border-b border-border py-1 pr-1 pl-4">
			<h2 class="mr-auto font-serif text-lg">{m.header_notifications()}</h2>
			<CloseButton onclick={close} />
		</div>
		{#if center.items.length === 0}
			<EmptyState
				variant="plain"
				icon={Inbox}
				title={m.notifications_none()}
				description={m.notifications_none_body()}
				class="min-h-0 gap-2 rounded-none border-0 bg-transparent p-8 sm:min-h-0 [&_h3]:text-xl sm:[&_h3]:text-xl"
			/>
		{:else}
			<ul class="max-h-96 min-h-0 divide-y divide-border overflow-y-auto">
				{#each center.items as item (item.id)}
					{@const unread = item.read_at === null}
					<li>
						<a
							href={destination(item)}
							class="relative flex items-center gap-3 px-4 py-3 text-foreground no-underline focus-visible:-outline-offset-2"
							onclick={(event) => visit(event, item.gallery_id)}
						>
							{#if item.hero_id}
								<img
									src={previewUrl(item.gallery_id, item.hero_id)}
									alt=""
									class="size-11 shrink-0 rounded-md object-cover transition-opacity {unread
										? ''
										: 'opacity-50 grayscale'}"
								/>
							{:else}
								<span class="size-11 shrink-0 rounded-md bg-border {unread ? '' : 'opacity-50'}"
								></span>
							{/if}
							<span class="min-w-0 flex-1">
								<span
									class="block text-sm wrap-anywhere {unread
										? 'font-bold text-foreground'
										: 'font-medium text-muted-foreground'}"
								>
									{item.title ?? m.gallery_untitled()}
								</span>
								<span
									class="block text-xs {unread
										? 'font-semibold text-foreground'
										: 'text-muted-foreground'}"
								>
									{notificationText(item)} · {timeAgo(item.updated_at)}
								</span>
								{#if unread}<span class="sr-only">{m.card_unread_activity()}</span>{/if}
							</span>
						</a>
					</li>
				{/each}
			</ul>
			<div class="flex shrink-0 justify-end gap-2 border-t border-border px-3 py-2">
				<Button
					variant="outline"
					size="sm"
					class="min-h-8! px-2.5 text-xs"
					disabled={center.unread === 0 || deleting}
					onclick={() => void center.markAllRead()}
				>
					{m.notifications_mark_all_read()}
				</Button>
				<Button
					variant="destructive"
					size="sm"
					class="relative min-h-8! px-2.5 text-xs"
					disabled={deleting}
					aria-busy={deleting}
					onclick={() => void deleteAll()}
				>
					<span class={deleting ? 'invisible' : ''}>{m.notifications_delete_all()}</span>
					{#if deleting}
						<LoaderCircle
							size={14}
							class="absolute animate-spin motion-reduce:animate-none"
							aria-hidden="true"
						/>
					{/if}
				</Button>
			</div>
		{/if}
	{/snippet}
</Popover>
