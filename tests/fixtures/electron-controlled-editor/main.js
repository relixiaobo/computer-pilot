const { app, BrowserWindow } = require("electron");

const title = process.env.FIXTURE_WINDOW_TITLE || "Computer Pilot Electron Fixture";
const userData = process.env.FIXTURE_USER_DATA;

if (userData) app.setPath("userData", userData);
app.setName("Electron");
app.commandLine.appendSwitch("force-renderer-accessibility");

app.whenReady().then(async () => {
  const window = new BrowserWindow({
    width: 720,
    height: 520,
    show: true,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  await window.loadFile("index.html", { query: { title } });
  window.setTitle(title);
  process.stdout.write(`READY ${process.pid} ${title}\n`);
});

app.on("window-all-closed", () => app.quit());
