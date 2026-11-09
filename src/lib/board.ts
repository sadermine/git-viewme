// Card placement on a board the interface already holds. Plain TypeScript so `npm test` runs it.
import type { Board } from './types';

/** Moves a card to the lane with `optionId`, at `index` or at the end. Does nothing and returns
 *  false when the card or the lane is not on the board. */
export function placeCard(board: Board, itemId: string, optionId: string | null, index?: number): boolean {
  const from = board.columns.find((column) => column.cards.some((card) => card.item_id === itemId));
  const to = board.columns.find((column) => column.option_id === optionId);
  if (from === undefined || to === undefined) return false;
  const [card] = from.cards.splice(
    from.cards.findIndex((each) => each.item_id === itemId),
    1,
  );
  to.cards.splice(Math.min(index ?? to.cards.length, to.cards.length), 0, card);
  return true;
}

/** A board fetched while moves were waiting for GitHub may still show those cards in their old
 *  lanes. Puts each one in the lane it is moving to, so a refresh never undoes a move on screen. */
export function withPendingMoves(board: Board, pending: Readonly<Record<string, string>>): Board {
  for (const [itemId, optionId] of Object.entries(pending)) {
    const lane = board.columns.find((column) => column.cards.some((card) => card.item_id === itemId));
    if (lane?.option_id !== optionId) placeCard(board, itemId, optionId);
  }
  return board;
}
