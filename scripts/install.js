#!/usr/bin/env node
'use strict';

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const BIN_DIR = path.join(ROOT, 'bin');
const RELEASE_BIN = path.join(ROOT, 'target', 'release', 'remote-bridge');
const DEST_BIN = path.join(BIN_DIR, 'remote-bridge');

function hasCargo() {
  try {
    execSync('cargo --version', { stdio: 'ignore' });
    return true;
  } catch (_) {
    return false;
  }
}

function build() {
  console.log('Building remote-bridge from source (this may take a minute)...');
  execSync('cargo build --release', { cwd: ROOT, stdio: 'inherit' });
}

function copyBinary() {
  if (!fs.existsSync(BIN_DIR)) {
    fs.mkdirSync(BIN_DIR, { recursive: true });
  }
  fs.copyFileSync(RELEASE_BIN, DEST_BIN);
  fs.chmodSync(DEST_BIN, 0o755);
  console.log('remote-bridge installed to', DEST_BIN);
}

// If the release binary already exists (prepublishOnly ran it), just copy.
if (fs.existsSync(RELEASE_BIN)) {
  copyBinary();
} else if (hasCargo()) {
  build();
  copyBinary();
} else {
  console.error(
    'Error: Rust/Cargo is not installed.\n' +
    'Please install Rust from https://rustup.rs/ and re-run npm install.'
  );
  process.exit(1);
}
