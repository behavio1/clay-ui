// Creates the npm package that carries one prebuilt binary.
//   node scripts/build-platform-package.mjs <platform-key> <binary-path> <output-root>
// Example: node scripts/build-platform-package.mjs darwin-arm64 target/release/clayui dist-npm
import { chmodSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TARGETS = {
  'darwin-arm64': { os: 'darwin', cpu: 'arm64', file: 'clayui' },
  'darwin-x64': { os: 'darwin', cpu: 'x64', file: 'clayui' },
  'linux-x64': { os: 'linux', cpu: 'x64', file: 'clayui' },
  'linux-arm64': { os: 'linux', cpu: 'arm64', file: 'clayui' },
  'win32-x64': { os: 'win32', cpu: 'x64', file: 'clayui.exe' },
};

const [key, binary, outputRoot] = process.argv.slice(2);
const target = TARGETS[key];
if (!target || !binary || !outputRoot) {
  console.error('Usage: build-platform-package.mjs <platform-key> <binary-path> <output-root>');
  process.exit(2);
}

const root = fileURLToPath(new URL('..', import.meta.url));
const main = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const name = `clayui-${key}`;
const folder = path.join(outputRoot, name);
mkdirSync(folder, { recursive: true });
const destination = path.join(folder, target.file);
copyFileSync(binary, destination);
chmodSync(destination, 0o755);
copyFileSync(path.join(root, 'LICENSE'), path.join(folder, 'LICENSE'));
copyFileSync(path.join(root, 'THIRD_PARTY_NOTICES.md'), path.join(folder, 'THIRD_PARTY_NOTICES.md'));
writeFileSync(path.join(folder, 'README.md'), `# ${name}\n\nThe prebuilt Clay UI binary for ${key}. Install \`clayui\` instead of this package.\n`);
writeFileSync(path.join(folder, 'package.json'), `${JSON.stringify({
  name,
  version: main.version,
  description: `Prebuilt Clay UI binary for ${key}.`,
  license: main.license,
  repository: main.repository,
  os: [target.os],
  cpu: [target.cpu],
  files: [target.file, 'README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md'],
}, null, 2)}\n`);
console.log(folder);
