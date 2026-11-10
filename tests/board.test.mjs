import assert from 'node:assert/strict';
import { test } from 'node:test';
import { placeCard, withPendingMoves } from '../src/lib/board.ts';

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
