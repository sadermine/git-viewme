<script lang="ts">
  import * as api from '$lib/api';
  import LoadView from '$lib/components/LoadView.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import { Load } from '$lib/session.svelte';
  import type { IssueState } from '$lib/types';
  import type { ViewProps } from '$lib/views';

  let { repo, track }: ViewProps = $props();

  const STATES: readonly { id: IssueState; label: string }[] = [
    { id: 'open', label: 'Open' },
    { id: 'closed', label: 'Closed' },
  ];

  let selected = $state<IssueState>('open');

  // Primitives, so a sidebar refresh that replaces `repo` with an equal one does not reload the list.
  const owner = $derived(repo.ref.owner);
  const name = $derived(repo.ref.name);
  const issues = $derived.by(() => {
    const ref = { owner, name };
    const which = selected;
    return new Load('issues', () => api.listIssues(ref, which));
  });
  $effect.pre(() => track(() => [issues]));

  const formatDate = (iso: string) =>
    new Date(iso).toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
</script>

<div class="issues">
  <div class="toolbar">
    <div class="states" role="group" aria-label="Issue state">
      {#each STATES as option (option.id)}
        <button class:on={selected === option.id} aria-pressed={selected === option.id} onclick={() => (selected = option.id)}>
          {option.label}
        </button>
      {/each}
    </div>
  </div>
  <LoadView load={issues}>
    {#snippet children(list)}
      {#if list.length === 0}
        <Notice title="{repo.ref.name} has no {selected} issues" />
      {:else}
        <ul class="list">
          {#each list as issue (issue.number)}
            <li>
              <button class="row" title={issue.html_url} onclick={() => api.openExternal(issue.html_url)}>
                <span class="number">#{issue.number}</span>
                <span class="main">
                  <span class="title">{issue.title}</span>
                  {#if issue.labels.length > 0}
                    <span class="labels">
                      {#each issue.labels as label (label)}<span class="label">{label}</span>{/each}
                    </span>
                  {/if}
                </span>
                <span class="meta">{issue.author} · {formatDate(issue.updated_at)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </LoadView>
</div>

<style>
  .issues {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .toolbar {
    flex: none;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .states {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }
  .states button {
    padding: 2px 12px;
    border-right: 1px solid var(--border);
  }
  .states button:last-child {
    border-right: 0;
  }
  .states button:hover {
    background: var(--bg-hover);
  }
  .states button.on {
    background: var(--fill);
    color: var(--on-fill);
  }
  .list {
    flex: 1;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow-y: auto;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 8px 12px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    text-align: left;
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .number {
    flex: none;
    min-width: 4ch;
    color: var(--text-muted);
    font-size: 12px;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
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
  .meta {
    flex: none;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
