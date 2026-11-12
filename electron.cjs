const { app, BrowserWindow, net, protocol } = require('electron');
const { resolve, sep } = require('node:path');
const { pathToFileURL } = require('node:url');

protocol.registerSchemesAsPrivileged([
  { scheme: 'robowarnet', privileges: { standard: true, secure: true, supportFetchAPI: true } },
]);

const root = resolve(__dirname, 'dist');
const entry = 'robowarnet://app/index.html';

function openWindow() {
  const window = new BrowserWindow({
    width: 1240,
    height: 850,
    backgroundColor: '#10171b',
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', event => event.preventDefault());
  window.loadURL(entry);
}

app.whenReady().then(() => {
  protocol.handle('robowarnet', request => {
    const url = new URL(request.url);
    let file;
    try { file = resolve(root, '.' + decodeURIComponent(url.pathname)); }
    catch { return new Response('Bad path', { status: 400 }); }
    if (url.host !== 'app' || !file.startsWith(root + sep)) {
      return new Response('Not found', { status: 404 });
    }
    return net.fetch(pathToFileURL(file).href);
  });
  openWindow();
});
app.on('activate', () => {
  if (!BrowserWindow.getAllWindows().length) openWindow();
});
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
