<script lang="ts">
  import { repoKey, type Repo } from '$lib/types';
  import type { Workspace } from '$lib/workspace.svelte';

  let { repos, workspace }: { repos: Repo[]; workspace: Workspace } = $props();

  let filter = $state('');

  const groups = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    const byOwner = new Map<string, Repo[]>();
    for (const repo of repos) {
      if (!repoKey(repo.ref).toLowerCase().includes(needle)) continue;
      const group = byOwner.get(repo.ref.owner);
      if (group) group.push(repo);
      else byOwner.set(repo.ref.owner, [repo]);
    }
    return [...byOwner];
  });
</script>

<h2>Repositories</h2>
<input bind:value={filter} placeholder="Filter repositories" aria-label="Filter repositories" spellcheck="false" />
<nav class="list" aria-label="Repositories">
  {#each groups as [owner, group] (owner)}
    <div class="owner">{owner}</div>
    {#each group as repo (repoKey(repo.ref))}
      <button
        class="repo"
        class:active={workspace.isActive(repo.ref)}
        aria-current={workspace.isActive(repo.ref) ? 'page' : undefined}
        title={repoKey(repo.ref)}
        onclick={() => workspace.open(repo.ref)}
      >
        <span class="dot" class:open={workspace.isOpen(repo.ref)}></span>
        <span class="name">{repo.ref.name}</span>
        {#if repo.linked_projects.length > 0}<span class="badge">project</span>{/if}
      </button>
    {/each}
  {:else}
    <div class="owner">No repositories match “{filter}”.</div>
  {/each}
</nav>

<style>
  h2 {
    margin: 0;
    padding: 10px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  input {
    margin: 4px 10px 8px;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    font: inherit;
    color: var(--text);
    background: var(--bg);
  }
  input::placeholder {
    color: var(--text-muted);
  }
  .list {
    flex: 1;
    overflow: auto;
    padding-bottom: 8px;
  }
  .owner {
    padding: 8px 10px 2px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .repo {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 3px 10px 3px 14px;
    text-align: left;
  }
  .repo:hover {
    background: var(--bg-hover);
  }
  .repo.active {
    background: var(--bg-selected);
  }
  .dot {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
  }
  .dot.open {
    background: var(--text);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex: none;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 10px;
    color: var(--text-muted);
  }
</style>
