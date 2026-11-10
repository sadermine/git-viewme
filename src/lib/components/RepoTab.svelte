<script lang="ts">
  import * as api from '$lib/api';
  import { combine, type Refreshable } from '$lib/refresh';
  import type { Repo } from '$lib/types';
  import { VIEWS, type ViewId, type ViewProps } from '$lib/views';
  import RefreshControl from './RefreshControl.svelte';

  let { repo, tab, update, hidden }: Omit<ViewProps, 'track'> & { hidden: boolean } = $props();

  // A view mounts the first time it is shown and then stays mounted, so its data and its folder
  // state survive switching views and tabs.
  let shown = $state<ViewId[]>([]);
  $effect.pre(() => {
    if (!shown.includes(tab.view)) shown.push(tab.view);
  });

  let tracked = $state<Partial<Record<ViewId, () => readonly (Refreshable | null)[]>>>({});
  const loads = $derived((tracked[tab.view]?.() ?? []).filter((load) => load !== null));
  const status = $derived(combine(loads.map((load) => load.status)));
  const refresh = () => void Promise.all(loads.map((load) => load.refresh()));

  function key(event: KeyboardEvent) {
    if (event.key === 'F5' && !hidden) refresh();
  }

  const project = $derived(repo.linked_projects[0]);
  const displayUrl = (r: Repo) => r.html_url.replace(/^https:\/\//, '');
</script>

<svelte:window onkeydown={key} />

<div class="repo-tab" {hidden}>
  <header class="repobar">
    <div class="views" role="group" aria-label="Views">
      {#each VIEWS as view (view.id)}
        <button class:on={tab.view === view.id} aria-pressed={tab.view === view.id} onclick={() => update({ view: view.id })}>
          {view.label}
        </button>
      {/each}
    </div>
    <span class="spacer"></span>
    <RefreshControl {status} {refresh} title="Refresh this view (F5)" />
    {#if repo.default_branch}<span class="chip" title="Default branch">⎇ {repo.default_branch}</span>{/if}
    {#if project}<span class="chip" title="Linked project">▦ {project.title}</span>{/if}
    <button class="url link" title="Open {repo.html_url} in the browser" onclick={() => api.openExternal(repo.html_url)}>
      {displayUrl(repo)} ↗
    </button>
  </header>

  {#each VIEWS as view (view.id)}
    {#if shown.includes(view.id)}
      <div class="view" hidden={tab.view !== view.id}>
        <view.component {repo} {tab} {update} track={(each) => (tracked[view.id] = each)} />
      </div>
    {/if}
  {/each}
</div>

<style>
  .repo-tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .repobar {
    display: flex;
    flex: none;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }
  .views {
    display: flex;
    flex: none;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }
  .views button {
    padding: 3px 12px;
    border-right: 1px solid var(--border);
  }
  .views button:last-child {
    border-right: 0;
  }
  .views button:hover {
    background: var(--bg-hover);
  }
  .views button.on {
    background: var(--fill);
    color: var(--on-fill);
  }
  .spacer {
    margin-left: auto;
  }
  .chip {
    flex: 0 1 auto;
    min-width: 3em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 1px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 12px;
  }
  /* The URL gives up its width first: it shrinks to an ellipsis before the chips shrink at all. */
  .url {
    flex: 0 100000 auto;
    min-width: 4em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .view {
    flex: 1;
    display: flex;
    min-height: 0;
    min-width: 0;
  }
</style>
