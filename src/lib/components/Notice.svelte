<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div class="notice">
  <div class="box">
    <strong>{title}</strong>
    {@render children?.()}
  </div>
</div>

<style>
  .notice {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 24px;
    color: var(--text-muted);
    text-align: center;
  }
  .box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
    border: 1px dashed var(--border);
    border-radius: 8px;
    padding: 24px 32px;
  }
  strong {
    color: var(--text);
  }
</style>
