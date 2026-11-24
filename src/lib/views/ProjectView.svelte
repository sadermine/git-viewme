<script lang="ts">
  import { tick, untrack } from 'svelte';
  import * as api from '$lib/api';
  import { moveTargets, placeCard, withPendingMoves } from '$lib/board';
  import LoadView from '$lib/components/LoadView.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import { describeError } from '$lib/errors';
  import { Load, session } from '$lib/session.svelte';
  import type { Board, Column, CoreError } from '$lib/types';
  import type { ViewProps } from '$lib/views';

  let { repo, track, refusals }: ViewProps = $props();

  /** Cards whose move GitHub has not answered yet, or answered while a board fetch that began before
   *  the answer is still running, each with the option it is moving to. A second move of the same card waits. */
  let pending = $state<Record<string, string>>({});

  const project = $derived(repo.linked_projects[0] ?? null);
  // A key, so a sidebar refresh that replaces `repo` with an equal one does not reload the board.
  const projectKey = $derived(project === null ? null : `${project.owner}/${project.number}`);
  const board = $derived.by(() => {
    if (projectKey === null) return null;
    const linked = untrack(() => project);
    return linked === null
      ? null
      : new Load('project board', async () => withPendingMoves(await api.projectBoard(linked), pending));
  });
  $effect.pre(() => track(() => [board]));
  const projectsUrl = $derived(`https://github.com/${repo.ref.owner}/${repo.ref.name}/projects`);

  // Kept in the workspace, so a refusal that arrives after the tab closed shows when it reopens.
  const failures = $derived(projectKey === null ? [] : (refusals.get(projectKey) ?? []));
  let dragging = $state<string | null>(null);
  let dropTarget = $state<Column | null>(null);
  let menuFor = $state<string | null>(null);

  /** The board on screen, which a refresh may have replaced since a move began. */
  const shown = (): Board | null => (board?.current.state.kind === 'ready' ? board.current.state.value : null);

  /** Moves the card at once and asks GitHub to set its Status. If GitHub refuses, the card goes
   *  back to where it was, on whichever board is on screen by then. */
  async function move(value: Board, itemId: string, to: Column, refocus: boolean) {
    const fieldId = value.status_field_id;
    const optionId = to.option_id;
    const from = value.columns.find((column) => column.cards.some((card) => card.item_id === itemId));
    // Captured before the await, so a late answer is recorded in the workspace the move began in.
    const key = projectKey;
    const store = refusals;
    if (key === null || fieldId === null || optionId === null || from === undefined || from === to || pending[itemId]) return;
    const index = from.cards.findIndex((card) => card.item_id === itemId);
    const title = from.cards[index].title;
    placeCard(value, itemId, optionId);
    store.set(key, (store.get(key) ?? []).filter((each) => each.itemId !== itemId));
    pending[itemId] = optionId;
    if (refocus) void focusCard(itemId);
    try {
      await api.moveCard({ project_id: value.project_id, item_id: itemId, field_id: fieldId, option_id: optionId });
      if (board?.current.refreshing) await board.refresh();
      delete pending[itemId];
    } catch (error) {
      delete pending[itemId];
      // Recorded before the board is touched: after the tab closed, shown() reads a destroyed view.
      const refusal = { itemId, title, error: api.asCoreError(error) };
      store.set(key, [...(store.get(key) ?? []).filter((each) => each.itemId !== itemId), refusal]);
      const current = shown();
      if (current !== null) placeCard(current, itemId, from.option_id, index);
      if (refusal.error.kind === 'unauthorized') session.lost();
    }
    if (refocus) void focusCard(itemId);
  }

  function failureMessage(error: CoreError): string {
    switch (error.kind) {
      case 'missing_scope':
        return 'Moving cards needs a newer sign-in than this one. Sign in again to move cards.';
      case 'forbidden':
        return `You may not have write access to this project. GitHub said: ${error.message}`;
      default:
        return describeError(error, 'project board');
    }
  }

  /** Focuses the card's Move to button or, when the card has no lane left to go to and so no
   *  button, its open button, unless the user has since focused something else. */
  async function focusCard(itemId: string) {
    await tick();
    if (document.activeElement !== null && document.activeElement !== document.body) return;
    const card = document.querySelector<HTMLElement>(`[data-card="${CSS.escape(itemId)}"]`);
    (card?.querySelector<HTMLElement>('[data-move]') ?? card?.querySelector<HTMLElement>('.open'))?.focus();
  }

  function closeMenu(itemId: string) {
    menuFor = null;
    void focusCard(itemId);
  }

  function focusFirstItem(menu: HTMLElement) {
    menu.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }

  function menuKey(event: KeyboardEvent, itemId: string) {
    const items = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Escape') {
      event.preventDefault();
      closeMenu(itemId);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
    }
  }

  function leaveMenu(event: FocusEvent) {
    const wrapper = event.currentTarget as HTMLElement;
    if (!wrapper.contains(event.relatedTarget as Node | null)) menuFor = null;
  }

  function startDrag(event: DragEvent, itemId: string) {
    dragging = itemId;
    menuFor = null;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', itemId);
    }
  }

  function endDrag() {
    dragging = null;
    dropTarget = null;
  }

  function dragOver(event: DragEvent, column: Column) {
    if (dragging === null || column.option_id === null) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    dropTarget = column;
  }

  function dragLeave(event: DragEvent, column: Column) {
    const lane = event.currentTarget as HTMLElement;
    if (dropTarget === column && !lane.contains(event.relatedTarget as Node | null)) dropTarget = null;
  }

  function drop(event: DragEvent, value: Board, column: Column) {
    event.preventDefault();
    const itemId = dragging;
    endDrag();
    if (itemId !== null) void move(value, itemId, column, false);
  }
