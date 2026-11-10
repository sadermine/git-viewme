// The domain types of crates/core, as the interface receives them. Field names match the Rust ones.

export type RepoRef = { owner: string; name: string };

export type ProjectRef = { owner: string; number: number; title: string; url: string };

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

export type Card = { item_id: string; title: string; number: number | null; url: string; labels: string[] };

/** `option_id` is null for "No status". */
export type Column = { name: string; option_id: string | null; cards: Card[] };

/** `status_field_id` is null when the project has no Status field, and then no card moves. */
export type Board = { project_id: string; status_field_id: string | null; columns: Column[] };

/** Sets a card's Status to `option_id`. No card moves to "No status". */
export type CardMove = { project_id: string; item_id: string; field_id: string; option_id: string };

export type IssueState = 'open' | 'closed';

export type Issue = {
  number: number;
  title: string;
  author: string;
  labels: string[];
  updated_at: string;
  html_url: string;
};

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
  | { kind: 'missing_scope' }
  | { kind: 'forbidden'; message: string }
  | { kind: 'no_client_id' }
  | { kind: 'client_id_rejected' }
  | { kind: 'other'; message: string };

export function repoKey(ref: RepoRef): string {
  return `${ref.owner}/${ref.name}`;
}
