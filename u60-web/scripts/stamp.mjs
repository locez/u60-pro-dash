// Rust verifies the exact source and output set before embedding a build.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, relative } from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
function files(dir) {
  return readdirSync(resolve(root, dir), { withFileTypes: true }).flatMap(e => {
    const path = `${dir}/${e.name}`;
    if (e.isSymbolicLink()) throw new Error(`Unexpected symlink: ${path}`);
    return e.isDirectory() ? files(path) : [path];
  });
}
const inputs = [
  ...files('src'), ...files('scripts'),
  'index.html', 'package.json', 'package-lock.json', 'tsconfig.json', 'vite.config.ts',
  '../locales/web.json',
  ...['cellular', 'clients-model', 'battery-model', 'routing-model', 'data-usage-model',
      'dns-model', 'wireguard-model', 'actions', 'advanced'].map(f => `../zte-agent/ui/${f}.js`),
];
function manifest(paths, base) {
  return paths.sort().map(path => {
    const hash = createHash('sha256').update(readFileSync(resolve(root, path))).digest('hex');
    return `${hash}  ${relative(base, resolve(root, path))}`;
  }).join('\n') + '\n';
}
writeFileSync(resolve(root, 'dist/inputs.sha256'), manifest(inputs, resolve(root, '..')));
writeFileSync(resolve(root, 'dist/assets.sha256'), manifest(
  files('dist').filter(p => !p.endsWith('.sha256')), resolve(root, 'dist')));
