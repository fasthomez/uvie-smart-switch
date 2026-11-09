// No content scripts, no tracking, and no external network requests.
const HOST = 'com.fasthomez.uvie_smart_switch';
const BROWSER = navigator.userAgent.includes('Edg/') ? 'msedge.exe' : 'chrome.exe';
let port = null;
let nextRequest = 0;
const responses = new Map();
let connected = false;
async function session() {
  const stored = await chrome.storage.session.get('uvieSession');
  if (stored.uvieSession) return stored.uvieSession;
  const id = crypto.randomUUID();
  await chrome.storage.session.set({uvieSession:id});
  return id;
}
function nativePort() {
  if (!port) {
    const p = chrome.runtime.connectNative(HOST);
    port = p;
    p.onMessage.addListener(message => {
      connected = message.ok === true;
      const pending = responses.get(message.id);
      if (pending) { responses.delete(message.id); pending(message); }
    });
    p.onDisconnect.addListener(() => {
      if (port === p) port = null;
      connected = false;
      for (const resolve of responses.values()) resolve({ok:false, error:'Native host disconnected'});
      responses.clear();
    });
  }
  return port;
}
async function send(command, extra = {}) {
  const id = ++nextRequest;
  const p = nativePort();
  return new Promise(resolve => {
    responses.set(id, resolve);
    try { p.postMessage({id, command, ...extra}); }
    catch (e) { responses.delete(id); resolve({ok:false, error:String(e)}); }
    setTimeout(() => { if (responses.delete(id)) resolve({ok:false, error:'Timeout'}); }, 2500);
  });
}
async function activeContext() {
  const win = await chrome.windows.getLastFocused({populate:true});
  if (!win.focused || win.type !== 'normal') return null;
  const tab = win.tabs?.find(tab => tab.active);
  if (!tab?.url) return null;
  let url;
  try { url = new URL(tab.url); } catch { return null; }
  if (!['https:', 'http:'].includes(url.protocol)) return null;
  return {browser:BROWSER, session:await session(), window_id:win.id,
    tab_id:tab.id, domain:url.hostname.toLowerCase(), title:tab.title || ''};
}
async function reportActive() {
  try {
    const context = await activeContext();
    if (context) await send('active', context);
  } catch (e) { console.warn('UVie companion:', String(e)); }
}
chrome.tabs.onActivated.addListener(reportActive);
chrome.tabs.onUpdated.addListener((_id, info) => { if (info.url || info.title || info.status === 'complete') reportActive(); });
chrome.windows.onFocusChanged.addListener(reportActive);
chrome.runtime.onStartup.addListener(reportActive);
chrome.runtime.onInstalled.addListener(reportActive);
chrome.alarms.create('uvie-heartbeat', {periodInMinutes:0.5});
chrome.alarms.onAlarm.addListener(a => { if (a.name === 'uvie-heartbeat') reportActive(); });
chrome.runtime.onMessage.addListener((message, _sender, reply) => {
  if (!['status', 'set_domain', 'clear_domain'].includes(message.command)) return;
  (async () => {
    const context = await activeContext();
    if (!context) return {ok:false, error:'Switch to a regular HTTP(S) tab'};
    if (message.command === 'status') {
      const result = await send('active', context);
      return {...result, domain:context.domain, connected};
    }
    if (message.command === 'set_domain' && typeof message.vi !== 'boolean')
      return {ok:false,error:'Invalid mode'};
    return send(message.command, {...context, vi:message.vi});
  })().then(reply, e => reply({ok:false,error:String(e)}));
  return true;
});
reportActive();
