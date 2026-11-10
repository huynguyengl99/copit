#!/usr/bin/env node
// Build the npm packages from release archives: one per platform, plus @copit/cli.
//
//   node npm/stage.mjs <version> <artifacts-dir> <out-dir> [--partial]

import { execFileSync } from 'node:child_process';
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const TARGETS = {
  'aarch64-apple-darwin': { os: 'darwin', cpu: 'arm64' },
  'x86_64-apple-darwin': { os: 'darwin', cpu: 'x64' },
  'x86_64-unknown-linux-gnu': { os: 'linux', cpu: 'x64', libc: 'glibc' },
  'aarch64-unknown-linux-gnu': { os: 'linux', cpu: 'arm64', libc: 'glibc' },
  'armv7-unknown-linux-gnueabihf': { os: 'linux', cpu: 'arm', libc: 'glibc' },
  'x86_64-unknown-linux-musl': { os: 'linux', cpu: 'x64', libc: 'musl' },
  'aarch64-unknown-linux-musl': { os: 'linux', cpu: 'arm64', libc: 'musl' },
  'x86_64-pc-windows-msvc': { os: 'win32', cpu: 'x64' },
  'aarch64-pc-windows-msvc': { os: 'win32', cpu: 'arm64' },
};

const [version, artifacts, out] = process.argv.slice(2, 5).map((arg, i) => (i ? resolve(arg) : arg?.replace(/^v/, '')));
const partial = process.argv.includes('--partial');
if (!version || !artifacts || !out) {
  console.error('usage: node npm/stage.mjs <version> <artifacts-dir> <out-dir> [--partial]');
  process.exit(2);
}

const here = new URL('.', import.meta.url).pathname;
const cli = JSON.parse(readFileSync(join(here, 'cli', 'package.json'), 'utf8'));
const { repository, license, homepage } = cli;
rmSync(out, { recursive: true, force: true });

function find(dir, name) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = find(path, name);
      if (found) return found;
    } else if (entry.name === name) return path;
  }
}

function extract(target, os) {
  const archive = join(artifacts, `copit-${target}.${os === 'win32' ? 'zip' : 'tar.gz'}`);
  if (!existsSync(archive)) return null;
  const scratch = mkdtempSync(join(tmpdir(), 'copit-'));
  if (os === 'win32') execFileSync('unzip', ['-q', archive, '-d', scratch]);
  else execFileSync('tar', ['xzf', archive, '-C', scratch]);
  return { scratch, binary: find(scratch, os === 'win32' ? 'copit.exe' : 'copit') };
}

const optionalDependencies = {};
for (const [target, { os, cpu, libc }] of Object.entries(TARGETS)) {
  const key = `${os}-${cpu}${libc === 'musl' ? '-musl' : ''}`;
  const name = `@copit/cli-${key}`;
  const extracted = extract(target, os);
  if (!extracted?.binary) {
    if (partial) continue;
    throw new Error(`no binary for ${target} in ${artifacts}`);
  }
  const dir = join(out, `cli-${key}`);
  const bin = join(dir, 'bin', os === 'win32' ? 'copit.exe' : 'copit');
  mkdirSync(join(dir, 'bin'), { recursive: true });
  cpSync(extracted.binary, bin);
  chmodSync(bin, 0o755);
  rmSync(extracted.scratch, { recursive: true, force: true });
  const manifest = {
    name,
    version,
    description: `The copit binary for ${target}`,
    license,
    homepage,
    repository,
    os: [os],
    cpu: [cpu],
    ...(libc && { libc: [libc] }),
    files: ['bin'],
    preferUnplugged: true,
  };
  writeFileSync(join(dir, 'package.json'), JSON.stringify(manifest, null, 2) + '\n');
  optionalDependencies[name] = version;
}

const main = join(out, 'cli');
cpSync(join(here, 'cli'), main, { recursive: true });
writeFileSync(join(main, 'package.json'), JSON.stringify({ name: cli.name, version, ...cli, optionalDependencies }, null, 2) + '\n');
console.log(`staged @copit/cli ${version} with ${Object.keys(optionalDependencies).length} platform packages in ${out}`);
