import * as api from './api';
import {
  begin,
  firstLoad,
  settle,
  SingleFlight,
  type Loaded,
  type Outcome,
  type Refreshable,
  type RefreshStatus,
} from './refresh';

export type Session = { state: 'checking' } | { state: 'signed_out' } | { state: 'signed_in' };

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

/** One view's data, fetched when the view is first shown and again on `refresh`. A refresh keeps
 *  the value on screen until the new one arrives, and one asked for while another runs joins it.
 *  An unauthorized answer ends the session. */
export class Load<T> implements Refreshable {
  current = $state<Loaded<T>>(firstLoad());
  readonly what: string;
  #fetch: () => Promise<T>;
  #flight = new SingleFlight();

  /** `what` names the data in messages, as in "Loading the {what}". */
  constructor(what: string, fetch: () => Promise<T>) {
    this.what = what;
    this.#fetch = fetch;
    void this.#flight.run(() => this.#fetchAndSettle());
  }

  get status(): RefreshStatus {
    const { refreshing, refreshError, finishedAt, succeededAt } = this.current;
    return { refreshing, failure: refreshError && { what: this.what, error: refreshError }, finishedAt, succeededAt };
  }

  refresh(): Promise<void> {
    return this.#flight.run(() => {
      this.current = begin(this.current);
      return this.#fetchAndSettle();
    });
  }

  async #fetchAndSettle(): Promise<void> {
    let outcome: Outcome<T>;
    try {
      outcome = { ok: true, value: await this.#fetch() };
    } catch (error) {
      outcome = { ok: false, error: api.asCoreError(error) };
    }
    this.current = settle(this.current, outcome, Date.now());
    if (!outcome.ok && outcome.error.kind === 'unauthorized') session.lost();
  }
}
