<script lang="ts">
  import { describeError } from '$lib/errors';
  import { updatedLabel, type RefreshStatus } from '$lib/refresh';

  let { status, refresh, title }: { status: RefreshStatus; refresh: () => void; title: string } = $props();

  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 15_000);
    return () => clearInterval(timer);
  });
</script>

<div class="refresh">
  <div class="row">
    {#if status.succeededAt !== null}
      <time datetime={new Date(status.succeededAt).toISOString()} title={new Date(status.succeededAt).toLocaleString()}>
        {updatedLabel(status.succeededAt, Math.max(now, status.succeededAt))}
      </time>
    {/if}
    <button {title} aria-busy={status.refreshing} onclick={refresh}>
      {status.refreshing ? 'Refreshing…' : 'Refresh'}
    </button>
  </div>
  {#if status.failure !== null}
    <p role="alert">Refresh failed. {describeError(status.failure.error, status.failure.what)}</p>
  {/if}
</div>

<style>
  .refresh {
    display: flex;
    flex: 0 1 auto;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    min-width: 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  time {
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  button {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 10px;
    white-space: nowrap;
  }
  button:hover {
    background: var(--bg-hover);
  }
  button[aria-busy='true'] {
    color: var(--text-muted);
  }
  p {
    margin: 0;
    max-width: 40ch;
    font-size: 12px;
    color: var(--danger);
    text-align: right;
  }
</style>
