import { SvelteMap } from 'svelte/reactivity';
import { repoKey, type RepoRef } from './types';
import type { ViewId } from './views';

export type RepoTabState = { view: ViewId; selectedPath: string | null };

/** The open repository tabs. One lives for each signed-in session and is dropped on sign-out. */
export class Workspace {
  openTabs = $state<RepoRef[]>([]);
  activeTab = $state<RepoRef | null>(null);
  /** Keyed by "owner/name". */
  tabState = new SvelteMap<string, RepoTabState>();

  isActive(ref: RepoRef): boolean {
    return this.activeTab !== null && repoKey(this.activeTab) === repoKey(ref);
  }

  isOpen(ref: RepoRef): boolean {
    return this.tabState.has(repoKey(ref));
  }

  open(ref: RepoRef): void {
    if (!this.isOpen(ref)) {
      this.openTabs.push(ref);
      this.tabState.set(repoKey(ref), { view: 'files', selectedPath: null });
    }
    this.activeTab = ref;
  }

  close(ref: RepoRef): void {
    const key = repoKey(ref);
    const index = this.openTabs.findIndex((tab) => repoKey(tab) === key);
    if (index === -1) return;
    this.openTabs.splice(index, 1);
    this.tabState.delete(key);
    if (this.activeTab !== null && repoKey(this.activeTab) === key) {
      this.activeTab = this.openTabs[Math.min(index, this.openTabs.length - 1)] ?? null;
    }
  }

  update(ref: RepoRef, change: Partial<RepoTabState>): void {
    const key = repoKey(ref);
    const state = this.tabState.get(key);
    if (state) this.tabState.set(key, { ...state, ...change });
  }
}
