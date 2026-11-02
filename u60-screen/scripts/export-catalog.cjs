#!/usr/bin/env node
// Keep the browser design and native feature index in sync. No Node dependency at build time.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const context = vm.createContext({ window: { U60Appearance: { defaults: { accent: '#2563eb' } } } });
for (const file of ['catalog.js', 'catalog-apps.js']) {
  vm.runInContext(fs.readFileSync(path.join(root, 'preview', file), 'utf8'), context, { timeout: 1000 });
}
const { groups, features } = context.window.U60Catalog;
const additions = JSON.parse(fs.readFileSync(path.join(root, 'assets/native-features.json'), 'utf8'));
const output = JSON.stringify({ groups, features: [...features, ...additions] }, null, 2) + '\n';
const target = path.join(root, 'assets/catalog.json');
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== output) {
    console.error('Catalog changed. Run node u60-screen/scripts/export-catalog.cjs');
    process.exit(1);
  }
} else {
  fs.writeFileSync(target, output);
}
console.log(`${groups.length} groups, ${features.length + additions.length} native feature entries`);
