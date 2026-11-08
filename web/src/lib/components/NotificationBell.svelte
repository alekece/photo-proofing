<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { Bell } from '@lucide/svelte';
	import Popover from '#lib/components/ui/Popover/Popover.svelte';
	import { previewUrl } from '#lib/gallery/photos.js';
	import {
		notificationText,
		timeAgo,
		type NotificationCenter
	} from '#lib/gallery/notifications.svelte.js';

	let { center, class: className }: { center: NotificationCenter; class?: string } = $props();

	let open = $state(false);
	let unread = $derived(center.items.filter((item) => item.read_at === null));

	function visit(id: string) {
		open = false;
		void center.markRead(id);
	}
</script>

<Popover
	bind:open
	label={center.unread > 0
		? m.notifications_unread({ count: center.unread })
		: m.header_notifications()}
	align="end"
	class="relative {className}"
	panelClass="w-[min(22rem,calc(100vw-1.5rem))] text-foreground"
>
	{#snippet trigger()}
		<Bell size={18} aria-hidden="true" />
		{#if center.unread > 0}
			<span
				aria-hidden="true"
				class="absolute -top-0.5 -right-0.5 grid min-w-4 place-items-center rounded-full bg-danger px-1 text-[0.625rem] leading-4 font-bold text-inverse"
				>{center.unread}</span
			>
		{/if}
	{/snippet}
	<div class="flex items-center justify-between px-3 py-2">
		<span class="text-sm font-semibold">{m.header_notifications()}</span>
		<button
			type="button"
			disabled={center.unread === 0}
			class="min-h-9 cursor-pointer text-sm text-muted-foreground underline disabled:cursor-default disabled:no-underline disabled:opacity-50"
			onclick={() => void center.markAllRead()}>{m.notifications_mark_all_read()}</button
		>
	</div>
	{#if unread.length === 0}
		<p class="px-3 pb-3 text-sm text-muted-foreground">{m.notifications_none()}</p>
	{:else}
		<ul class="max-h-96 overflow-y-auto">
			{#each unread as item (item.id)}
				<li>
					<a
						href={`/dashboard/${item.gallery_id}`}
						class="flex min-h-12 items-center gap-3 rounded-control px-3 py-2 text-foreground no-underline hover:bg-border/60"
						onclick={() => visit(item.id)}
					>
						{#if item.hero_id}
							<img
								src={previewUrl(item.gallery_id, item.hero_id)}
								alt=""
								class="size-10 shrink-0 rounded-md object-cover"
							/>
						{:else}
							<span class="size-10 shrink-0 rounded-md bg-border"></span>
						{/if}
						<span class="min-w-0 flex-1">
							<span class="block truncate text-sm font-semibold"
								>{item.title ?? m.gallery_untitled()}</span
							>
							<span class="block truncate text-xs text-muted-foreground"
								>{notificationText(item)} · {timeAgo(item.updated_at)}</span
							>
						</span>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</Popover>
