<script lang="ts">
  import { repoKey, type RepoRef } from '$lib/types';
  import type { Workspace } from '$lib/workspace.svelte';

  let { workspace }: { workspace: Workspace } = $props();

  function middleClose(event: MouseEvent, ref: RepoRef) {
    if (event.button === 1) {
      event.preventDefault();
      workspace.close(ref);
    }
  }
</script>

<div class="tabs" role="tablist" aria-label="Open repositories">
  {#each workspace.openTabs as ref (repoKey(ref))}
    <div
      class="tab"
      class:active={workspace.isActive(ref)}
      title={repoKey(ref)}
      role="presentation"
      onauxclick={(event) => middleClose(event, ref)}
      onmousedown={(event) => event.button === 1 && event.preventDefault()}
    >
      <button class="label" role="tab" aria-selected={workspace.isActive(ref)} onclick={() => workspace.open(ref)}>
        {ref.name}
      </button>
      <button class="close" aria-label="Close {ref.name}" title="Close" onclick={() => workspace.close(ref)}>×</button>
    </div>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    flex: none;
    min-height: 34px;
    overflow-x: auto;
    border-bottom: 1px solid var(--border);
    background: var(--bg-chrome);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: 6px;
    border-right: 1px solid var(--border);
    white-space: nowrap;
    color: var(--text-muted);
  }
  .tab.active {
    margin-bottom: -1px;
    background: var(--bg);
    color: var(--text);
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .label {
    align-self: stretch;
    padding: 0 6px 0 12px;
  }
  .close {
    width: 20px;
    height: 20px;
    border-radius: 3px;
    visibility: hidden;
  }
  .tab:hover .close,
  .tab.active .close,
  .close:focus-visible {
    visibility: visible;
  }
  .close:hover {
    background: var(--bg-hover);
  }
</style>
