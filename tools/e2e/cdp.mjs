import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

export class CdpSession {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.ws = null;
    this.nextId = 1;
    this.pendingCallbacks = new Map();
    this.eventListeners = new Map();
  }

  async connect() {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(this.wsUrl);
      this.ws.onopen = () => resolve();
      this.ws.onerror = (err) => reject(new Error(`WebSocket error: ${err.message || err}`));
      this.ws.onclose = () => {
        for (const [, { reject }] of this.pendingCallbacks) {
          reject(new Error('WebSocket connection closed'));
        }
        this.pendingCallbacks.clear();
      };
      this.ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          if (msg.id !== undefined && this.pendingCallbacks.has(msg.id)) {
            const { resolve, reject } = this.pendingCallbacks.get(msg.id);
            this.pendingCallbacks.delete(msg.id);
            if (msg.error) {
              reject(new Error(`CDP Error (${msg.error.code}): ${msg.error.message}`));
            } else {
              resolve(msg.result);
            }
          } else if (msg.method) {
            const listeners = this.eventListeners.get(msg.method) || [];
            for (const l of listeners) l(msg.params);
          }
        } catch (e) {
          console.error('[CDP PARSE ERROR]', e);
        }
      };
    });
  }

  send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = this.nextId++;
      this.pendingCallbacks.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  on(event, callback) {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event).push(callback);
  }

  async evaluate(expression, awaitPromise = true) {
    const res = await this.send('Runtime.evaluate', {
      expression,
      awaitPromise,
      returnByValue: true,
    });
    if (res.exceptionDetails) {
      throw new Error(`Evaluation exception: ${res.exceptionDetails.text || JSON.stringify(res.exceptionDetails)}`);
    }
    return res.result ? res.result.value : undefined;
  }

  async waitForFunction(predicateFnStr, { timeout = 30000, pollInterval = 100 } = {}) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      try {
        const val = await this.evaluate(`(${predicateFnStr})()`);
        if (val) return val;
      } catch {
        // Retry
      }
      await new Promise((r) => setTimeout(r, pollInterval));
    }
    throw new Error(`Timeout after ${timeout}ms waiting for condition: ${predicateFnStr}`);
  }

  async waitForSelector(selector, { timeout = 30000, visible = true } = {}) {
    return this.waitForFunction(
      `function() {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) return false;
        ${visible ? 'return el.offsetParent !== null || el.getClientRects().length > 0;' : 'return true;'}
      }`,
      { timeout }
    );
  }

  async click(selector) {
    await this.waitForSelector(selector);
    const success = await this.evaluate(`
      (function() {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) return false;
        el.scrollIntoView({ block: 'center' });
        el.click();
        return true;
      })()
    `);
    if (!success) {
      throw new Error(`Failed to click selector: ${selector}`);
    }
  }

  async type(selector, text) {
    await this.waitForSelector(selector);
    await this.evaluate(`
      (function() {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) return false;
        el.focus();
        el.value = ${JSON.stringify(text)};
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        return true;
      })()
    `);
  }

  async captureScreenshot(outputPath) {
    const dir = path.dirname(outputPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    const result = await this.send('Page.captureScreenshot', {
      format: 'png',
      fromSurface: true,
      captureBeyondViewport: false,
    });
    const buffer = Buffer.from(result.data, 'base64');
    fs.writeFileSync(outputPath, buffer);
    return buffer.length;
  }

  async close() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.close();
    }
  }
}

export async function launchApp(options = {}) {
  const repoRoot = path.resolve('.');
  const exe = options.executable || path.resolve(repoRoot, 'target/installed/sayl-desktop.exe');
  const port = options.port || 9222;
  const dataDir = options.dataDir || path.resolve(repoRoot, 'target/sayl-data');
  const downloadDir = options.downloadDir || path.resolve(repoRoot, 'target/sayl-downloads');

  if (!fs.existsSync(dataDir)) {
    fs.mkdirSync(dataDir, { recursive: true });
  }
  if (!fs.existsSync(downloadDir)) {
    fs.mkdirSync(downloadDir, { recursive: true });
  }

  const env = {
    ...process.env,
    SAYL_DATA_DIR: dataDir,
    SAYL_DOWNLOAD_DIR: downloadDir,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  };

  // stdio ignore avoids named pipe EPERM in confined sandbox
  const proc = spawn(exe, [], { env, stdio: ['ignore', 'ignore', 'ignore'] });

  let cdpSession = null;
  const startTime = Date.now();
  const launchTimeout = options.launchTimeout || 30000;

  try {
    let target = null;
    while (Date.now() - startTime < launchTimeout) {
      try {
        const res = await fetch(`http://127.0.0.1:${port}/json`);
        if (res.ok) {
          const list = await res.json();
          target = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl) || list.find((t) => t.webSocketDebuggerUrl);
          if (target && target.webSocketDebuggerUrl) {
            break;
          }
        }
      } catch {
        // App still starting up
      }
      await new Promise((r) => setTimeout(r, 250));
    }

    if (!target || !target.webSocketDebuggerUrl) {
      throw new Error(`Failed to discover CDP target on port ${port} within ${launchTimeout}ms`);
    }

    cdpSession = new CdpSession(target.webSocketDebuggerUrl);
    await cdpSession.connect();

    await cdpSession.send('Page.enable');
    await cdpSession.send('Runtime.enable');
    await cdpSession.send('DOM.enable');

    // Wait until document body is ready
    await cdpSession.waitForFunction('() => document.readyState === "complete" && !!document.body');

    return {
      proc,
      cdp: cdpSession,
      target,
      port,
      dataDir,
      downloadDir,
      async close() {
        try {
          await cdpSession.close();
        } catch {
          // ignore
        }
        try {
          proc.kill();
        } catch {
          // ignore
        }
      },
    };
  } catch (err) {
    try {
      if (cdpSession) await cdpSession.close();
    } catch {}
    try {
      proc.kill();
    } catch {}
    throw err;
  }
}
