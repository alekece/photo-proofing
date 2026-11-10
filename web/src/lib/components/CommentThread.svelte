<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { CommentsClient } from '#lib/gallery/comments.js';
	import type { Comment } from '#lib/gallery/schemas.js';
	import { formatDateTime } from '#lib/gallery/format.js';
	import { Check, Pencil, Trash2, X } from '@lucide/svelte';
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
	// The first answer is what the placeholder waits for; later photos keep the thread in place.
	let loaded = $state(false);
	let threadPhoto: string | undefined;
	let pending = $state(false);
	let error = $state('');
	let draft = $state('');
	let editing = $state<string | null>(null);
	let editDraft = $state('');
	let deleting = $state<string | null>(null);
	const valid = $derived(draft.trim().length > 0 && [...draft].length <= 500);
	const editValid = $derived(editDraft.trim().length > 0 && [...editDraft].length <= 500);

	$effect(() => {
		const current = photoId;
		void refresh;
		let active = true;

		if (current !== threadPhoto) {
			threadPhoto = current;
			editing = null;
			deleting = null;
			draft = '';
		}

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
				if (!active) return;
				loading = false;
				loaded = true;
			});
		return () => {
			active = false;
		};
	});

	async function submit(save: () => Promise<void>) {
		if (pending) return;
		pending = true;
		error = '';
		try {
			await save();
			onchange?.();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : m.api_error_generic();
		} finally {
			pending = false;
		}
	}

	const send = () =>
		valid &&
		submit(async () => {
			comments = [...comments, await client.add(photoId, draft)];
			draft = '';
		});

	const saveEdit = () =>
		editing &&
		editValid &&
		submit(async () => {
			const comment = await client.edit(editing!, editDraft);
			comments = comments.map((item) => (item.id === comment.id ? comment : item));
			editing = null;
		});

	const swap =
		'col-start-1 row-start-1 flex gap-0.5 transition-[opacity,scale] duration-200 motion-reduce:transition-none';
	const hidden = 'pointer-events-none scale-75 opacity-0';
	const iconButton =
		'inline-grid size-7 cursor-pointer place-items-center rounded-full text-panel/75 transition-colors hover:bg-white/14 hover:text-panel focus-visible:outline-2 focus-visible:outline-viewer-focus disabled:opacity-30';

	async function remove() {
		if (!deleting) return;
		await client.remove(deleting);
		comments = await client.thread(photoId);
		onchange?.();
	}
</script>

<section class="flex h-full min-h-0 flex-col gap-4 text-panel" aria-label={m.comments_title()}>
	<h2 class="font-serif text-xl">{m.comments_title()}</h2>
	<div
		class="min-h-0 flex-1 [scrollbar-width:none] overflow-y-auto overscroll-contain transition-opacity delay-150 duration-200 [&::-webkit-scrollbar]:hidden {loading &&
		loaded
			? 'opacity-50'
			: ''}"
	>
		{#if !loaded}<p role="status" class="text-sm text-panel/75">{m.comments_loading()}</p>{/if}
		{#if loaded && !loading && comments.length === 0}<p class="text-sm text-panel/75">
				{m.comments_empty()}
			</p>{/if}
		<ol class="space-y-5">
			{#each comments as comment (comment.id)}
				<li class="group relative border-b border-panel/15 pb-4">
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
					{#if editing === comment.id}
						<form
							id="{id}-edit"
							class="grid gap-2"
							onsubmit={(event) => {
								event.preventDefault();
								void saveEdit();
							}}
						>
							<textarea
								aria-label={m.comments_edit()}
								bind:value={editDraft}
								disabled={pending}
								rows="3"
								class="max-h-40 min-h-20 w-full resize-none rounded-md border border-panel/30 bg-dark p-3 text-base text-panel focus-visible:outline-2 focus-visible:outline-viewer-focus disabled:opacity-50"
							></textarea>
						</form>
					{:else}
						<p
							class="wrap-anywhere whitespace-pre-wrap {comment.deleted_at
								? 'text-panel/65'
								: ''} {comment.deleted_at && comment.body !== null ? 'line-through' : ''}"
						>
							{comment.body ?? m.comments_deleted()}
						</p>
					{/if}
					{#if comment.author === viewer && !comment.deleted_at}
						{@const isEditing = editing === comment.id}
						<div
							class="absolute top-0 right-0 grid [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-focus-within:opacity-100 [@media(hover:hover)]:group-hover:opacity-100 {isEditing
								? 'opacity-100!'
								: ''}"
						>
							<div
								class="{swap} {isEditing ? hidden : ''}"
								aria-hidden={isEditing}
								inert={isEditing}
							>
								<button
									type="button"
									disabled={pending}
									aria-label={m.comments_edit()}
									class={iconButton}
									onclick={() => {
										editing = comment.id;
										editDraft = comment.body ?? '';
									}}><Pencil size={14} aria-hidden="true" /></button
								>
								<button
									type="button"
									disabled={pending}
									aria-label={m.comments_delete()}
									class="{iconButton} hover:text-warning-on-dark"
									onclick={() => (deleting = comment.id)}
									><Trash2 size={14} aria-hidden="true" /></button
								>
							</div>
							<div
								class="{swap} {isEditing ? '' : hidden}"
								aria-hidden={!isEditing}
								inert={!isEditing}
							>
								<button
									type="button"
									disabled={pending}
									aria-label={m.common_cancel()}
									class={iconButton}
									onclick={() => (editing = null)}><X size={14} aria-hidden="true" /></button
								>
								<button
									type="submit"
									form="{id}-edit"
									disabled={pending || !editValid}
									aria-label={m.comments_save()}
									class="{iconButton} hover:text-ok-on-dark"
									><Check size={14} aria-hidden="true" /></button
								>
							</div>
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
			>{viewer === 'client' ? m.comments_add() : m.comments_reply()}</label
		>
		<textarea
			id="{id}-body"
			bind:value={draft}
			disabled={pending || !loaded}
			rows="3"
			class="max-h-40 min-h-20 w-full resize-none rounded-md border border-panel/30 bg-dark p-3 text-base text-panel focus-visible:outline-2 focus-visible:outline-viewer-focus disabled:opacity-50"
		></textarea>
		<div class="flex flex-wrap items-center justify-end gap-2">
			<div class="flex gap-2">
				<ViewerButton
					type="submit"
					disabled={pending || !loaded || !valid}
					class="bg-panel text-foreground hover:bg-panel/90"
					>{pending && !editing ? m.comments_saving() : m.comments_send()}</ViewerButton
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
