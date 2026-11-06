<script lang="ts">
  import * as api from '$lib/api';
  import { describeError } from '$lib/errors';
  import { session } from '$lib/session.svelte';
  import type { CoreError, DevicePrompt } from '$lib/types';

  type Step = { kind: 'idle' } | { kind: 'starting' } | { kind: 'waiting'; prompt: DevicePrompt } | { kind: 'failed'; error: CoreError };

  let step = $state<Step>({ kind: 'idle' });

  async function signIn() {
    step = { kind: 'starting' };
    try {
      await api.signIn((prompt) => {
        step = { kind: 'waiting', prompt };
        void api.openExternal(prompt.verification_uri);
      });
      session.signedIn();
    } catch (error) {
      step = { kind: 'failed', error: api.asCoreError(error) };
    }
  }
</script>

<main class="sign-in">
  <h1>git-viewme</h1>
  <p class="lead">Browse your GitHub repositories and their projects. git-viewme only reads; it never changes anything on GitHub.</p>

  {#if step.kind === 'waiting'}
    <p>Enter this code on the GitHub page that just opened in your browser:</p>
    <p class="code mono" aria-label="One-time code">{step.prompt.user_code}</p>
    <p class="muted">
      Page did not open?
      <button class="link" onclick={() => step.kind === 'waiting' && api.openExternal(step.prompt.verification_uri)}>
        Open {step.prompt.verification_uri.replace(/^https:\/\//, '')}
      </button>
    </p>
    <p class="muted" role="status">Waiting for you to approve git-viewme on GitHub…</p>
  {:else}
    <button class="primary" onclick={signIn} disabled={step.kind === 'starting'}>Sign in with GitHub</button>
    {#if step.kind === 'failed'}
      <p class="error" role="alert">{describeError(step.error, 'sign-in page')}</p>
    {/if}
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
