const $ = id => document.getElementById(id);
function request(command, extra={}) {
  return new Promise(resolve => chrome.runtime.sendMessage({command,...extra}, response =>
    resolve(chrome.runtime.lastError ? {ok:false,error:chrome.runtime.lastError.message} : (response || {ok:false}))));
}
async function refresh() {
  const result = await request('status');
  $('connection').textContent = result.ok ? '● Connected to UVie' : '● Native Host unavailable';
  $('connection').className = result.ok ? 'ready' : 'error';
  $('domain').textContent = result.domain || 'No supported tab';
  $('mode').textContent = result.mode === null || result.mode === undefined ? 'Domain default: inherit' : `Domain default: ${result.mode ? 'VI' : 'EN'}`;
}
async function change(command, vi) {
  const r = await request(command, {vi});
  if (!r.ok) $('connection').textContent = r.error || 'Could not update preference';
  await refresh();
}
$('vi').addEventListener('click', () => change('set_domain', true));
$('en').addEventListener('click', () => change('set_domain', false));
$('reset').addEventListener('click', () => change('clear_domain'));
refresh();
