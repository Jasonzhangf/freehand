import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../apps/freehand-server/assets/webui.js', import.meta.url), 'utf8');
const functionMatch = source.match(/export function classifyLayoutShape[\s\S]*?\n}\n\nfunction isMobileDrawerLayout/);
assert(functionMatch, 'classifyLayoutShape export must exist before DOM bindings');

const functionSource = functionMatch[0]
  .replace('export function classifyLayoutShape', 'function classifyLayoutShape')
  .replace('export function classifyLayoutShapeForClient', 'function classifyLayoutShapeForClient')
  .replace('\n\nfunction isMobileDrawerLayout', '');

const { classifyLayoutShape, classifyLayoutShapeForClient } = Function(
  `${functionSource}; return { classifyLayoutShape, classifyLayoutShapeForClient };`,
)();

const cases = [
  { width: 375, height: 812, expected: 'tall_phone' },
  { width: 430, height: 932, expected: 'tall_phone' },
  { width: 844, height: 390, expected: 'phone_landscape' },
  { width: 768, height: 1024, expected: 'tablet_portrait' },
  { width: 1024, height: 768, expected: 'foldable_unfolded' },
  { width: 900, height: 1000, expected: 'foldable_unfolded' },
  { width: 1280, height: 900, expected: 'desktop_large' },
];

const clientCases = [
  { width: 980, height: 1882, client: 'android-webview', expected: 'tablet_portrait' },
  { width: 412, height: 915, client: 'android-webview', expected: 'tall_phone' },
  { width: 915, height: 412, client: 'android-webview', expected: 'phone_landscape' },
];

for (const item of cases) {
  assert.equal(
    classifyLayoutShape(item.width, item.height),
    item.expected,
    `${item.width}x${item.height}`,
  );
}

for (const item of clientCases) {
  assert.equal(
    classifyLayoutShapeForClient(item.width, item.height, item.client),
    item.expected,
    `${item.client}:${item.width}x${item.height}`,
  );
}

console.log(JSON.stringify({ ok: true, cases, clientCases }, null, 2));
