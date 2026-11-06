<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import type { Load } from '$lib/session.svelte';
  import { describeError } from '$lib/errors';

  let { load, what, children }: { load: Load<T>; what: string; children: Snippet<[T]> } = $props();
</script>

{#if load.current.kind === 'loading'}
  <div class="placeholder" role="status">Loading the {what}…</div>
{:else if load.current.kind === 'failed'}
  <div class="failed" role="alert">
    <p>{describeError(load.current.error, what)}</p>
    <button class="retry" onclick={() => load.retry()}>Retry</button>
  </div>
{:else}
  {@render children(load.current.value)}
{/if}

<style>
  .placeholder,
  .failed {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    color: var(--text-muted);
    text-align: center;
  }
  .failed p {
    margin: 0;
    max-width: 48ch;
    color: var(--danger);
  }
  .retry {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 14px;
    color: var(--text);
    background: var(--bg);
  }
  .retry:hover {
    background: var(--bg-hover);
  }
</style>
