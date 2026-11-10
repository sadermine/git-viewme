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
    <button {title} aria-label={status.refreshing ? 'Refreshing' : title} aria-busy={status.refreshing} onclick={refresh}>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
        <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        <path d="M13.5 1.5v3.5H10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
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
    display: flex;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 5px;
  }
  button:hover {
    background: var(--bg-hover);
  }
  button[aria-busy='true'] {
    color: var(--text-muted);
  }
  button[aria-busy='true'] svg {
    animation: spin 1s linear infinite;
  }
  @media (prefers-reduced-motion: reduce) {
    button[aria-busy='true'] svg {
      animation: none;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  p {
    margin: 0;
    max-width: 40ch;
    font-size: 12px;
    color: var(--danger);
    text-align: right;
  }
</style>
