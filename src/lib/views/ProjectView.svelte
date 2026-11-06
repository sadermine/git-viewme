<script lang="ts">
  import * as api from '$lib/api';
  import LoadView from '$lib/components/LoadView.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import { Load } from '$lib/session.svelte';
  import type { ViewProps } from '$lib/views';

  let { repo }: ViewProps = $props();

  const project = $derived(repo.linked_projects[0] ?? null);
  const board = $derived.by(() => {
    const linked = project;
    return linked === null ? null : new Load(() => api.projectBoard(linked));
  });
  const projectsUrl = $derived(`https://github.com/${repo.ref.owner}/${repo.ref.name}/projects`);
</script>

{#if board === null}
  <Notice title="No project linked to {repo.ref.name}">
    <button class="link" onclick={() => api.openExternal(projectsUrl)}>Open its projects on GitHub</button>
  </Notice>
{:else}
  <LoadView load={board} what="project board">
    {#snippet children(value)}
      <div class="board">
        {#each value.columns as column (column.name)}
          <section class="column" aria-label={column.name}>
            <header>
              <span class="name">{column.name}</span>
              <span class="count">{column.cards.length}</span>
            </header>
            <div class="cards">
              {#each column.cards as card, i (`${card.url}#${i}`)}
                <button class="card" title={card.url} onclick={() => api.openExternal(card.url)}>
                  <span class="title">{card.title}</span>
                  {#if card.number !== null}<span class="number">#{card.number}</span>{/if}
                  {#if card.labels.length > 0}
                    <span class="labels">
                      {#each card.labels as label (label)}<span class="label">{label}</span>{/each}
                    </span>
                  {/if}
                </button>
              {/each}
            </div>
          </section>
        {/each}
      </div>
    {/snippet}
  </LoadView>
{/if}

<style>
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
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 8px 8px;
    overflow-y: auto;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    text-align: left;
  }
  .card:hover {
    border-color: var(--accent);
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
</style>
