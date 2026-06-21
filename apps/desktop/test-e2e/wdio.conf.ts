import type { Options } from "@wdio/types";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const appBinary = path.resolve(
  __dirname,
  "../../../target/release/synapse" + (process.platform === "win32" ? ".exe" : ""),
);

export const config: Options.Testrunner = {
  specs: ["tests/**/*.test.ts"],
  maxInstances: 1,

  capabilities: [
    {
      // tauri-driver accepts the application path as a capability.
      // @ts-expect-error — non-standard capability key
      "tauri:options": {
        application: appBinary,
      },
    },
  ],

  // No service — CI starts `tauri-driver` as a background process before
  // running this suite. tauri-driver listens on port 4444 by default.
  services: [],

  framework: "mocha",
  reporters: ["spec"],

  mochaOpts: {
    timeout: 30_000,
  },

  hostname: "localhost",
  port: 4444,
  path: "/",
};
