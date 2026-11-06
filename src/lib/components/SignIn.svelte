<script lang="ts">
  import * as api from '$lib/api';
  import { describeError } from '$lib/errors';
  import { session } from '$lib/session.svelte';
  import type { CoreError, DevicePrompt } from '$lib/types';

  const REGISTER_URL = 'https://github.com/settings/applications/new';

  type Step =
    | { kind: 'loading' }
    | { kind: 'entering'; saved: string | null; error: CoreError | null }
    | { kind: 'ready'; clientId: string; error: CoreError | null }
    | { kind: 'starting'; clientId: string }
    | { kind: 'waiting'; clientId: string; prompt: DevicePrompt };

  let step = $state<Step>({ kind: 'loading' });
  let draft = $state('');

  void load();

  async function load() {
    try {
      const clientId = await api.clientId();
      step = clientId === null ? { kind: 'entering', saved: null, error: null } : { kind: 'ready', clientId, error: null };
    } catch (error) {
      step = { kind: 'entering', saved: null, error: api.asCoreError(error) };
    }
  }

  function change(clientId: string) {
    draft = clientId;
    step = { kind: 'entering', saved: clientId, error: null };
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (step.kind !== 'entering') return;
    const saved = step.saved;
    try {
      step = { kind: 'ready', clientId: await api.saveClientId(draft), error: null };
    } catch (error) {
      step = { kind: 'entering', saved, error: api.asCoreError(error) };
    }
  }

  async function signIn(clientId: string) {
    step = { kind: 'starting', clientId };
    try {
      await api.signIn((prompt) => {
        step = { kind: 'waiting', clientId, prompt };
        void api.openExternal(prompt.verification_uri);
      });
      session.signedIn();
    } catch (error) {
      const failure = api.asCoreError(error);
      step = failure.kind === 'no_client_id' ? { kind: 'entering', saved: null, error: failure } : { kind: 'ready', clientId, error: failure };
    }
  }
</script>

<main class="sign-in">
  <h1>git-viewme</h1>
  <p class="lead">Browse your GitHub repositories and their projects. git-viewme only reads; it never changes anything on GitHub.</p>

  {#if step.kind === 'entering'}
    <form class="client-id" onsubmit={save}>
      <p>git-viewme signs in through a GitHub OAuth app that you register on your own account. Register one, then enter its client ID here.</p>
      <ol>
        <li><button type="button" class="link" onclick={() => api.openExternal(REGISTER_URL)}>Register a new OAuth app on GitHub</button>. Any name and homepage URL will do.</li>
        <li>Tick <strong>Enable Device Flow</strong>.</li>
        <li>Leave <strong>Expire user access tokens</strong> unticked. git-viewme reads no refresh token, so an expiring token would send you back to sign-in every 8 hours.</li>
        <li>git-viewme uses no redirect URI. Click <strong>Delete</strong> on the empty <strong>Redirect URI</strong> row. If GitHub still asks for one, enter <span class="mono">http://localhost</span>.</li>
        <li>Copy the <strong>Client ID</strong> from the app's page and enter it below.</li>
      </ol>
      <label>
        Client ID
        <input class="mono" bind:value={draft} spellcheck="false" autocomplete="off" />
      </label>
      <div class="actions">
        <button class="primary" type="submit">Save client ID</button>
        {#if step.saved !== null}
          {@const saved = step.saved}
          <button type="button" class="link" onclick={() => (step = { kind: 'ready', clientId: saved, error: null })}>Cancel</button>
        {/if}
      </div>
      {#if step.error}
        <p class="error" role="alert">{describeError(step.error, 'settings')}</p>
      {/if}
    </form>
  {:else if step.kind === 'waiting'}
    <p>Enter this code on the GitHub page that just opened in your browser:</p>
    <p class="code mono" aria-label="One-time code">{step.prompt.user_code}</p>
    <p class="muted">
      Page did not open?
      <button class="link" onclick={() => step.kind === 'waiting' && api.openExternal(step.prompt.verification_uri)}>
        Open {step.prompt.verification_uri.replace(/^https:\/\//, '')}
      </button>
    </p>
    <p class="muted" role="status">Waiting for you to approve git-viewme on GitHub…</p>
  {:else if step.kind === 'ready' || step.kind === 'starting'}
    {@const clientId = step.clientId}
    <button class="primary" onclick={() => signIn(clientId)} disabled={step.kind === 'starting'}>Sign in with GitHub</button>
    {#if step.kind === 'ready' && step.error}
      <p class="error" role="alert">{describeError(step.error, 'sign-in page')}</p>
    {/if}
    <p class="muted">
      OAuth app client ID <span class="mono">{clientId}</span>.
      <button class="link" onclick={() => change(clientId)}>Change client ID</button>
    </p>
  {/if}
</main>

<style>
  .sign-in {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
    text-align: center;
    overflow: auto;
  }
  h1 {
    margin: 0;
    font-size: 22px;
  }
  p {
    margin: 0;
    max-width: 52ch;
  }
  .lead,
  .muted {
    color: var(--text-muted);
  }
  .client-id {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 60ch;
    text-align: left;
  }
  .client-id p {
    max-width: none;
  }
  ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-weight: 600;
  }
  input {
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    background: var(--bg);
    font-weight: normal;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .primary {
    margin-top: 8px;
    padding: 8px 20px;
    border-radius: 6px;
    background: var(--fill);
    color: var(--on-fill);
    font-weight: 600;
  }
  .primary:disabled {
    cursor: progress;
  }
  .code {
    font-size: 28px;
    letter-spacing: 0.15em;
    padding: 8px 16px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-chrome);
    user-select: all;
  }
  .error {
    color: var(--danger);
  }
</style>
