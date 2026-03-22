#!/usr/bin/env node
'use strict';

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const binaryPath = path.join(__dirname, 'remote-bridge');

if (!fs.existsSync(binaryPath)) {
  console.error(
    'remote-bridge binary not found. Try reinstalling: npm install -g remote-bridge-cli'
  );
  process.exit(1);
}

const child = spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
child.on('exit', (code) => process.exit(code ?? 0));
