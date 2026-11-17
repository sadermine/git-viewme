import assert from 'node:assert/strict';
import { test } from 'node:test';
import { begin, chain, combine, firstLoad, settle, SingleFlight, updatedLabel } from '../src/lib/refresh.ts';

const NETWORK = { kind: 'network' };

function loadedWith(value, at) {
  return settle(firstLoad(), { ok: true, value }, at);
}

test('a refresh keeps the value on screen until the new one arrives', () => {
  const refreshing = begin(loadedWith(['#1'], 1_000));
  assert.deepEqual(refreshing.state, { kind: 'ready', value: ['#1'] });
  assert.equal(refreshing.refreshing, true);

  const done = settle(refreshing, { ok: true, value: ['#1', '#2'] }, 2_000);
  assert.deepEqual(done.state, { kind: 'ready', value: ['#1', '#2'] });
  assert.equal(done.refreshing, false);
  assert.equal(done.finishedAt, 2_000);
  assert.equal(done.succeededAt, 2_000);
});

test('a failed refresh keeps the value, reports why and records only the finish time', () => {
  const failed = settle(begin(loadedWith(['#1'], 1_000)), { ok: false, error: NETWORK }, 2_000);
  assert.deepEqual(failed.state, { kind: 'ready', value: ['#1'] });
  assert.deepEqual(failed.refreshError, NETWORK);
  assert.equal(failed.finishedAt, 2_000);
  assert.equal(failed.succeededAt, 1_000);

  const recovered = settle(begin(failed), { ok: true, value: ['#1'] }, 3_000);
  assert.equal(recovered.refreshError, null);
  assert.equal(recovered.succeededAt, 3_000);
});

test('a failed first load shows the failure in place of the data, and its refresh shows loading', () => {
  const failed = settle(firstLoad(), { ok: false, error: NETWORK }, 1_000);
  assert.deepEqual(failed.state, { kind: 'failed', error: NETWORK });
  assert.equal(failed.refreshError, null);
  assert.equal(failed.succeededAt, null);
  assert.deepEqual(begin(failed).state, { kind: 'loading' });
});

test('refreshes asked for while one runs send one request', async () => {
  const flight = new SingleFlight();
  let requests = 0;
  let answer;
  const task = () => {
    requests += 1;
    return new Promise((resolve) => (answer = resolve));
  };

  const clicks = Array.from({ length: 5 }, () => flight.run(task));
  assert.equal(requests, 1);
  answer();
  await Promise.all(clicks);

  const next = flight.run(task);
  assert.equal(requests, 2, 'a refresh after the last one finished sends a new request');
  answer();
  await next;
});

test('a view made of several loads is as fresh as its oldest part', () => {
  const tree = { refreshing: false, failure: null, finishedAt: 5_000, succeededAt: 5_000 };
  const file = { refreshing: true, failure: { what: 'file', error: NETWORK }, finishedAt: 3_000, succeededAt: 2_000 };
  assert.deepEqual(combine([tree, file]), { refreshing: true, failure: file.failure, finishedAt: 3_000, succeededAt: 2_000 });

  const neverLoaded = { refreshing: true, failure: null, finishedAt: null, succeededAt: null };
  assert.equal(combine([tree, neverLoaded]).succeededAt, null);
  assert.equal(combine([]).succeededAt, null);
});

test('the Updated label counts whole minutes', () => {
  assert.equal(updatedLabel(0, 59_999), 'Updated less than a minute ago');
  assert.equal(updatedLabel(0, 2 * 60_000), 'Updated 2 min ago');
  assert.equal(updatedLabel(0, 59 * 60_000 + 59_999), 'Updated 59 min ago');
  assert.equal(updatedLabel(0, 3 * 3_600_000), 'Updated 3 h ago');
  assert.equal(updatedLabel(0, 2 * 86_400_000), 'Updated 2 d ago');
});

/** A hand-rolled load. Each refresh logs its start and end and waits for `answer` before it
 *  settles at the time `at` holds then. */
function fakeLoad(name, log, status) {
  const load = {
    status,
    refreshes: 0,
    at: null,
    answer: null,
    refresh() {
      load.refreshes += 1;
      log.push(`${name} start`);
      return new Promise((resolve) => {
        load.answer = () => {
          log.push(`${name} end`);
          load.status = { refreshing: false, failure: null, finishedAt: load.at, succeededAt: load.at };
          resolve();
        };
      });
    },
  };
  return load;
}

const settled = (at) => ({ refreshing: false, failure: null, finishedAt: at, succeededAt: at });
const tick = () => new Promise((resolve) => setImmediate(resolve));

test('a load the first one drops is never requested and leaves the status', async () => {
  const log = [];
  const tree = fakeLoad('tree', log, settled(1_000));
  const file = fakeLoad('file', log, { refreshing: false, failure: { what: 'file', error: NETWORK }, finishedAt: 2_000, succeededAt: 500 });
  let listed = true;
  const files = chain(tree, () => (listed ? file : null));

  const done = files.refresh();
  tree.at = 3_000;
  listed = false;
  tree.answer();
  await done;

  assert.equal(file.refreshes, 0);
  assert.equal(files.status.failure, null);
  assert.equal(files.status.succeededAt, 3_000);
});

test('a load still listed is refreshed after the first settles, not alongside it', async () => {
  const log = [];
  const tree = fakeLoad('tree', log, settled(1_000));
  const file = fakeLoad('file', log, settled(1_000));
  const files = chain(tree, () => file);

  const done = files.refresh();
  assert.deepEqual(log, ['tree start']);
  tree.at = 2_000;
  tree.answer();
  await tick();
  assert.deepEqual(log, ['tree start', 'tree end', 'file start']);
  file.at = 2_500;
  file.answer();
  await done;
  assert.equal(files.status.succeededAt, 2_000);
});

test('refreshes asked for while a chain runs refresh each load once', async () => {
  const log = [];
  const tree = fakeLoad('tree', log, settled(1_000));
  const file = fakeLoad('file', log, settled(1_000));
  const files = chain(tree, () => file);

  const clicks = Array.from({ length: 5 }, () => files.refresh());
  tree.answer();
  await tick();
  file.answer();
  await Promise.all(clicks);

  assert.equal(tree.refreshes, 1);
  assert.equal(file.refreshes, 1);
});
