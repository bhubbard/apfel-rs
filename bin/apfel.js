#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const bundledBin = path.join(__dirname, 'apfel-bin');
let binaryPath = bundledBin;

if (!fs.existsSync(binaryPath)) {
  const localRelease = path.join(__dirname, '..', 'target', 'release', 'apfel');
  if (fs.existsSync(localRelease)) {
    binaryPath = localRelease;
  } else {
    binaryPath = 'apfel';
  }
}

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit'
});

child.on('error', (err) => {
  if (err.code === 'ENOENT') {
    console.error('Error: apfel native binary not found.');
    console.error('Please ensure you are running on macOS with Apple Silicon (arm64).');
  } else {
    console.error(err);
  }
  process.exit(1);
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 0);
  }
});
