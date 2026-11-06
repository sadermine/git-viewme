import * as api from './api';
import type { CoreError } from './types';

export type Session = { state: 'checking' } | { state: 'signed_out' } | { state: 'signed_in' };

export type LoadState<T> =
  | { kind: 'loading' }
  | { kind: 'ready'; value: T }
  | { kind: 'failed'; error: CoreError };

class SessionStore {
  current = $state<Session>({ state: 'checking' });

  async check(): Promise<void> {
    try {
      this.current = { state: (await api.signedIn()) ? 'signed_in' : 'signed_out' };
    } catch {
      this.current = { state: 'signed_out' };
    }
  }

  signedIn(): void {
    this.current = { state: 'signed_in' };
  }

  /** Core has already deleted the token when a request comes back unauthorized. */
  lost(): void {
    this.current = { state: 'signed_out' };
  }

  async signOut(): Promise<void> {
    await api.signOut();
    this.current = { state: 'signed_out' };
  }
}

export const session = new SessionStore();

/** One request's state for a view. A view creates one when it is first shown and calls `retry`
 *  from its Retry button. An unauthorized answer ends the session. */
export class Load<T> {
  current = $state<LoadState<T>>({ kind: 'loading' });
  #fetch: () => Promise<T>;
  #attempt = 0;

  constructor(fetch: () => Promise<T>) {
    this.#fetch = fetch;
    void this.#run();
  }

  retry(): void {
    this.current = { kind: 'loading' };
    void this.#run();
  }

  async #run(): Promise<void> {
    const attempt = ++this.#attempt;
    let next: LoadState<T>;
    try {
      next = { kind: 'ready', value: await this.#fetch() };
    } catch (error) {
      next = { kind: 'failed', error: api.asCoreError(error) };
    }
    if (attempt !== this.#attempt) return;
    this.current = next;
    if (next.kind === 'failed' && next.error.kind === 'unauthorized') session.lost();
  }
}
