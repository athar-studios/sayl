import http from 'node:http';
import crypto from 'node:crypto';
import path from 'node:path';

export const FIXTURE_SIZE = 24 * 1024 * 1024; // 24 MiB = 25,165,824 bytes
export const FIXTURE_FILENAME = 'sayl-fixture-24mb.bin';
export const ETAG = '"sayl-fixture-24mb-v1"';

export function generateFixtureBuffer() {
  const buf = Buffer.alloc(FIXTURE_SIZE);
  const chunkSize = 64 * 1024; // 64 KiB
  const chunkPattern = Buffer.alloc(chunkSize);
  for (let i = 0; i < chunkSize; i++) {
    chunkPattern[i] = (i * 31 + (i >> 8) * 17 + 101) & 0xff;
  }
  for (let offset = 0; offset < FIXTURE_SIZE; offset += chunkSize) {
    chunkPattern.copy(buf, offset);
    const blockIndex = offset / chunkSize;
    buf[offset] ^= blockIndex & 0xff;
    buf[offset + 1] ^= (blockIndex >> 8) & 0xff;
  }
  return buf;
}

export const fixtureBuffer = generateFixtureBuffer();
export const FIXTURE_SHA256 = crypto.createHash('sha256').update(fixtureBuffer).digest('hex');

export const requestLogs = [];

export function createFixtureServer() {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host || '127.0.0.1'}`);

    if (url.pathname === '/health') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ status: 'ok', size: FIXTURE_SIZE, sha256: FIXTURE_SHA256 }));
      return;
    }

    if (url.pathname === '/requests') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(requestLogs));
      return;
    }

    if (url.pathname === '/reset-requests') {
      requestLogs.length = 0;
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ status: 'cleared' }));
      return;
    }

    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405, { 'Content-Type': 'text/plain' });
      res.end('Method Not Allowed');
      return;
    }

    const rangeHeader = req.headers['range'];

    requestLogs.push({
      time: Date.now(),
      method: req.method,
      path: url.pathname,
      range: rangeHeader || null,
      query: Object.fromEntries(url.searchParams.entries()),
    });

    const isThrottled = url.searchParams.has('delay') || url.pathname.includes('/slow/');
    const delayPerChunk = parseInt(url.searchParams.get('delay') || '25', 10);

    const commonHeaders = {
      'Accept-Ranges': 'bytes',
      'ETag': ETAG,
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': `attachment; filename="${FIXTURE_FILENAME}"`,
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    };

    if (!rangeHeader) {
      res.writeHead(200, {
        ...commonHeaders,
        'Content-Length': FIXTURE_SIZE,
      });
      if (req.method === 'HEAD') {
        res.end();
      } else if (!isThrottled) {
        res.end(fixtureBuffer);
      } else {
        await streamThrottled(res, fixtureBuffer, delayPerChunk);
      }
      return;
    }

    const match = /^bytes=(\d*)-(\d*)$/.exec(rangeHeader.trim());
    if (!match) {
      res.writeHead(416, { 'Content-Range': `bytes */${FIXTURE_SIZE}` });
      res.end('Invalid Range');
      return;
    }

    let start = match[1] ? parseInt(match[1], 10) : undefined;
    let end = match[2] ? parseInt(match[2], 10) : undefined;

    if (start === undefined && end !== undefined) {
      start = FIXTURE_SIZE - end;
      end = FIXTURE_SIZE - 1;
    } else if (start !== undefined && end === undefined) {
      end = FIXTURE_SIZE - 1;
    }

    if (start === undefined || end === undefined || start > end || start >= FIXTURE_SIZE || start < 0) {
      res.writeHead(416, { 'Content-Range': `bytes */${FIXTURE_SIZE}` });
      res.end('Range Not Satisfiable');
      return;
    }

    end = Math.min(end, FIXTURE_SIZE - 1);
    const chunkLength = end - start + 1;

    res.writeHead(206, {
      ...commonHeaders,
      'Content-Range': `bytes ${start}-${end}/${FIXTURE_SIZE}`,
      'Content-Length': chunkLength,
    });

    if (req.method === 'HEAD') {
      res.end();
    } else if (!isThrottled) {
      res.end(fixtureBuffer.subarray(start, end + 1));
    } else {
      await streamThrottled(res, fixtureBuffer.subarray(start, end + 1), delayPerChunk);
    }
  });

  return server;
}

async function streamThrottled(res, buffer, delayMs) {
  const blockSize = 64 * 1024; // 64 KiB
  for (let offset = 0; offset < buffer.length; offset += blockSize) {
    if (res.writableEnded || res.destroyed) break;
    const end = Math.min(offset + blockSize, buffer.length);
    res.write(buffer.subarray(offset, end));
    await new Promise((r) => setTimeout(r, delayMs));
  }
  if (!res.writableEnded && !res.destroyed) {
    res.end();
  }
}

export function startServer(port = 8765, host = '127.0.0.1') {
  return new Promise((resolve, reject) => {
    const server = createFixtureServer();
    server.listen(port, host, () => {
      const address = server.address();
      const actualPort = typeof address === 'object' && address ? address.port : port;
      resolve({
        server,
        port: actualPort,
        url: `http://${host}:${actualPort}/${FIXTURE_FILENAME}`,
        slowUrl: `http://${host}:${actualPort}/slow/${FIXTURE_FILENAME}`,
        size: FIXTURE_SIZE,
        sha256: FIXTURE_SHA256,
      });
    });
    server.on('error', reject);
  });
}

if (process.argv[1] && import.meta.filename && path.resolve(process.argv[1]) === path.resolve(import.meta.filename)) {
  const port = parseInt(process.argv[2] || '8765', 10);
  startServer(port).then(({ port, url, size, sha256 }) => {
    console.log(`[FIXTURE_SERVER_READY] url=${url} port=${port} size=${size} sha256=${sha256}`);
  }).catch((err) => {
    console.error('[FIXTURE_SERVER_ERROR]', err);
    process.exit(1);
  });
}
