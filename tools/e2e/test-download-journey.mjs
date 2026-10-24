import { startServer, FIXTURE_SIZE, FIXTURE_SHA256 } from './serve-fixture.mjs';
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

function runProcessAsync(cmd, args, stdio) {
  return new Promise((resolve, reject) => {
    const proc = spawn(cmd, args, { stdio });
    proc.on('error', reject);
    proc.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`Command ${cmd} exited with code ${code}`));
    });
  });
}

async function main() {
  console.log('--- SAYL Real Download Journey ---');
  console.log(`Fixture size: ${FIXTURE_SIZE} bytes`);
  console.log(`Fixture SHA256: ${FIXTURE_SHA256}`);

  const port = 8765;
  const fixture = await startServer(port);
  console.log(`Fixture server listening at ${fixture.url}`);

  const repoRoot = path.resolve('.');
  const saylExe = path.join(repoRoot, 'target/release/sayl.exe');
  const dlDir = path.join(repoRoot, 'target/journey-downloads');
  const dataDir = path.join(repoRoot, 'target/journey-data');

  if (fs.existsSync(dlDir)) fs.rmSync(dlDir, { recursive: true, force: true });
  if (fs.existsSync(dataDir)) fs.rmSync(dataDir, { recursive: true, force: true });
  fs.mkdirSync(dlDir, { recursive: true });
  fs.mkdirSync(dataDir, { recursive: true });

  try {
    // 1. Add URL
    console.log(`\n[1/3] Adding URL: ${fixture.url}`);
    const tempAddOut = path.join(dataDir, 'add-out.json');
    const fdAdd = fs.openSync(tempAddOut, 'w');
    await runProcessAsync(
      saylExe,
      ['add', fixture.url, '--download-dir', dlDir, '--data-dir', dataDir, '--json'],
      ['ignore', fdAdd, 'inherit']
    );
    fs.closeSync(fdAdd);
    const addOutput = fs.readFileSync(tempAddOut, 'utf8');
    console.log('Add response:', addOutput.trim());
    const tasks = JSON.parse(addOutput);
    const taskId = tasks[0];
    console.log(`Created Task ID: ${taskId}`);

    // 2. Start Download
    console.log(`\n[2/3] Starting download for task ${taskId}...`);
    await runProcessAsync(
      saylExe,
      ['start', taskId, '--download-dir', dlDir, '--data-dir', dataDir],
      ['ignore', 'inherit', 'inherit']
    );
    console.log('Download execution completed.');

    // 3. Inspect Info
    const tempInfoOut = path.join(dataDir, 'info-out.json');
    const fdInfo = fs.openSync(tempInfoOut, 'w');
    await runProcessAsync(
      saylExe,
      ['info', taskId, '--download-dir', dlDir, '--data-dir', dataDir, '--json'],
      ['ignore', fdInfo, 'inherit']
    );
    fs.closeSync(fdInfo);
    const infoOutput = fs.readFileSync(tempInfoOut, 'utf8');
    const info = JSON.parse(infoOutput);
    console.log(`Task state: ${info.state}, written: ${info.written}/${info.total_size}`);

    if (info.state !== 'Completed') {
      throw new Error(`Expected task state Completed, got: ${info.state}`);
    }

    // 4. Verify downloaded file on disk
    console.log('\n[3/3] Verifying downloaded file on disk...');
    const downloadedFilePath = path.join(dlDir, info.filename || 'sayl-fixture-24mb.bin');
    if (!fs.existsSync(downloadedFilePath)) {
      throw new Error(`Downloaded file does not exist at: ${downloadedFilePath}`);
    }

    const stat = fs.statSync(downloadedFilePath);
    console.log(`Downloaded file path: ${downloadedFilePath}`);
    console.log(`Downloaded file size: ${stat.size} bytes (expected: ${FIXTURE_SIZE})`);
    if (stat.size !== FIXTURE_SIZE) {
      throw new Error(`Size mismatch: got ${stat.size}, expected ${FIXTURE_SIZE}`);
    }

    const fileBuffer = fs.readFileSync(downloadedFilePath);
    const hash = crypto.createHash('sha256').update(fileBuffer).digest('hex');
    console.log(`Downloaded file SHA256: ${hash}`);
    console.log(`Fixture expected SHA256: ${FIXTURE_SHA256}`);

    if (hash.toLowerCase() !== FIXTURE_SHA256.toLowerCase()) {
      throw new Error(`SHA256 mismatch! Got: ${hash}, expected: ${FIXTURE_SHA256}`);
    }

    console.log('\n✅ REAL DOWNLOAD JOURNEY PASSED WITH CRYPTOGRAPHIC MATCH!');
    process.exit(0);
  } finally {
    fixture.server.close();
  }
}

main().catch((err) => {
  console.error('\n❌ Download journey failed:', err);
  process.exit(1);
});
