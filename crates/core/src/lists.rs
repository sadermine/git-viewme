use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use crate::{Issue, IssueState, PullRequest, PullRequestState};

/// An item of a cached list: an issue or a pull request.
pub trait Listed: Clone + PartialEq {
    fn number(&self) -> u32;
    fn updated_at(&self) -> &str;
}

impl Listed for Issue {
    fn number(&self) -> u32 {
        self.number
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }
}

impl Listed for PullRequest {
    fn number(&self) -> u32 {
        self.number
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }
}

/// One repository's issue or pull request lists, one per state, kept between fetches so that a
/// list is fetched in full once and only its changes after that.
pub struct ListCache<S, T> {
    lists: HashMap<S, List<T>>,
    since: Option<String>,
    etag: Option<String>,
}

struct List<T> {
    items: BTreeMap<u32, T>,
    /// GitHub's count minus the list's length, taken by `settle` after each full fetch.
    offset: i64,
}

pub type IssueCache = ListCache<IssueState, Issue>;
pub type PullRequestCache = ListCache<PullRequestState, PullRequest>;

impl<S, T> Default for ListCache<S, T> {
    fn default() -> Self {
        Self {
            lists: HashMap::new(),
            since: None,
            etag: None,
        }
    }
}

impl<S: Copy + Eq + Hash, T: Listed> ListCache<S, T> {
    /// The cached list for `state`, most recently updated first, or `None` before its first fetch.
    pub fn list(&self, state: S) -> Option<Vec<T>> {
        let mut items: Vec<T> = self.lists.get(&state)?.items.values().cloned().collect();
        items.sort_by(|a, b| {
            b.updated_at()
                .cmp(a.updated_at())
                .then_with(|| b.number().cmp(&a.number()))
        });
        Some(items)
    }

    /// Stores a full fetch of `state`. `since` is the GitHub time the fetch started at, and the
    /// earliest one held is kept, because asking for changes from too early only repeats some.
    pub fn replace(&mut self, state: S, items: Vec<T>, since: String) {
        let items = items
            .into_iter()
            .map(|item| (item.number(), item))
            .collect();
        self.lists.insert(state, List { items, offset: 0 });
        if self.since.as_ref().is_none_or(|held| since < *held) {
            self.since = Some(since);
            self.etag = None;
        }
    }

    /// Merges changed items, each with its state now: it joins that state's list if that list is
    /// cached and leaves every other. Returns whether any list changed.
    pub fn merge(&mut self, changes: impl IntoIterator<Item = (S, T)>) -> bool {
        let mut changed = false;
        for (state, item) in changes {
            for (held, list) in &mut self.lists {
                if *held == state {
                    if list.items.get(&item.number()) != Some(&item) {
                        list.items.insert(item.number(), item.clone());
                        changed = true;
                    }
                } else if list.items.remove(&item.number()).is_some() {
                    changed = true;
                }
            }
        }
        changed
    }

    /// Whether `state`'s cached list disagrees with GitHub's `count` of items in that state, so a
    /// deletion or a transfer, which no change request returns, has happened.
    pub fn count_differs(&self, state: S, count: usize) -> bool {
        self.lists
            .get(&state)
            .is_some_and(|list| i64::try_from(count).unwrap_or(i64::MAX) - list.offset != len(list))
    }

    /// Records how far GitHub's `count` is from `state`'s list just after its full fetch, once the
    /// changes made while it paged are merged. Some issues are counted but never listed, and this
    /// keeps them from making the count differ on every refresh.
    pub fn settle(&mut self, state: S, count: usize) {
        if let Some(list) = self.lists.get_mut(&state) {
            list.offset = i64::try_from(count).unwrap_or(i64::MAX) - len(list);
        }
    }

    /// The GitHub time to ask for changes after. `None` until a list is fetched.
    pub fn since(&self) -> Option<&str> {
        self.since.as_deref()
    }

    /// Records a change answer to a request whose URL carries `since`. `since` moves on only when
    /// the answer ran past one page, whatever it held, so the next refresh sends the same URL and
    /// its ETag earns the free `304`. Pull requests on the issues endpoint change no list but still
    /// count as pages.
    pub fn answered(&mut self, pages: usize, since: String, etag: Option<String>) {
        if pages > 1 {
            self.advance(since);
        } else {
            self.etag = etag;
        }
    }

    /// Moves `since` on. The stored ETag was for the old `since`, so it goes too.
    pub fn advance(&mut self, since: String) {
        self.since = Some(since);
        self.etag = None;
    }

    /// The ETag of the last change request, sent back so an unchanged list answers `304`.
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    pub fn set_etag(&mut self, etag: Option<String>) {
        self.etag = etag;
    }
}

fn len<T>(list: &List<T>) -> i64 {
    i64::try_from(list.items.len()).unwrap_or(i64::MAX)
}
