<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import type { CommentsClient } from '#lib/gallery/comments.js';
	import type { Comment } from '#lib/gallery/schemas.js';
	import { formatDateTime } from '#lib/gallery/format.js';
	import { Check, LoaderCircle, Pencil, SendHorizontal, Trash2, X } from '@lucide/svelte';
	import ConfirmDialog from './ui/ConfirmDialog/ConfirmDialog.svelte';

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
	let composer = $state<HTMLTextAreaElement>();
	let editing = $state<string | null>(null);
	let editDraft = $state('');
	let deleting = $state<string | null>(null);
	const valid = $derived(draft.trim().length > 0 && [...draft].length <= 500);
	const editValid = $derived(editDraft.trim().length > 0 && [...editDraft].length <= 500);

	$effect(() => {
		void draft;
		if (!composer) return;
		composer.style.height = 'auto';
		composer.style.height = `${composer.scrollHeight}px`;
	});

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
		<ol class="space-y-4">
			{#each comments as comment (comment.id)}
				{@const mine = comment.author === viewer}
				{@const editable = mine && !comment.deleted_at}
				{@const isEditing = editing === comment.id}
				<li class="group flex flex-col gap-1 {mine ? 'items-end' : 'items-start'}">
					<div class="flex min-h-7 w-full items-center justify-between gap-2 text-xs text-panel/75">
						<span class="flex gap-x-2">
							<time datetime={comment.created_at}>{formatDateTime(comment.created_at)}</time>
							{#if comment.edited_at && !comment.deleted_at}<span>{m.comments_edited()}</span>{/if}
						</span>
						{#if editable}
							<div
								class="grid shrink-0 [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-focus-within:opacity-100 [@media(hover:hover)]:group-hover:opacity-100 {isEditing
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
					</div>
					<div class="flex w-full min-w-0 items-center gap-1 {mine ? 'justify-end' : ''}">
						{#if isEditing}
							<form
								id="{id}-edit"
								class="grid min-w-0 flex-1 gap-2"
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
									class="max-h-40 min-h-20 w-full resize-none [scrollbar-width:none] rounded-2xl border border-panel/30 bg-dark p-3 text-base text-panel focus-visible:border-panel focus-visible:outline-none disabled:opacity-50 [&::-webkit-scrollbar]:hidden"
								></textarea>
							</form>
						{:else}
							<p
								class="w-full rounded-2xl px-3.5 py-2 wrap-anywhere whitespace-pre-wrap {mine
									? 'rounded-br-md bg-panel text-foreground'
									: 'rounded-bl-md bg-white/10 text-panel'} {comment.deleted_at
									? 'italic opacity-65'
									: ''} {comment.deleted_at && comment.body !== null ? 'line-through' : ''}"
							>
								{comment.body ?? m.comments_deleted()}
							</p>
						{/if}
					</div>
					<strong class="text-xs font-semibold text-panel"
						>{mine
							? m.comments_you()
							: comment.author === 'client'
								? m.comments_client()
								: m.comments_photographer()}</strong
					>
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
		<label for="{id}-body" class="sr-only"
			>{viewer === 'client' ? m.comments_add() : m.comments_reply()}</label
		>
		<div class="flex items-end gap-2">
			<textarea
				id="{id}-body"
				bind:this={composer}
				bind:value={draft}
				disabled={pending || !loaded}
				rows="1"
				placeholder={viewer === 'client' ? m.comments_add() : m.comments_reply()}
				class="max-h-40 min-h-11 min-w-0 flex-1 resize-none [scrollbar-width:none] rounded-3xl border border-panel/30 bg-dark px-4 py-2.5 text-base leading-6 text-panel placeholder:text-panel/55 focus-visible:border-panel focus-visible:outline-none disabled:opacity-50 [&::-webkit-scrollbar]:hidden"
			></textarea>
			<button
				type="submit"
				disabled={pending || !loaded || !valid}
				aria-label={pending && !editing ? m.comments_saving() : m.comments_send()}
				class="inline-grid size-11 shrink-0 cursor-pointer place-items-center rounded-full bg-panel text-foreground transition-colors hover:bg-panel/90 focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-viewer-focus disabled:cursor-default disabled:bg-white/10 disabled:text-panel/40 disabled:hover:bg-white/10"
			>
				{#if pending && !editing}
					<LoaderCircle size={18} aria-hidden="true" class="animate-spin-slow" />
				{:else}
					<SendHorizontal size={18} aria-hidden="true" />
				{/if}
			</button>
		</div>
	</form>
</section>

{#if deleting}
	<ConfirmDialog
		title={m.comments_delete_question()}
		confirmLabel={m.comments_delete()}
		destructive
		onconfirm={remove}
		onclose={() => (deleting = null)}
	/>
{/if}
