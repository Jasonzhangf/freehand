import assert from 'node:assert/strict';

const {
  classifyLayoutShape,
  classifyLayoutShapeForClient,
} = await import('../apps/freehand-server/assets/webui/app-shell/layout-shape.js');

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
