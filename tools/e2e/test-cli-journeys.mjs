import { startServer, FIXTURE_SIZE, FIXTURE_SHA256 } from './serve-fixture.mjs';
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

function runSaylAsync(saylExe, args, outFile = null) {
  return new Promise((resolve, reject) => {
    let stdio = ['ignore', 'inherit', 'inherit'];
    let fd = null;
    if (outFile) {
      fd = fs.openSync(outFile, 'w');
      stdio = ['ignore', fd, 'inherit'];
    }
    const proc = spawn(saylExe, args, { stdio });
    proc.on('error', (err) => {
      if (fd !== null) fs.closeSync(fd);
      reject(err);
    });
    proc.on('exit', (code) => {
      if (fd !== null) fs.closeSync(fd);
      if (code === 0) resolve(outFile ? fs.readFileSync(outFile, 'utf8') : '');
      else reject(new Error(`sayl ${args.join(' ')} failed with code ${code}`));
    });
  });
}

async function main() {
  console.log('============================================================');
  console.log(' SAYL CLI Runtime Journeys & Recovery Verification');
  console.log('============================================================');

  const port = 8769;
  const fixture = await startServer(port);
  console.log(`[FIXTURE] Server ready at ${fixture.url}`);
  console.log(`[FIXTURE] Size: ${FIXTURE_SIZE} bytes | SHA256: ${FIXTURE_SHA256}`);

  const repoRoot = path.resolve('.');
  const saylExe = path.join(repoRoot, 'target/release/sayl.exe');
  const baseDir = path.join(repoRoot, 'target/cli-journeys');
  if (fs.existsSync(baseDir)) fs.rmSync(baseDir, { recursive: true, force: true });
  fs.mkdirSync(baseDir, { recursive: true });

  try {
    // ------------------------------------------------------------------------
    // Journey 1A: sayl pause on Queued task & Resume to completion
    // ------------------------------------------------------------------------
    console.log('\n--- Journey 1A: sayl pause on Queued task & Resume ---');
    const dlDir1A = path.join(baseDir, 'j1a-downloads');
    const dataDir1A = path.join(baseDir, 'j1a-data');
    fs.mkdirSync(dlDir1A, { recursive: true });
    fs.mkdirSync(dataDir1A, { recursive: true });

    const addOut1A = path.join(dataDir1A, 'add.json');
    await runSaylAsync(saylExe, ['add', fixture.url, '--download-dir', dlDir1A, '--data-dir', dataDir1A, '--json'], addOut1A);
    const taskId1A = JSON.parse(fs.readFileSync(addOut1A, 'utf8'))[0];
    console.log(`[J1A] Task created: ${taskId1A}`);

    // Verify state before pause
    const prePauseOut = path.join(dataDir1A, 'info-prepause.json');
    await runSaylAsync(saylExe, ['info', taskId1A, '--download-dir', dlDir1A, '--data-dir', dataDir1A, '--json'], prePauseOut);
    const prePauseInfo = JSON.parse(fs.readFileSync(prePauseOut, 'utf8'));
    console.log(`[J1A] State before pause: ${prePauseInfo.state}`);
    if (prePauseInfo.state !== 'Queued') {
      throw new Error(`Expected state Queued, got: ${prePauseInfo.state}`);
    }

    // Execute sayl pause
    console.log(`[J1A] Executing "sayl pause ${taskId1A}"...`);
    await runSaylAsync(saylExe, ['pause', taskId1A, '--download-dir', dlDir1A, '--data-dir', dataDir1A]);

    // Verify state after pause
    const postPauseOut = path.join(dataDir1A, 'info-postpause.json');
    await runSaylAsync(saylExe, ['info', taskId1A, '--download-dir', dlDir1A, '--data-dir', dataDir1A, '--json'], postPauseOut);
    const postPauseInfo = JSON.parse(fs.readFileSync(postPauseOut, 'utf8'));
    console.log(`[J1A] State after pause: ${postPauseInfo.state}`);
    if (postPauseInfo.state !== 'Paused') {
      throw new Error(`Expected state Paused, got: ${postPauseInfo.state}`);
    }

    // Resume via sayl start
    console.log(`[J1A] Resuming via "sayl start ${taskId1A}"...`);
    await runSaylAsync(saylExe, ['start', taskId1A, '--download-dir', dlDir1A, '--data-dir', dataDir1A]);

    // Verify completed
    const finalInfoOut = path.join(dataDir1A, 'info-final.json');
    await runSaylAsync(saylExe, ['info', taskId1A, '--download-dir', dlDir1A, '--data-dir', dataDir1A, '--json'], finalInfoOut);
    const finalInfo = JSON.parse(fs.readFileSync(finalInfoOut, 'utf8'));
    console.log(`[J1A] State after start: ${finalInfo.state} (written: ${finalInfo.written}/${finalInfo.total_size})`);
    if (finalInfo.state !== 'Completed') {
      throw new Error(`Expected state Completed, got: ${finalInfo.state}`);
    }
    console.log('✅ Journey 1A (sayl pause & resume) PASSED!');

    // ------------------------------------------------------------------------
    // Journey 1B: Mid-download Interruption & Safe Resume without re-fetching
    // ------------------------------------------------------------------------
    console.log('\n--- Journey 1B: Mid-download Interruption & Safe Resume ---');
    const dlDir1B = path.join(baseDir, 'j1b-downloads');
    const dataDir1B = path.join(baseDir, 'j1b-data');
    fs.mkdirSync(dlDir1B, { recursive: true });
    fs.mkdirSync(dataDir1B, { recursive: true });

    const addOut1B = path.join(dataDir1B, 'add.json');
    await runSaylAsync(saylExe, ['add', fixture.slowUrl, '--download-dir', dlDir1B, '--data-dir', dataDir1B, '--json'], addOut1B);
    const taskId1B = JSON.parse(fs.readFileSync(addOut1B, 'utf8'))[0];
    console.log(`[J1B] Throttled task created: ${taskId1B}`);

    console.log('[J1B] Starting throttled download process...');
    const dlProc1B = spawn(saylExe, ['start', taskId1B, '--download-dir', dlDir1B, '--data-dir', dataDir1B], {
      stdio: ['ignore', 'inherit', 'inherit'],
    });

    // Wait until partial download is active
    await new Promise((r) => setTimeout(r, 800));
    console.log('[J1B] Terminating download process mid-transfer...');
    dlProc1B.kill();
    await new Promise((resolve) => dlProc1B.on('exit', resolve));
    await new Promise((r) => setTimeout(r, 200));

    // Reconcile and recover
    console.log('[J1B] Running "sayl recover"...');
    await runSaylAsync(saylExe, ['recover', '--download-dir', dlDir1B, '--data-dir', dataDir1B]);

    const partialInfoOut = path.join(dataDir1B, 'info-partial.json');
    await runSaylAsync(saylExe, ['info', taskId1B, '--download-dir', dlDir1B, '--data-dir', dataDir1B, '--json'], partialInfoOut);
    const partialInfo = JSON.parse(fs.readFileSync(partialInfoOut, 'utf8'));
    console.log(`[J1B] Recovered task state: ${partialInfo.state}, written: ${partialInfo.written} bytes`);

    // Reset request logs on server to record resumed range requests
    await fetch(`http://127.0.0.1:${port}/reset-requests`);

    // Resume task to completion
    console.log('[J1B] Resuming task to completion via "sayl start"...');
    await runSaylAsync(saylExe, ['start', taskId1B, '--download-dir', dlDir1B, '--data-dir', dataDir1B]);

    // Check request logs
    const reqRes = await fetch(`http://127.0.0.1:${port}/requests`);
    const reqLogs = await reqRes.json();
    console.log(`[J1B] HTTP requests recorded during resume: ${reqLogs.length}`);
    for (const r of reqLogs) {
      if (r.range) {
        console.log(`     Range: ${r.range}`);
      }
    }

    const compInfoOut = path.join(dataDir1B, 'info-comp.json');
    await runSaylAsync(saylExe, ['info', taskId1B, '--download-dir', dlDir1B, '--data-dir', dataDir1B, '--json'], compInfoOut);
    const compInfo = JSON.parse(fs.readFileSync(compInfoOut, 'utf8'));
    console.log(`[J1B] Final state: ${compInfo.state}, total written: ${compInfo.written}`);
    if (compInfo.state !== 'Completed') {
      throw new Error(`Expected state Completed, got: ${compInfo.state}`);
    }

    const downloaded1B = path.join(dlDir1B, compInfo.filename);
    const hash1B = crypto.createHash('sha256').update(fs.readFileSync(downloaded1B)).digest('hex');
    console.log(`[J1B] SHA256 of completed file: ${hash1B}`);
    if (hash1B.toLowerCase() !== FIXTURE_SHA256.toLowerCase()) {
      throw new Error('Hash mismatch after resume!');
    }
    console.log('✅ Journey 1B (Mid-download interrupt & Safe Resume) PASSED!');

    // ------------------------------------------------------------------------
    // Journey 2: sayl cancel Verification
    // ------------------------------------------------------------------------
    console.log('\n--- Journey 2: sayl cancel Verification ---');
    const dlDir2 = path.join(baseDir, 'j2-downloads');
    const dataDir2 = path.join(baseDir, 'j2-data');
    fs.mkdirSync(dlDir2, { recursive: true });
    fs.mkdirSync(dataDir2, { recursive: true });

    const addOut2 = path.join(dataDir2, 'add.json');
    await runSaylAsync(saylExe, ['add', fixture.url, '--download-dir', dlDir2, '--data-dir', dataDir2, '--json'], addOut2);
    const taskId2 = JSON.parse(fs.readFileSync(addOut2, 'utf8'))[0];
    console.log(`[J2] Task created: ${taskId2}`);

    const preCancelOut = path.join(dataDir2, 'info-precancel.json');
    await runSaylAsync(saylExe, ['info', taskId2, '--download-dir', dlDir2, '--data-dir', dataDir2, '--json'], preCancelOut);
    const preCancelInfo = JSON.parse(fs.readFileSync(preCancelOut, 'utf8'));
    console.log(`[J2] State before cancel: ${preCancelInfo.state}`);

    console.log(`[J2] Executing "sayl cancel ${taskId2}"...`);
    await runSaylAsync(saylExe, ['cancel', taskId2, '--download-dir', dlDir2, '--data-dir', dataDir2]);

    const postCancelOut = path.join(dataDir2, 'info-postcancel.json');
    await runSaylAsync(saylExe, ['info', taskId2, '--download-dir', dlDir2, '--data-dir', dataDir2, '--json'], postCancelOut);
    const postCancelInfo = JSON.parse(fs.readFileSync(postCancelOut, 'utf8'));
    console.log(`[J2] State after cancel: ${postCancelInfo.state}`);
    if (postCancelInfo.state !== 'Cancelled') {
      throw new Error(`Expected state Cancelled, got: ${postCancelInfo.state}`);
    }
    console.log('✅ Journey 2 (sayl cancel) PASSED!');

    // ------------------------------------------------------------------------
    // Journey 3: Process Crash & sayl recover Verification
    // ------------------------------------------------------------------------
    console.log('\n--- Journey 3: Process Crash & sayl recover ---');
    const dlDir3 = path.join(baseDir, 'j3-downloads');
    const dataDir3 = path.join(baseDir, 'j3-data');
    fs.mkdirSync(dlDir3, { recursive: true });
    fs.mkdirSync(dataDir3, { recursive: true });

    const addOut3 = path.join(dataDir3, 'add.json');
    await runSaylAsync(saylExe, ['add', fixture.slowUrl, '--download-dir', dlDir3, '--data-dir', dataDir3, '--json'], addOut3);
    const taskId3 = JSON.parse(fs.readFileSync(addOut3, 'utf8'))[0];
    console.log(`[J3] Task created: ${taskId3}`);

    console.log('[J3] Spawning download process...');
    const crashProc = spawn(saylExe, ['start', taskId3, '--download-dir', dlDir3, '--data-dir', dataDir3], {
      stdio: ['ignore', 'inherit', 'inherit'],
    });

    await new Promise((r) => setTimeout(r, 600));
    console.log('[J3] Killing process to simulate sudden crash...');
    crashProc.kill();
    await new Promise((resolve) => crashProc.on('exit', resolve));
    await new Promise((r) => setTimeout(r, 200));

    console.log('[J3] Running "sayl recover" in fresh process...');
    await runSaylAsync(saylExe, ['recover', '--download-dir', dlDir3, '--data-dir', dataDir3]);

    console.log('[J3] Resuming task to completion...');
    await runSaylAsync(saylExe, ['start', taskId3, '--download-dir', dlDir3, '--data-dir', dataDir3]);

    const recFinalOut = path.join(dataDir3, 'info-recovered.json');
    await runSaylAsync(saylExe, ['info', taskId3, '--download-dir', dlDir3, '--data-dir', dataDir3, '--json'], recFinalOut);
    const recFinal = JSON.parse(fs.readFileSync(recFinalOut, 'utf8'));
    console.log(`[J3] Recovered state: ${recFinal.state}`);
    if (recFinal.state !== 'Completed') {
      throw new Error(`Expected state Completed, got: ${recFinal.state}`);
    }

    const recFile = path.join(dlDir3, recFinal.filename);
    const recStat = fs.statSync(recFile);
    const recHash = crypto.createHash('sha256').update(fs.readFileSync(recFile)).digest('hex');
    console.log(`[J3] Recovered file size: ${recStat.size} bytes (expected: ${FIXTURE_SIZE})`);
    console.log(`[J3] Recovered file SHA256: ${recHash}`);
    if (recHash.toLowerCase() !== FIXTURE_SHA256.toLowerCase()) {
      throw new Error('Hash mismatch after crash recovery!');
    }
    console.log('✅ Journey 3 (Crash & sayl recover) PASSED!');

    // ------------------------------------------------------------------------
    // Journey 4: Custom --download-dir
    // ------------------------------------------------------------------------
    console.log('\n--- Journey 4: Custom --download-dir Folder ---');
    const customFolder = path.join(baseDir, 'custom-folder-destination');
    const dataDir4 = path.join(baseDir, 'j4-data');
    fs.mkdirSync(customFolder, { recursive: true });
    fs.mkdirSync(dataDir4, { recursive: true });

    const addOut4 = path.join(dataDir4, 'add.json');
    await runSaylAsync(saylExe, ['add', fixture.url, '--download-dir', customFolder, '--data-dir', dataDir4, '--json'], addOut4);
    const taskId4 = JSON.parse(fs.readFileSync(addOut4, 'utf8'))[0];
    console.log(`[J4] Task added to custom folder: ${taskId4}`);

    await runSaylAsync(saylExe, ['start', taskId4, '--download-dir', customFolder, '--data-dir', dataDir4]);

    const targetFile = path.join(customFolder, 'sayl-fixture-24mb.bin');
    if (!fs.existsSync(targetFile)) {
      throw new Error(`File was not found in custom download dir: ${targetFile}`);
    }
    const customStat = fs.statSync(targetFile);
    const customHash = crypto.createHash('sha256').update(fs.readFileSync(targetFile)).digest('hex');
    console.log(`[J4] File successfully landed at: ${targetFile}`);
    console.log(`[J4] Custom folder file size: ${customStat.size}`);
    console.log(`[J4] Custom folder file hash: ${customHash}`);
    if (customHash.toLowerCase() !== FIXTURE_SHA256.toLowerCase()) {
      throw new Error('Hash mismatch on custom folder file!');
    }
    console.log('✅ Journey 4 (Custom --download-dir) PASSED!');

    console.log('\n============================================================');
    console.log(' ALL CLI RUNTIME JOURNEYS VERIFIED & PASSED (Exit Code: 0)');
    console.log('============================================================');
    process.exit(0);
  } finally {
    fixture.server.close();
  }
}

main().catch((err) => {
  console.error('\n❌ CLI Journeys failed:', err);
  process.exit(1);
});
