import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

const SIGN_IN = new URL('../src/lib/components/SignIn.svelte', import.meta.url);

/** The text of each step in the client ID form's numbered list, markup removed. */
async function clientIdSteps() {
  const source = await readFile(SIGN_IN, 'utf8');
  const form = source.match(/<form class="client-id"[\s\S]*?<ol>([\s\S]*?)<\/ol>/);
  assert.ok(form, 'SignIn.svelte has the client ID form with a numbered list');
  return [...form[1].matchAll(/<li>([\s\S]*?)<\/li>/g)].map((step) =>
    step[1].replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim(),
  );
}

test('client ID steps follow the order of GitHub\'s Register a new OAuth app form', async () => {
  const steps = await clientIdSteps();

  const order = [
    'Register a new OAuth app',
    'Redirect URI',
    'Enable Device Flow',
    'Expire user access tokens',
    'Client ID',
  ].map((setting) => steps.findIndex((step) => step.includes(setting)));

  assert.equal(steps.length, 5);
  assert.deepEqual(order, [0, 1, 2, 3, 4], `steps were: ${JSON.stringify(steps, null, 2)}`);
});
