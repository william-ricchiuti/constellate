// The Folder section, against the example's stand-in (VW_SETTINGS_KIND=folder, the
// default, with VW_SETTINGS_CAPPED=1). Prints what it sees and what reached the file.
(async () => {
  const L = m => window.ipc.postMessage(m);
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const disk = async () => (await fetch('vw://app/api/config')).json();
  const $ = id => document.getElementById(id);
  const set = async (id, v) => { const el = $(id); el.value = v; el.dispatchEvent(new Event('change', { bubbles: true })); await sleep(300); };
  L('path=' + $('folderPath').textContent);
  L('kind=' + $('folderKind').textContent);
  L('capped hidden=' + $('folderCapped').hidden + ' text=' + $('folderCapped').textContent);
  L('read as options=' + Array.from($('f-sourceKind').options).map(o => o.value + ':' + o.textContent).join(', '));
  L('read as value=' + $('f-sourceKind').value + ' node size value=' + $('f-nodeSize').value);
  L('colour labels=' + Array.from($('f-nodeColorMode').options).map(o => o.value + ':' + o.textContent).join(', '));
  L('node size row hidden=' + $('f-nodeSize-row').hidden + ' unresolved row hidden=' + $('f-showUnresolvedLinks-row').hidden);
  await set('f-sourceKind', 'logseq'); L('sourceKind -> ' + (await disk()).sourceKind);
  await set('f-nodeSize', 'fileSize'); L('nodeSize -> ' + (await disk()).nodeSize);
  await set('f-sourceKind', 'auto'); L('sourceKind back to auto -> ' + JSON.stringify((await disk()).sourceKind) + ' select=' + $('f-sourceKind').value);
  // An Obsidian vault hides the folder-only bits and keeps the plain colour labels.
  window.__vwFolder = { path: '~/Vault', kind: 'obsidian', kindLabel: 'Obsidian vault', reason: 'found .obsidian', capped: false, sample: false };
  folder = window.__vwFolder; renderFolder();
  L('obsidian: kind=' + $('folderKind').textContent + ' node size row hidden=' + $('f-nodeSize-row').hidden + ' colour labels=' + Array.from($('f-nodeColorMode').options).map(o => o.textContent).join('/'));
  folder = { sample: true }; renderFolder();
  L('sample: path=' + $('folderPath').textContent);
  L('DONE');
})().catch(e => window.ipc.postMessage('SCRIPT ERROR ' + e.message + ' ' + e.stack));
