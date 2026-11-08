import type { CoreError } from './types';

/** The sentence a view shows for a failed request. `what` names the thing that failed to load. */
export function describeError(error: CoreError, what: string): string {
  switch (error.kind) {
    case 'rate_limited': {
      const at = new Date(error.reset_at * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
      return `GitHub's rate limit is used up. It resets at ${at}.`;
    }
    case 'network':
      return 'GitHub could not be reached. Check the network connection, then retry.';
    case 'not_found':
      return `GitHub could not find the ${what}. It may have moved, or this account cannot see it.`;
    case 'missing_scope':
      return 'This sign-in does not grant the access git-viewme needs. Sign out and sign in again.';
    case 'forbidden':
      return `You do not have permission to change the ${what}.`;
    case 'no_client_id':
      return 'Enter the client ID of your GitHub OAuth app first.';
    case 'client_id_rejected':
      return 'GitHub rejected this client ID. Check that it is copied in full and that the OAuth app has Enable Device Flow ticked.';
    case 'unauthorized':
      return 'GitHub no longer accepts this sign-in.';
    case 'other':
      return error.message;
  }
}
