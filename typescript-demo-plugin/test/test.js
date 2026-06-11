const { existsSync, statSync } = require('node:fs');
const { join } = require('node:path');

const root = join(__dirname, '..');
const requiredArtifacts = [
  'dist/lib.js',
  'dist/lib.d.ts',
  'dist/plugin.wasm',
];

for (const artifact of requiredArtifacts) {
  const path = join(root, artifact);
  if (!existsSync(path)) {
    throw new Error(`Missing build artifact: ${artifact}`);
  }

  if (statSync(path).size === 0) {
    throw new Error(`Build artifact is empty: ${artifact}`);
  }
}

console.log('TypeScript demo plugin artifacts verified');
