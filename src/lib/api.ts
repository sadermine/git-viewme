// The only file that talks to the Tauri side. A hosted version replaces this file with HTTP calls.
import { Channel, invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import type {
  Board,
  CardMove,
  CoreError,
  DevicePrompt,
  FileContent,
  Issue,
  IssueState,
  ProjectRef,
  PullRequest,
  PullRequestState,
  Repo,
  RepoRef,
  TreeEntry,
} from './types';

const ERROR_KINDS: ReadonlySet<string> = new Set<CoreError['kind']>([
  'unauthorized',
  'rate_limited',
  'network',
  'not_found',
  'missing_scope',
  'forbidden',
  'no_client_id',
  'client_id_rejected',
  'other',
]);

function isCoreError(value: unknown): value is CoreError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    typeof value.kind === 'string' &&
    ERROR_KINDS.has(value.kind)
  );
}

/** Every rejection from this module is a CoreError, whatever the IPC layer threw. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw asCoreError(error);
  }
}

export function asCoreError(error: unknown): CoreError {
  return isCoreError(error) ? error : { kind: 'other', message: String(error) };
}

export const signedIn = () => call<boolean>('signed_in');

/** The saved OAuth app client ID, or null when the user has not entered one. */
export const clientId = () => call<string | null>('client_id');

/** Validates and saves the client ID, and resolves to it as saved (trimmed). */
export const saveClientId = (clientId: string) => call<string>('save_client_id', { clientId });

export function signIn(onPrompt: (prompt: DevicePrompt) => void): Promise<void> {
  const channel = new Channel<DevicePrompt>();
  channel.onmessage = onPrompt;
  return call('sign_in', { onPrompt: channel });
}

export const signOut = () => call<void>('sign_out');

export const listRepos = () => call<Repo[]>('list_repos');

export const fileTree = (repo: RepoRef, branch: string) =>
  call<TreeEntry[]>('file_tree', { repo, branch });

export const fileContent = (repo: RepoRef, branch: string, path: string) =>
  call<FileContent>('file_content', { repo, branch, path });

export const projectBoard = (project: ProjectRef) => call<Board>('project_board', { project });

export const moveCard = (cardMove: CardMove) => call<void>('move_card', { cardMove });

export const listIssues = (repo: RepoRef, state: IssueState) =>
  call<Issue[]>('list_issues', { repo, state });

export const listPullRequests = (repo: RepoRef, state: PullRequestState) =>
  call<PullRequest[]>('list_pull_requests', { repo, state });

/** Opens a GitHub page in the system browser. The capability allows https://github.com only. */
export const openExternal = (url: string) => openUrl(url);
