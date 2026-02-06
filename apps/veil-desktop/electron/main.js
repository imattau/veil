const { app, BrowserWindow } = require("electron");
const path = require("path");
const { spawn } = require("child_process");

let relayProcess = null;

const startRelay = () => {
  const relayPath = process.env.VEIL_DESKTOP_RELAY_PATH;
  if (!relayPath) {
    return;
  }
  relayProcess = spawn(relayPath, [], {
    stdio: "inherit",
    env: {
      ...process.env,
      VEIL_RELAY_BIND: process.env.VEIL_RELAY_BIND ?? "127.0.0.1:9001",
    },
  });
};

const stopRelay = () => {
  if (relayProcess) {
    relayProcess.kill();
    relayProcess = null;
  }
};

const createWindow = () => {
  const win = new BrowserWindow({
    width: 1200,
    height: 800,
    backgroundColor: "#0f1117",
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
    },
  });

  const devUrl = process.env.ELECTRON_START_URL;
  if (devUrl) {
    win.loadURL(devUrl);
  } else {
    win.loadFile(path.join(__dirname, "..", "dist", "index.html"));
  }
};

app.whenReady().then(() => {
  startRelay();
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  stopRelay();
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("quit", () => {
  stopRelay();
});
