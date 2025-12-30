#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const zlib = require('zlib');

const VERSION = '2.0.0';
const REPO = 'RandomsUsernames/Searchgrep';

function getPlatform() {
  const platform = process.platform;
  const arch = process.arch;

  const platforms = {
    'darwin-x64': 'x86_64-apple-darwin',
    'darwin-arm64': 'aarch64-apple-darwin',
    'linux-x64': 'x86_64-unknown-linux-gnu',
    'linux-arm64': 'aarch64-unknown-linux-gnu',
    'win32-x64': 'x86_64-pc-windows-msvc',
  };

  const key = `${platform}-${arch}`;
  const target = platforms[key];

  if (!target) {
    console.error(`Unsupported platform: ${key}`);
    console.error('Supported platforms:', Object.keys(platforms).join(', '));
    process.exit(1);
  }

  return { platform, arch, target };
}

function download(url) {
  return new Promise((resolve, reject) => {
    const request = (url) => {
      https.get(url, { headers: { 'User-Agent': 'searchgrep-installer' } }, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          request(res.headers.location);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`Download failed: ${res.statusCode}`));
          return;
        }
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => resolve(Buffer.concat(chunks)));
        res.on('error', reject);
      }).on('error', reject);
    };
    request(url);
  });
}

async function install() {
  const { platform, target } = getPlatform();
  const binDir = path.join(__dirname, '..', 'bin');
  const ext = platform === 'win32' ? '.exe' : '';
  const binPath = path.join(binDir, `searchgrep${ext}`);

  console.log(`Installing searchgrep for ${target}...`);

  // Try to download pre-built binary
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/searchgrep-${target}.tar.gz`;

  try {
    console.log(`Downloading from ${url}`);
    const data = await download(url);

    // Extract tar.gz
    const unzipped = zlib.gunzipSync(data);

    // Simple tar extraction (just get the binary)
    // tar format: 512-byte header blocks followed by file content
    let offset = 0;
    while (offset < unzipped.length) {
      const header = unzipped.slice(offset, offset + 512);
      if (header[0] === 0) break; // End of archive

      const fileName = header.slice(0, 100).toString().replace(/\0/g, '').trim();
      const sizeOctal = header.slice(124, 136).toString().replace(/\0/g, '').trim();
      const fileSize = parseInt(sizeOctal, 8) || 0;

      offset += 512; // Move past header

      if (fileName === 'searchgrep' || fileName.endsWith('/searchgrep')) {
        const content = unzipped.slice(offset, offset + fileSize);
        fs.writeFileSync(binPath, content);
        fs.chmodSync(binPath, 0o755);
        console.log(`Installed searchgrep to ${binPath}`);

        // Create MCP wrapper
        const mcpPath = path.join(binDir, `searchgrep-mcp${ext}`);
        if (platform === 'win32') {
          fs.writeFileSync(mcpPath, `@echo off\n"${binPath}" mcp-server %*`);
        } else {
          fs.writeFileSync(mcpPath, `#!/bin/sh\nexec "${binPath}" mcp-server "$@"\n`);
          fs.chmodSync(mcpPath, 0o755);
        }
        console.log(`Installed searchgrep-mcp to ${mcpPath}`);

        console.log('\nInstallation complete!');
        console.log('\nQuick start:');
        console.log('  searchgrep watch .          # Index current directory');
        console.log('  searchgrep search "query"   # Semantic search');
        console.log('  searchgrep --help           # See all options');
        return;
      }

      // Move to next file (aligned to 512 bytes)
      offset += Math.ceil(fileSize / 512) * 512;
    }

    throw new Error('Binary not found in archive');
  } catch (err) {
    console.log(`Pre-built binary not available: ${err.message}`);
    console.log('Attempting to build from source...');

    // Try cargo install
    try {
      execSync('cargo --version', { stdio: 'ignore' });
      console.log('Rust found, building from source...');
      execSync(`cargo install --git https://github.com/${REPO}.git`, { stdio: 'inherit' });

      // Create symlinks to cargo bin
      const cargoBin = path.join(process.env.HOME || process.env.USERPROFILE, '.cargo', 'bin', `searchgrep${ext}`);
      if (fs.existsSync(cargoBin)) {
        fs.copyFileSync(cargoBin, binPath);
        fs.chmodSync(binPath, 0o755);

        const mcpPath = path.join(binDir, `searchgrep-mcp${ext}`);
        if (platform === 'win32') {
          fs.writeFileSync(mcpPath, `@echo off\n"${binPath}" mcp-server %*`);
        } else {
          fs.writeFileSync(mcpPath, `#!/bin/sh\nexec "${binPath}" mcp-server "$@"\n`);
          fs.chmodSync(mcpPath, 0o755);
        }
        console.log('Build complete!');
      }
    } catch (cargoErr) {
      console.error('\nFailed to install searchgrep.');
      console.error('Please install Rust (https://rustup.rs) and try again, or');
      console.error('download a pre-built binary from:');
      console.error(`  https://github.com/${REPO}/releases`);
      process.exit(1);
    }
  }
}

install().catch((err) => {
  console.error('Installation failed:', err.message);
  process.exit(1);
});
