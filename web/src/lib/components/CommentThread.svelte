<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { CommentsClient } from '#lib/gallery/comments.js';
	import type { Comment } from '#lib/gallery/schemas.js';
	import { formatDateTime } from '#lib/gallery/format.js';
	import ConfirmDialog from './ui/ConfirmDialog/ConfirmDialog.svelte';
	import ViewerButton from './ViewerButton.svelte';

	let {
		client,
		photoId,
		viewer,
		onchange,
		refresh = 0
	}: {
		client: CommentsClient;
		photoId: string;
		viewer: 'client' | 'photographer';
		onchange?: () => void;
		refresh?: number;
	} = $props();

	const id = $props.id();
	let comments = $state<Comment[]>([]);
	let loading = $state(true);
	let pending = $state(false);
	let error = $state('');
	let draft = $state('');
	let editing = $state<string | null>(null);
	let deleting = $state<string | null>(null);
	const length = $derived([...draft].length);
	const valid = $derived(draft.trim().length > 0 && length <= 1000);

	$effect(() => {
		const current = photoId;
		void refresh;
		let active = true;
		loading = true;
		client
			.thread(current)
			.then((thread) => {
				if (active) {
					comments = thread;
					error = '';
				}
			})
			.catch((cause) => {
				if (active) error = cause instanceof Error ? cause.message : m.api_error_generic();
			})
			.finally(() => {
				if (active) loading = false;
			});
		return () => {
			active = false;
		};
	});

	async function send() {
		if (pending || !valid) return;
		pending = true;
		error = '';
		try {
			const comment = editing
				? await client.edit(editing, draft)
				: await client.add(photoId, draft);
			comments = editing
				? comments.map((item) => (item.id === comment.id ? comment : item))
				: [...comments, comment];
			draft = '';
			editing = null;
			onchange?.();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : m.api_error_generic();
		} finally {
			pending = false;
		}
	}

	async function remove() {
		if (!deleting) return;
		await client.remove(deleting);
		comments = await client.thread(photoId);
		onchange?.();
	}
</script>

<section class="flex h-full min-h-0 flex-col gap-4 text-panel" aria-label={m.comments_title()}>
	<h2 class="font-serif text-xl">{m.comments_title()}</h2>
	<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
		{#if loading}<p role="status" class="text-sm text-panel/75">{m.comments_loading()}</p>{/if}
		{#if !loading && comments.length === 0}<p class="text-sm text-panel/75">
				{m.comments_empty()}
			</p>{/if}
		<ol class="space-y-5">
			{#each comments as comment (comment.id)}
				<li class="border-b border-panel/15 pb-4">
					<div class="mb-1 flex flex-wrap items-baseline gap-x-2 gap-y-1 text-xs text-panel/75">
						<strong class="font-semibold text-panel"
							>{comment.author === viewer
								? m.comments_you()
								: comment.author === 'client'
									? m.comments_client()
									: m.comments_photographer()}</strong
						>
						<time datetime={comment.created_at}>{formatDateTime(comment.created_at)}</time>
						{#if comment.edited_at && !comment.deleted_at}<span>{m.comments_edited()}</span>{/if}
					</div>
					<p
						class="wrap-anywhere whitespace-pre-wrap {comment.deleted_at
							? 'text-panel/65'
							: ''} {comment.deleted_at && comment.body !== null ? 'line-through' : ''}"
					>
						{comment.body ?? m.comments_deleted()}
					</p>
					{#if comment.author === viewer && !comment.deleted_at}
						<div class="mt-1 flex gap-1">
							<ViewerButton
								disabled={pending}
								onclick={() => {
									editing = comment.id;
									draft = comment.body ?? '';
								}}>{m.comments_edit()}</ViewerButton
							>
							<ViewerButton disabled={pending} tone="warn" onclick={() => (deleting = comment.id)}
								>{m.comments_delete()}</ViewerButton
							>
						</div>
					{/if}
				</li>
			{/each}
		</ol>
	</div>
	{#if error}<p role="alert" class="text-sm text-warning-on-dark">{error}</p>{/if}
	<form
		class="grid gap-2"
		onsubmit={(event) => {
			event.preventDefault();
			void send();
		}}
	>
		<label for="{id}-body" class="text-sm"
			>{editing
				? m.comments_edit()
				: viewer === 'client'
					? m.comments_add()
					: m.comments_reply()}</label
		>
		<textarea
			id="{id}-body"
			bind:value={draft}
			disabled={pending || loading}
			rows="3"
			class="max-h-40 min-h-20 w-full resize-y rounded-md border border-panel/30 bg-dark p-3 text-base text-panel focus-visible:outline-2 focus-visible:outline-viewer-focus disabled:opacity-50"
		></textarea>
		<div class="flex flex-wrap items-center justify-end gap-2">
			<div class="flex gap-2">
				{#if editing}<ViewerButton
						disabled={pending}
						onclick={() => {
							editing = null;
							draft = '';
						}}>{m.common_cancel()}</ViewerButton
					>{/if}
				<ViewerButton
					type="submit"
					disabled={pending || loading || !valid}
					class="bg-panel text-foreground hover:bg-panel/90"
					>{pending
						? m.comments_saving()
						: editing
							? m.comments_save()
							: m.comments_send()}</ViewerButton
				>
			</div>
		</div>
	</form>
</section>

{#if deleting}
	<ConfirmDialog
		title={m.comments_delete_question()}
		confirmLabel={m.comments_delete_confirm()}
		destructive
		onconfirm={remove}
		onclose={() => (deleting = null)}
	/>
{/if}
