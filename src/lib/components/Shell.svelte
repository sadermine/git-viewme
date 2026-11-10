<script lang="ts">
  import * as api from '$lib/api';
  import { describeError } from '$lib/errors';
  import { Load, session } from '$lib/session.svelte';
  import { repoKey, type Repo } from '$lib/types';
  import { Workspace } from '$lib/workspace.svelte';
  import LoadView from './LoadView.svelte';
  import Notice from './Notice.svelte';
  import RefreshControl from './RefreshControl.svelte';
  import RepoTab from './RepoTab.svelte';
  import Sidebar from './Sidebar.svelte';
  import Tabs from './Tabs.svelte';

  const workspace = new Workspace();
  const repos = new Load('repository list', api.listRepos);
  let signOutError = $state<string | null>(null);

  const byKey = $derived(
    new Map(repos.current.state.kind === 'ready' ? repos.current.state.value.map((repo): [string, Repo] => [repoKey(repo.ref), repo]) : []),
  );

  /** F5 refreshes the active tab (RepoTab) and never reloads the window, which would close every tab. */
  function key(event: KeyboardEvent) {
    if (event.key === 'F5') event.preventDefault();
  }

  async function signOut() {
    try {
      await session.signOut();
    } catch (error) {
      signOutError = `Sign-out failed. ${describeError(api.asCoreError(error), 'stored sign-in')}`;
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="shell">
  <aside class="sidebar">
    <div class="sidebar-head">
      <h2>Repositories</h2>
      <RefreshControl status={repos.status} refresh={() => void repos.refresh()} title="Refresh the repository list" />
    </div>
    <LoadView load={repos}>
      {#snippet children(list)}
        <Sidebar repos={list} {workspace} />
      {/snippet}
    </LoadView>
  </aside>

  <section class="main">
    {#if workspace.openTabs.length === 0}
      <Notice title="No repository open">Pick one from the sidebar.</Notice>
    {:else}
      <Tabs {workspace} />
      {#each workspace.openTabs as ref (repoKey(ref))}
        {@const repo = byKey.get(repoKey(ref))}
        {@const tab = workspace.tabState.get(repoKey(ref))}
        {#if repo && tab}
          <RepoTab {repo} {tab} hidden={!workspace.isActive(ref)} update={(change) => workspace.update(ref, change)} />
        {/if}
      {/each}
    {/if}
  </section>
</div>

<footer class="status">
  <span>{repos.current.state.kind === 'ready' ? `${repos.current.state.value.length} repositories` : 'Loading repositories'}</span>
  <span>{workspace.openTabs.length} open</span>
  {#if signOutError}<span class="error" role="alert">{signOutError}</span>{/if}
  <button class="sign-out" onclick={signOut}>Sign out</button>
</footer>

<style>
  .shell {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .sidebar {
    width: 260px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--bg-chrome);
  }
  .sidebar-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px 4px;
  }
  h2 {
    margin: 0;
    padding-top: 2px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .status {
    height: 24px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 10px;
    border-top: 1px solid var(--border);
    background: var(--bg-chrome);
    color: var(--text-muted);
    font-size: 12px;
  }
  .error {
    color: var(--danger);
  }
  .sign-out {
    margin-left: auto;
    padding: 0 6px;
    border-radius: 3px;
  }
  .sign-out:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
</style>
