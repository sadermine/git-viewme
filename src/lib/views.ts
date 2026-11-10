import type { Component } from 'svelte';
import type { Refreshable } from './refresh';
import type { Repo } from './types';
import type { RepoTabState } from './workspace.svelte';
import FilesView from './views/FilesView.svelte';
import IssuesView from './views/IssuesView.svelte';
import ProjectView from './views/ProjectView.svelte';
import PullRequestsView from './views/PullRequestsView.svelte';

export type ViewProps = {
  repo: Repo;
  tab: RepoTabState;
  update: (change: Partial<RepoTabState>) => void;
  /** A view calls this once with the loads its content comes from. The header's Refresh refreshes
   *  them and shows their status. A null entry is a load the view does not have right now. */
  track: (loads: () => readonly (Refreshable | null)[]) => void;
};

/** The views inside a repository tab. The header's view buttons render from this list, so a new
 *  view is one entry here and one component. */
export const VIEWS = [
  { id: 'files', label: 'Files', component: FilesView },
  { id: 'project', label: 'Project', component: ProjectView },
  { id: 'issues', label: 'Issues', component: IssuesView },
  { id: 'pulls', label: 'Pull requests', component: PullRequestsView },
] as const satisfies readonly { id: string; label: string; component: Component<ViewProps> }[];

export type ViewId = (typeof VIEWS)[number]['id'];
