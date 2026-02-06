import { spawn } from "node:child_process";
import path from "node:path";

export type RelayProcess = {
  stop: () => void;
};

export function startLocalRelay(): RelayProcess | null {
  const relayPath = process.env.VEIL_DESKTOP_RELAY_PATH;
  if (!relayPath) {
    return null;
  }

  const relay = spawn(relayPath, [], {
    stdio: "inherit",
    env: {
      ...process.env,
      VEIL_RELAY_BIND: process.env.VEIL_RELAY_BIND ?? "127.0.0.1:9001",
    },
  });

  return {
    stop: () => {
      relay.kill();
    },
  };
}
