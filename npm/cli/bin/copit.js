#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const path = require('node:path');

function isMusl() {
  try {
    return !process.report.getReport().header.glibcVersionRuntime;
  } catch {
    return false;
  }
}

function candidates() {
  const key = `${process.platform}-${process.arch}`;
  if (process.platform !== 'linux') return [key];
  return isMusl() ? [`${key}-musl`, key] : [key, `${key}-musl`];
}

function binary() {
  if (process.env.COPIT_BINARY) return process.env.COPIT_BINARY;
  const name = process.platform === 'win32' ? 'copit.exe' : 'copit';
  for (const key of candidates()) {
    try {
      const manifest = require.resolve(`@copit/cli-${key}/package.json`);
      return path.join(path.dirname(manifest), 'bin', name);
    } catch {}
  }
  console.error(
    `copit: no prebuilt binary for ${process.platform}-${process.arch}. ` +
      'Reinstall without --omit=optional, or use https://github.com/huynguyengl99/copit#installation.',
  );
  process.exit(1);
}

const result = spawnSync(binary(), process.argv.slice(2), { stdio: 'inherit' });
if (result.error) throw result.error;
if (result.signal) process.kill(process.pid, result.signal);
process.exit(result.status ?? 1);
