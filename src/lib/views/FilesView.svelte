<script lang="ts">
  import { SvelteSet } from 'svelte/reactivity';
  import * as api from '$lib/api';
  import LoadView from '$lib/components/LoadView.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import { Load } from '$lib/session.svelte';
  import type { TreeEntry } from '$lib/types';
  import type { ViewProps } from '$lib/views';

  let { repo, tab, update, track }: ViewProps = $props();

  const branch = $derived(repo.default_branch);
  // A derived primitive, so switching views (which replaces `tab`) does not refetch the file.
  const selectedPath = $derived(tab.selectedPath);
  const expanded = new SvelteSet<string>();

  // Primitives, so a sidebar refresh that replaces `repo` with an equal one does not reload the tree.
  const owner = $derived(repo.ref.owner);
  const name = $derived(repo.ref.name);

  const tree = $derived.by(() => {
    const ref = { owner, name };
    return branch === null ? null : new Load('file tree', () => api.fileTree(ref, branch));
  });

  const content = $derived.by(() => {
    const ref = { owner, name };
    const path = selectedPath;
    return branch === null || path === null ? null : new Load('file', () => api.fileContent(ref, branch, path));
  });

  $effect.pre(() => track(() => [tree, content]));

  /** Relies on core returning the tree depth first, so a collapsed folder's contents follow it. */
  function visible(entries: TreeEntry[]): TreeEntry[] {
    const out: TreeEntry[] = [];
    let hiddenPrefix: string | null = null;
    for (const entry of entries) {
      if (hiddenPrefix !== null && entry.path.startsWith(hiddenPrefix)) continue;
      hiddenPrefix = null;
      out.push(entry);
      if (entry.kind === 'dir' && !expanded.has(entry.path)) hiddenPrefix = `${entry.path}/`;
    }
    return out;
  }

  function toggle(path: string) {
    if (!expanded.delete(path)) expanded.add(path);
  }

  const depth = (path: string) => path.split('/').length - 1;
  const baseName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

  function lines(text: string): string[] {
    const all = text.split(/\r?\n/);
    if (all.length > 1 && all[all.length - 1] === '') all.pop();
    return all;
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} bytes`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function blobUrl(branchName: string, path: string): string {
    const encode = (value: string) => value.split('/').map(encodeURIComponent).join('/');
    return `${repo.html_url}/blob/${encode(branchName)}/${encode(path)}`;
  }
</script>

{#if branch === null || tree === null}
  <Notice title="This repository is empty">It has no commits yet, so there are no files to show.</Notice>
{:else}
  <LoadView load={tree}>
    {#snippet children(entries)}
      <div class="files">
        <nav class="tree" aria-label="Files">
          {#each visible(entries) as entry (entry.path)}
            <button
              class="row"
              class:selected={entry.path === selectedPath}
              style:padding-left="{8 + depth(entry.path) * 14}px"
              title={entry.path}
              aria-expanded={entry.kind === 'dir' ? expanded.has(entry.path) : undefined}
              onclick={() => (entry.kind === 'dir' ? toggle(entry.path) : update({ selectedPath: entry.path }))}
            >
              <span class="twisty" aria-hidden="true">
                {entry.kind === 'dir' ? (expanded.has(entry.path) ? '▾' : '▸') : ''}
              </span>
              <span class="name">{baseName(entry.path)}</span>
            </button>
          {/each}
        </nav>

        <section class="content">
          {#if content === null || selectedPath === null}
            <Notice title="No file open">Pick a file from the tree.</Notice>
          {:else if !entries.some((entry) => entry.path === selectedPath)}
            <Notice title="{baseName(selectedPath)} no longer exists">It is not on {branch} any more. Pick another file from the tree.</Notice>
          {:else}
            <div class="crumbs" aria-label="Path">
              {#each selectedPath.split('/') as part, i (i)}
                {#if i > 0}<span class="sep">/</span>{/if}<span>{part}</span>
              {/each}
            </div>
            <LoadView load={content}>
              {#snippet children(file)}
                {#if file.kind === 'text'}
                  {@const numbered = lines(file.text)}
                  <div class="code mono">
                    <pre class="gutter" aria-hidden="true">{numbered.map((_, i) => i + 1).join('\n')}</pre>
                    <pre class="text">{numbered.join('\n')}</pre>
                  </div>
                {:else if file.kind === 'binary'}
                  <Notice title="Binary file not shown">{formatSize(file.size)}</Notice>
                {:else}
                  <Notice title="File too large to show">
                    <span>{formatSize(file.size)}</span>
                    <button class="link" onclick={() => api.openExternal(blobUrl(branch, selectedPath))}>
                      Open it on GitHub
                    </button>
                  </Notice>
                {/if}
              {/snippet}
            </LoadView>
          {/if}
        </section>
      </div>
    {/snippet}
  </LoadView>
{/if}

<style>
  .files {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
  }
  .tree {
    width: 280px;
    flex: none;
    overflow: auto;
    padding: 4px 0;
    border-right: 1px solid var(--border);
  }
  .row {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    text-align: left;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.selected {
    background: var(--bg-selected);
  }
  .twisty {
    width: 1em;
    flex: none;
    color: var(--text-muted);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .crumbs {
    flex: none;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .crumbs span:last-child {
    color: var(--text);
  }
  .sep {
    padding: 0 4px;
  }
  .code {
    flex: 1;
    display: flex;
    align-items: flex-start;
    overflow: auto;
    min-height: 0;
  }
  pre {
    margin: 0;
    padding: 8px 12px;
    line-height: 1.5;
  }
  .gutter {
    position: sticky;
    left: 0;
    flex: none;
    min-height: 100%;
    text-align: right;
    color: var(--text-muted);
    background: var(--bg-chrome);
    border-right: 1px solid var(--border);
    user-select: none;
  }
  .text {
    flex: 1;
    white-space: pre;
  }
</style>
