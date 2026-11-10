// What a view's data is and what its last refresh did. Plain TypeScript with no Svelte or Tauri
// import, so `npm test` runs it under Node directly.
import type { CoreError } from './types';

export type LoadState<T> =
  | { kind: 'loading' }
  | { kind: 'ready'; value: T }
  | { kind: 'failed'; error: CoreError };

export type Loaded<T> = {
  state: LoadState<T>;
  refreshing: boolean;
  /** Why the last refresh failed while the previous value stayed on screen. */
  refreshError: CoreError | null;
  /** When the last refresh finished, whether it succeeded or failed. Milliseconds since the epoch. */
  finishedAt: number | null;
  succeededAt: number | null;
};

export type Outcome<T> = { ok: true; value: T } | { ok: false; error: CoreError };

/** A first load: nothing on screen yet, its request already sent. */
export function firstLoad<T>(): Loaded<T> {
  return { state: { kind: 'loading' }, refreshing: true, refreshError: null, finishedAt: null, succeededAt: null };
}

/** A refresh starts. A value on screen stays there; anything else shows the loading placeholder. */
export function begin<T>(loaded: Loaded<T>): Loaded<T> {
  return {
    ...loaded,
    state: loaded.state.kind === 'ready' ? loaded.state : { kind: 'loading' },
    refreshing: true,
  };
}

export function settle<T>(loaded: Loaded<T>, outcome: Outcome<T>, at: number): Loaded<T> {
  if (outcome.ok) {
    return { state: { kind: 'ready', value: outcome.value }, refreshing: false, refreshError: null, finishedAt: at, succeededAt: at };
  }
  const kept = loaded.state.kind === 'ready';
  return {
    ...loaded,
    state: kept ? loaded.state : { kind: 'failed', error: outcome.error },
    refreshing: false,
    refreshError: kept ? outcome.error : null,
    finishedAt: at,
  };
}

/** Runs one task at a time. A run asked for while one is going gets that run's promise and starts
 *  nothing, so repeated refreshes send one request. */
export class SingleFlight {
  #running: Promise<void> | null = null;

  run(task: () => Promise<void>): Promise<void> {
    this.#running ??= task().finally(() => {
      this.#running = null;
    });
    return this.#running;
  }
}

export type RefreshStatus = {
  refreshing: boolean;
  failure: { what: string; error: CoreError } | null;
  finishedAt: number | null;
  succeededAt: number | null;
};

export interface Refreshable {
  readonly status: RefreshStatus;
  refresh(): Promise<void>;
}

const oldest = (times: (number | null)[]): number | null =>
  times.length === 0 || times.includes(null) ? null : Math.min(...(times as number[]));

/** The status of a view made of several loads. Its times are the oldest of theirs, because the
 *  view is only as fresh as its stalest part. */
export function combine(statuses: readonly RefreshStatus[]): RefreshStatus {
  return {
    refreshing: statuses.some((status) => status.refreshing),
    failure: statuses.find((status) => status.failure !== null)?.failure ?? null,
    finishedAt: oldest(statuses.map((status) => status.finishedAt)),
    succeededAt: oldest(statuses.map((status) => status.succeededAt)),
  };
}

export function updatedLabel(at: number, now: number): string {
  const minutes = Math.floor((now - at) / 60_000);
  if (minutes < 1) return 'Updated less than a minute ago';
  if (minutes < 60) return `Updated ${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Updated ${hours} h ago`;
  return `Updated ${Math.floor(hours / 24)} d ago`;
}
