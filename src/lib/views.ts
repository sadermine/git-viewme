import type { Component } from 'svelte';
import type { Repo } from './types';
import type { RepoTabState } from './workspace.svelte';
import FilesView from './views/FilesView.svelte';
import ProjectView from './views/ProjectView.svelte';

export type ViewProps = {
  repo: Repo;
  tab: RepoTabState;
  update: (change: Partial<RepoTabState>) => void;
};

/** The views inside a repository tab. The header's view buttons render from this list, so a new
 *  view is one entry here and one component. */
export const VIEWS = [
  { id: 'files', label: 'Files', component: FilesView },
  { id: 'project', label: 'Project', component: ProjectView },
] as const satisfies readonly { id: string; label: string; component: Component<ViewProps> }[];

export type ViewId = (typeof VIEWS)[number]['id'];