</script>

{#if board === null}
  <Notice title="No project linked to {repo.ref.name}">
    <button class="link" onclick={() => api.openExternal(projectsUrl)}>Open its projects on GitHub</button>
  </Notice>
{:else}
  <LoadView load={board}>
    {#snippet children(value)}
      <div class="project">
        {#if failures.length > 0}
          <div class="failure" role="alert">
            <div class="lines">
              {#each failures as refusal (refusal.itemId)}
                <p>"{refusal.title}" went back to its lane. {failureMessage(refusal.error)}</p>
              {/each}
            </div>
            {#if failures.some((each) => each.error.kind === 'missing_scope')}
              <button onclick={() => session.signOut()}>Sign out and sign in again</button>
            {/if}
            <button onclick={() => projectKey !== null && refusals.delete(projectKey)}>Dismiss</button>
          </div>
        {/if}
        {#if value.columns.length === 0}
          <Notice title="This project has no items." />
        {:else}
          <div class="board">
            {#each value.columns as column (column.option_id ?? '')}
              {@const targets = moveTargets(value, column)}
              <section
                class="column"
                class:target={dropTarget === column}
                aria-label={column.name}
                ondragover={(event) => dragOver(event, column)}
                ondragleave={(event) => dragLeave(event, column)}
                ondrop={(event) => drop(event, value, column)}
              >
                <header>
                  <span class="name">{column.name}</span>
                  <span class="count">{column.cards.length}</span>
                </header>
                <div class="cards">
                  {#each column.cards as card (card.item_id)}
                    {@const busy = pending[card.item_id] !== undefined}
                    <article
                      class="card"
                      data-card={card.item_id}
                      class:dragging={dragging === card.item_id}
                      aria-busy={busy}
                      draggable={targets.length > 0 && !busy}
                      ondragstart={(event) => startDrag(event, card.item_id)}
                      ondragend={endDrag}
                    >
                      <button class="open" title={card.url} onclick={() => api.openExternal(card.url)}>
                        <span class="title">{card.title}</span>
                        {#if card.number !== null}<span class="number">#{card.number}</span>{/if}
                        {#if card.labels.length > 0}
                          <span class="labels">
                            {#each card.labels as label (label)}<span class="label">{label}</span>{/each}
                          </span>
                        {/if}
                      </button>
                      {#if targets.length > 0 || busy}
                        <div class="move" onfocusout={leaveMenu}>
                          <button
                            class="move-button"
                            data-move={card.item_id}
                            aria-haspopup="menu"
                            aria-expanded={menuFor === card.item_id}
                            aria-disabled={busy}
                            onclick={() => {
                              if (!busy) menuFor = menuFor === card.item_id ? null : card.item_id;
                            }}
                          >
                            {busy ? 'Moving…' : 'Move to'}
                          </button>
                          {#if menuFor === card.item_id}
                            <div
                              class="menu"
                              role="menu"
                              tabindex="-1"
                              aria-label="Move {card.title} to"
                              {@attach focusFirstItem}
                              onkeydown={(event) => menuKey(event, card.item_id)}
                            >
                              {#each targets as target (target.option_id)}
                                <button
                                  role="menuitem"
                                  onclick={() => {
                                    menuFor = null;
                                    void move(value, card.item_id, target, true);
                                  }}>{target.name}</button
                                >
                              {/each}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </article>
                  {/each}
                </div>
              </section>
            {/each}
          </div>
        {/if}
      </div>
    {/snippet}
  </LoadView>
{/if}

<style>
  .project {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .failure {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 10px 10px 0;
    padding: 8px 10px;
    border: 1px solid var(--danger);
    border-radius: 6px;
  }
  .failure .lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .failure p {
    margin: 0;
    color: var(--danger);
  }
  .failure button {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 10px;
    color: var(--text);
    background: var(--bg);
  }
  .board {
    flex: 1;
    display: flex;
    gap: 10px;
    padding: 10px;
    overflow: auto;
    min-width: 0;
    min-height: 0;
  }
  .column {
    width: 260px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-chrome);
  }
  .column.target {
    border-color: var(--accent);
    background: var(--bg-selected);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    font-weight: 600;
  }
  .count {
    margin-left: auto;
    padding: 0 6px;
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--bg-hover);
    font-size: 12px;
  }
  .cards {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 8px 8px;
    overflow-y: auto;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
  }
  .card:hover,
  .card:focus-within {
    border-color: var(--accent);
  }
  .card.dragging,
  .card[aria-busy='true'] {
    opacity: 0.6;
  }
  .card[draggable='true'] {
    cursor: grab;
  }
  .open {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    border: none;
    background: none;
    text-align: left;
    cursor: inherit;
  }
  .number {
    color: var(--text-muted);
    font-size: 12px;
  }
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .label {
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .move {
    padding: 0 8px 8px;
  }
  .move-button {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 8px;
    color: var(--text-muted);
    background: var(--bg);
    font-size: 12px;
  }
  .move-button:hover:not([aria-disabled='true']) {
    color: var(--text);
    background: var(--bg-hover);
  }
  .menu {
    position: absolute;
    z-index: 1;
    left: 8px;
    right: 8px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
  }
  .menu button {
    padding: 4px 8px;
    border: none;
    border-radius: 3px;
    color: var(--text);
    background: none;
    text-align: left;
  }
  .menu button:hover,
  .menu button:focus {
    background: var(--bg-hover);
  }
</style>
