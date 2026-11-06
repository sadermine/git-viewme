// The domain types of crates/core, as the interface receives them. Field names match the Rust ones.

export type RepoRef = { owner: string; name: string };

export type ProjectRef = { owner: string; number: number; title: string };

export type Repo = {
  ref: RepoRef;
  default_branch: string | null;
  html_url: string;
  private: boolean;
  linked_projects: ProjectRef[];
};

export type TreeEntryKind = 'file' | 'dir';

export type TreeEntry = { path: string; kind: TreeEntryKind; size: number | null };

export type FileContent =
  | { kind: 'text'; text: string }
  | { kind: 'binary'; size: number }
  | { kind: 'too_large'; size: number };

export type Card = { title: string; number: number | null; url: string; labels: string[] };

export type Column = { name: string; cards: Card[] };

export type Board = { columns: Column[] };

export type PullRequestState = 'open' | 'closed';

export type PullRequestStatus = 'open' | 'draft' | 'merged' | 'closed';

export type PullRequest = {
  number: number;
  title: string;
  author: string;
  labels: string[];
  head: string;
  base: string;
  status: PullRequestStatus;
  updated_at: string;
  html_url: string;
};

export type DevicePrompt = { user_code: string; verification_uri: string; expires_in: number };

export type CoreError =
  | { kind: 'unauthorized' }
  | { kind: 'rate_limited'; reset_at: number }
  | { kind: 'network' }
  | { kind: 'not_found' }
  | { kind: 'no_client_id' }
  | { kind: 'client_id_rejected' }
  | { kind: 'other'; message: string };

export function repoKey(ref: RepoRef): string {
  return `${ref.owner}/${ref.name}`;
}
