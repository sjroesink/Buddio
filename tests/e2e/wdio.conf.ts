import type { Options } from "@wdio/types";
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { platform } from "node:os";
import { resolve } from "node:path";

// Resolve the tauri-driver binary. On Windows we use msedgedriver as the
// WebDriver backend, on Linux WebKitWebDriver ships with webkit2gtk.
let tauriDriver: ChildProcess;
const extension = platform() === "win32" ? ".exe" : "";
const buddioBinary = resolve(
  __dirname,
  `../../src-tauri/target/debug/buddio${extension}`,
);
const legacyBinary = resolve(
  __dirname,
  `../../src-tauri/target/debug/golaunch${extension}`,
);
const applicationBinary = existsSync(buddioBinary) ? buddioBinary : legacyBinary;

export const config: Options.Testrunner = {
  runner: "local",
  autoCompileOpts: {
    tsNodeOpts: { project: "./tsconfig.node.json" },
  },

  specs: [resolve(__dirname, "specs/**/*.test.ts")],
  maxInstances: 1,

  capabilities: [
    {
      // tauri-driver exposes a WebDriver interface on port 4444 by default
      "browserName": "wry",
      "tauri:options": {
        application: applicationBinary,
      },
    } as WebdriverIO.Capabilities,
  ],

  logLevel: "warn",
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  // Start tauri-driver before tests
  onPrepare() {
    tauriDriver = spawn("tauri-driver", [], {
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env,
        BUDDIO_TEST: "1",
      },
    });

    // Wait for tauri-driver to start
    return new Promise<void>((resolve) => {
      // Give tauri-driver a moment to start listening
      setTimeout(resolve, 2000);
    });
  },

  // Stop tauri-driver after tests
  onComplete() {
    tauriDriver?.kill();
  },
};
