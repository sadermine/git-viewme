import assert from 'node:assert/strict';
import { test } from 'node:test';
import { moveTargets, placeCard, withPendingMoves } from '../src/lib/board.ts';

const card = (id) => ({ item_id: id, title: id, number: null, url: `https://github.com/o/r/issues/${id}`, labels: [] });

/** A board as GitHub returns it: lane name to the card IDs in it. */
function board(lanes) {
  return {
    project_id: 'P',
    status_field_id: 'F',
    columns: Object.entries(lanes).map(([name, ids]) => ({ name, option_id: name, cards: ids.map(card) })),
  };
}

const lanes = (value) => Object.fromEntries(value.columns.map((column) => [column.name, column.cards.map((each) => each.item_id)]));

test('a board fetched during a move shows the card in the lane it is moving to', () => {
  const fetched = board({ Todo: ['a', 'b'], Done: ['c'] });
  withPendingMoves(fetched, { a: 'Done' });
  assert.deepEqual(lanes(fetched), { Todo: ['b'], Done: ['c', 'a'] });
});

test('a board GitHub already updated keeps the card where GitHub put it', () => {
  const fetched = board({ Todo: ['b'], Done: ['a', 'c'] });
  withPendingMoves(fetched, { a: 'Done' });
  assert.deepEqual(lanes(fetched), { Todo: ['b'], Done: ['a', 'c'] });
});

test('a refused move puts the card back at its old place', () => {
  const shown = board({ Todo: ['a', 'b'], Done: ['c'] });
  placeCard(shown, 'a', 'Done');
  assert.equal(placeCard(shown, 'a', 'Todo', 0), true);
  assert.deepEqual(lanes(shown), { Todo: ['a', 'b'], Done: ['c'] });
});

test('a card or lane that is not on the board is left alone', () => {
  const shown = board({ Todo: ['a'] });
  assert.equal(placeCard(shown, 'a', 'Gone'), false);
  assert.equal(placeCard(shown, 'z', 'Todo'), false);
  assert.deepEqual(lanes(shown), { Todo: ['a'] });
});

/** A board whose lanes are given as [name, option_id, card IDs], so a lane can have no option. */
function boardOf(...columns) {
  return {
    project_id: 'P',
    status_field_id: 'F',
    columns: columns.map(([name, option_id, ids]) => ({ name, option_id, cards: ids.map(card) })),
  };
}

const names = (columns) => columns.map((column) => column.name);

test('a Status field with no options gives a card no lane to move to', () => {
  const shown = boardOf(['No status', null, ['a']]);
  assert.deepEqual(moveTargets(shown, shown.columns[0]), []);
});

test('a card already in the only option lane has no lane to move to', () => {
  const shown = boardOf(['Todo', 'todo', ['a']]);
  assert.deepEqual(moveTargets(shown, shown.columns[0]), []);
});

test('a card with no status can move to every option lane, in board order, and not to its own lane', () => {
  const shown = boardOf(['No status', null, ['a']], ['Todo', 'todo', []], ['Done', 'done', []]);
  assert.deepEqual(names(moveTargets(shown, shown.columns[0])), ['Todo', 'Done']);
});

test('a card in one of two option lanes can move to the other one only', () => {
  const shown = boardOf(['No status', null, ['b']], ['Todo', 'todo', ['a']], ['Done', 'done', []]);
  assert.deepEqual(names(moveTargets(shown, shown.columns[1])), ['Done']);
});
