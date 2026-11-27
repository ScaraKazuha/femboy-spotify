// ♡ femboy spotify — UI logic ♡
'use strict';

/* ───────────── backend bridge (Tauri, or demo mode in a plain browser) ───────────── */

const TAURI = window.__TAURI__;
const DEMO = !TAURI;
const invoke = DEMO ? demoInvoke : (cmd, args) => TAURI.core.invoke(cmd, args);
const listen = DEMO ? demoListen : (ev, cb) => TAURI.event.listen(ev, (e) => cb(e.payload));
const appWindow = DEMO ? null : TAURI.window.getCurrentWindow();

/* ───────────── state ───────────── */

const store = {
  get(k, d) { try { const v = localStorage.getItem(k); return v === null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); return true; } catch { return false; } },
};

const S = {
  me: null,
  playlists: [],
  liked: null,
  recent: null,
  view: { name: 'home' },
  back: [],
  forward: [],
  filter: 'all',
  queue: [],
  index: -1,
  context: '',
  playing: false,
  buffering: false,
  pos: 0,
  posAt: performance.now(),
  volume: store.get('volume', 70),
  muted: false,
  shuffle: store.get('shuffle', false),
  repeat: store.get('repeat', 'off'), // off | all | one
  lyrics: null,
  lyricsFor: null,
  saved: new Set(),
  // Spotify Connect
  now: null,          // track playing on the active device
  ctx: null,          // context string sent with play
  durationMs: 0,
  devices: [],
  activeDevice: null,
  activeDeviceName: null,
  cache: new Map(),   // uri -> track info
};

const $ = (id) => document.getElementById(id);
const main = $('main');

function esc(s) {
  return String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
function fmt(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
let toastTimer;
function toast(msg) {
  const t = $('toast');
  t.textContent = String(msg);
  t.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove('show'), 3200);
}
const PLACEHOLDER = 'data:image/svg+xml,' + encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffd0e4"/><stop offset="1" stop-color="#d9c5f7"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/><path d="M50 74S24 58 20 44c-3-10 3-20 13-20 7 0 12 4 17 10 5-6 10-10 17-10 10 0 16 10 13 20-4 14-30 30-30 30z" fill="#fff" opacity=".85"/></svg>');
const img = (url) => esc(url || PLACEHOLDER);
const current = () => S.now;

/* ───────────── navigation ───────────── */

function go(view, { push = true } = {}) {
  if (push) { S.back.push(S.view); S.forward = []; }
  S.view = view;
  render();
  main.scrollTop = 0;
}
function goBack() { if (S.back.length) { S.forward.push(S.view); S.view = S.back.pop(); render(); } }
function goForward() { if (S.forward.length) { S.back.push(S.view); S.view = S.forward.pop(); render(); } }

async function openLiked() {
  go({ name: 'list', kind: 'liked', title: 'Liked Songs', sub: 'your favorite songs ♡', tracks: S.liked, loading: !S.liked });
  if (!S.liked) {
    try { S.liked = await invoke('liked'); } catch (e) { return fail(e); }
    if (S.view.kind === 'liked') { S.view.tracks = S.liked; S.view.loading = false; render(); }
  }
  S.liked.forEach((t) => S.saved.add(t.id));
}

// history of what you played in femboy spotify (kept on this computer)
function remember(t) {
  const h = store.get('history', []).filter((x) => x.uri !== t.uri);
  h.unshift(t);
  store.set('history', h.slice(0, 100));
}

function openRecent() {
  go({ name: 'list', kind: 'recent', title: 'Recently played', sub: 'what you listened to lately', tracks: store.get('history', []) });
}

async function openPlaylist(p) {
  const view = { name: 'list', kind: 'playlist', id: p.id, title: p.name, sub: p.owner ? `by ${p.owner}` : 'playlist', image: p.image, tracks: null, loading: true };
  go(view);
  try { view.tracks = await invoke('playlist_tracks', { id: p.id }); cacheTracks(view.tracks); } catch (e) { return fail(e); }
  view.loading = false;
  if (S.view === view) render();
}

let searchSeq = 0;
async function doSearch(q) {
  q = q.trim();
  if (q.length < 2) return;
  const seq = ++searchSeq;
  const view = { name: 'list', kind: 'search', q, title: `"${q}"`, sub: 'search results', tracks: null, loading: true };
  go(view, { push: S.view.kind !== 'search' });
  try {
    const tracks = await invoke('search', { q });
    cacheTracks(tracks);
    if (seq !== searchSeq) return;
    view.tracks = tracks;
    view.loading = false;
    if (S.view === view) render();
  } catch (e) { fail(e); }
}

function fail(e) {
  console.error(e);
  toast(`oopsie: ${e}`);
  if (S.view.loading) { S.view.loading = false; S.view.tracks = S.view.tracks || []; render(); }
}

/* ───────────── rendering ───────────── */

function render() {
  const v = S.view;
  if (v.name === 'home') main.innerHTML = homeHtml();
  else if (v.name === 'library') main.innerHTML = libraryHtml();
  else if (v.name === 'list') main.innerHTML = listHtml(v);
  else if (v.name === 'queue') main.innerHTML = listHtml({ kind: 'queue', title: 'Up next', sub: S.context ? `playing from ${S.context}` : 'your queue', tracks: S.queue });
  else if (v.name === 'lyrics') main.innerHTML = lyricsViewHtml();
  hydrateIcons(main);
  fitHeroTitle();
  updateSidebar();
  $('btn-back').style.opacity = S.back.length ? 1 : .4;
  $('btn-forward').style.opacity = S.forward.length ? 1 : .4;
}

function featured() {
  const custom = store.get('banner', null);
  const title = store.get('bannerTitle', '') || (S.playlists[0]?.name) || 'Liked Songs';
  const pl = S.playlists[0] || null;
  return { custom, title, pl };
}

function heroHtml() {
  const { custom, title, pl } = featured();
  // default look: the Boykisser Mix artwork (title & play button are painted into the picture)
  if (!custom && !store.get('bannerTitle', '')) {
    return `
    <section class="hero art-hero" style="background-image:url('art/boykisser-banner.jpg')">
      <button class="art-play" id="hero-play" title="play ${esc(featuredPlaylist()?.name || 'Liked Songs')}"></button>
      <button class="icon-btn hero-edit" id="hero-edit" title="customize banner">${icon('pen')}</button>
    </section>`;
  }
  const covers = S.playlists.filter((p) => p.image).slice(0, 2);
  const art = custom
    ? `<div class="hero-art" style="background-image:url('${esc(custom)}')"></div>`
    : `<div class="hero-stickers">
         ${covers[1] ? `<div class="sticker small" style="background-image:url('${img(covers[1].image)}')"></div>` : ''}
         <div class="sticker big" style="background-image:url('${img(covers[0]?.image)}')"></div>
       </div>`;
  const deco = [
    ['s', 3, 10, 22], ['s', 30, 6, 16], ['s', 58, 70, 28], ['s', 88, 12, 20], ['s', 46, 22, 14], ['s', 93, 70, 26],
    ['h', 2, 52, 30], ['h', 52, 14, 40], ['h', 40, 78, 26], ['h', 85, 36, 30],
  ].map(([k, x, y, s], i) => `<span class="${k}" style="left:${x}%;top:${y}%;width:${s}px;height:${s}px;animation-delay:${i * .37}s">${icon(k === 's' ? 'sparkle' : 'heart')}</span>`).join('');
  return `
  <section class="hero">
    ${art}
    <div class="hero-deco">${deco}</div>
    <div class="hero-content">
      <h1 class="hero-title">${esc(title)}</h1>
      <div class="hero-row">
        <button class="hero-play" id="hero-play" title="play">${icon('play')}</button>
        <span class="hero-sub">${pl && !store.get('bannerTitle', '') ? `${pl.total} songs` : ''}</span>
      </div>
    </div>
    <button class="icon-btn hero-edit" id="hero-edit" title="customize banner">${icon('pen')}</button>
  </section>`;
}

// Biggest font size at which the hero title fits without breaking words.
function fitHeroTitle() {
  const el = main.querySelector('.hero-title');
  if (!el) return;
  const box = el.parentElement;
  const maxH = box.clientHeight - 110;
  let size = 150;
  el.style.fontSize = size + 'px';
  while (size > 40 && (el.scrollWidth > box.clientWidth || el.offsetHeight > maxH)) {
    size -= 4;
    el.style.fontSize = size + 'px';
  }
}
window.addEventListener('resize', fitHeroTitle);

function homeHtml() {
  const chips = [['all', 'All'], ['playlists', 'Playlists'], ['liked', 'Liked'], ['recent', 'Recent']]
    .map(([k, l]) => `<button class="chip ${S.filter === k ? 'active' : ''}" data-filter="${k}">${l}</button>`).join('');
  const tiles = [
    `<button class="tile" data-open="liked"><div class="tile-art liked-art">${icon('heart')}</div><span>Liked Songs</span></button>`,
    `<button class="tile" data-open="dj" title="Spotify DJ"><img class="dj-art" src="art/headphones.png" alt=""><span>DJ X</span></button>`,
  ]
    .concat(S.playlists.slice(0, 6).map((p) => `<button class="tile" data-playlist="${esc(p.id)}"><img src="${img(p.image)}" alt=""><span>${esc(p.name)}</span></button>`))
    .join('');
  const cards = (S.filter === 'playlists' ? S.playlists : S.playlists.slice(7, 19).concat(S.playlists.length <= 7 ? S.playlists : []))
    .map(cardHtml).join('');
  return `
    ${heroHtml()}
    <div class="chips">${chips}</div>
    ${S.filter === 'all' ? `<div class="tiles">${tiles}</div>` : ''}
    ${cards ? `<div class="section-head"><h2>${S.filter === 'playlists' ? 'Your playlists' : 'Made for you'}</h2><div class="line">${icon('heart')}</div></div>
    <div class="cards">${cards}</div>` : ''}
    ${!S.playlists.length ? `<div class="empty"><div class="big">♡</div>loading your library...</div>` : ''}`;
}

function cardHtml(p) {
  return `<button class="card-item" data-playlist="${esc(p.id)}">
      <img class="cov" src="${img(p.image)}" alt="" loading="lazy">
      <div class="t">${esc(p.name)}</div>
      <div class="st">${esc(p.owner || 'playlist')} · ${p.total} songs</div>
    </button>`;
}

function libraryHtml() {
  return `
    <div class="list-head"><div>
      <div class="list-kind">library</div>
      <h1 class="list-title">Your playlists</h1>
      <div class="list-meta">${S.playlists.length} playlists</div>
    </div></div>
    <div class="cards">
      <button class="card-item" data-open="liked"><div class="cov liked-art">${icon('heart')}</div><div class="t">Liked Songs</div><div class="st">♡</div></button>
      <button class="card-item" data-open="dj"><img class="cov dj-art" src="art/headphones.png" alt=""><div class="t">DJ X</div><div class="st">Spotify DJ ♪</div></button>
      ${S.playlists.map(cardHtml).join('')}
    </div>`;
}

function listHtml(v) {
  const cover = v.kind === 'liked'
    ? `<div class="list-cover liked-art">${icon('heart')}</div>`
    : `<img class="list-cover" src="${img(v.image || v.tracks?.[0]?.image)}" alt="">`;
  const kinds = { liked: 'playlist', playlist: 'playlist', search: 'search', recent: 'history', queue: 'queue' };
  const cur = current();
  let body;
  if (v.loading) body = `<div class="empty"><div class="spinner">${icon('sparkle')}</div>loading... ₍ᐢ. .ᐢ₎</div>`;
  else if (!v.tracks?.length) body = `<div class="empty"><div class="big">(｡•́︿•̀｡)</div>nothing here yet~</div>`;
  else body = `<div class="tracks">
      <div class="trow head"><div class="tnum">#</div><div>title</div><div>album</div><div class="tdur">time</div></div>
      ${v.tracks.map((t, i) => {
        const playing = v.kind === 'queue' ? i === S.index : cur && cur.uri === t.uri;
        return `<div class="trow ${playing ? 'playing' : ''}" data-index="${i}">
          <div class="tnum">${playing ? '♪' : i + 1}</div>
          <div class="tmain"><img src="${img(t.image)}" alt="" loading="lazy"><div class="ttext"><div class="tname">${esc(t.name)}</div><div class="tartist">${esc(t.artists)}</div></div></div>
          <div class="talbum">${esc(t.album)}</div>
          <div class="tdur">${fmt(t.duration_ms)}</div>
        </div>`;
      }).join('')}
    </div>`;
  return `
    <div class="list-head">${cover}<div>
      <div class="list-kind">${kinds[v.kind] || ''}</div>
      <h1 class="list-title" style="font-size:${v.title.length > 24 ? 40 : 60}px">${esc(v.title)}</h1>
      <div class="list-meta">${esc(v.sub || '')}${v.tracks?.length ? ` · ${v.tracks.length} songs` : ''}</div>
    </div></div>
    ${v.tracks?.length ? `<div class="list-actions"><button class="round-play" id="list-play" title="play">${icon('play')}</button>
      <button class="icon-btn" id="list-shuffle" title="shuffle play">${icon('shuffle')}</button></div>` : ''}
    ${body}`;
}

function lyricsViewHtml() {
  const t = current();
  if (!t) return `<div class="empty"><div class="big">♪</div>play something first~</div>`;
  if (!S.lyrics) return `<div class="empty"><div class="big">♪</div>${S.lyricsFor === t.id ? 'no lyrics for this song (｡•́︿•̀｡)' : 'loading lyrics...'}</div>`;
  return `<div class="lyrics-view" id="lyrics-view">${S.lyrics.map((l, i) => `<p data-line="${i}" data-ms="${l.start_ms}">${esc(l.words)}</p>`).join('')}</div>`;
}

function updateSidebar() {
  const v = S.view;
  const active = v.name === 'library' ? 'library' : v.name === 'queue' ? 'queue' : v.kind === 'liked' ? 'liked' : v.kind === 'recent' ? 'recent' : v.kind === 'search' ? 'search' : '';
  document.querySelectorAll('.side-btn[data-nav]').forEach((b) => b.classList.toggle('active', b.dataset.nav === active));
  $('btn-home').style.background = v.name === 'home' ? '#fff' : '';
  $('btn-queue').classList.toggle('active', v.name === 'queue');
  $('btn-lyrics').classList.toggle('active', v.name === 'lyrics');
}

function renderSideThumbs() {
  $('side-thumbs').innerHTML = S.playlists.slice(0, 12)
    .map((p) => `<button data-playlist="${esc(p.id)}" title="${esc(p.name)}"><img src="${img(p.image)}" alt=""></button>`).join('');
}

/* ───────────── now playing ───────────── */

function renderNow() {
  const t = current();
  const cover = t?.image_large || t?.image;
  const coverHtml = cover ? `<img src="${esc(cover)}" alt="">` : `<div class="cover-placeholder">${icon('heart')}</div>`;
  $('right-cover').innerHTML = coverHtml;
  $('np-cover').innerHTML = t?.image ? `<img src="${esc(t.image)}" alt="">` : `<div class="cover-placeholder">${icon('heart')}</div>`;
  $('right-name').textContent = t ? t.name : 'nothing yet~';
  $('right-artist').textContent = t ? t.artists : 'pick a song ♡';
  $('np-name').textContent = t ? t.name : '—';
  $('np-artist').textContent = t ? t.artists : '';
  renderDeviceInfo();
  document.title = t ? `${t.name} · ${t.artists} ♡` : 'Femboy Spotify';
  const saved = t && S.saved.has(t.id);
  for (const id of ['right-add', 'np-add']) {
    setIcon($(id), saved ? 'check-circle' : 'plus-circle');
    $(id).classList.toggle('saved', !!saved);
  }
  renderPlayState();
  renderLyricsCard();
  // highlight the playing row
  document.querySelectorAll('.trow[data-index]').forEach((row) => {
    const i = +row.dataset.index;
    const tr = S.view.name === 'queue' ? (i === S.index) : (S.view.tracks?.[i]?.uri === t?.uri);
    row.classList.toggle('playing', !!tr);
    row.querySelector('.tnum').textContent = tr ? '♪' : i + 1;
  });
}

function renderPlayState() {
  setIcon($('btn-play'), S.playing ? 'pause' : 'play');
  $('btn-panel').classList.toggle('vibing', S.playing && !S.buffering);
  $('btn-panel').classList.toggle('dj', !!S.djOn);
  $('btn-shuffle').classList.toggle('active', S.shuffle);
  setIcon($('btn-repeat'), S.repeat === 'one' ? 'repeat-one' : 'repeat');
  $('btn-repeat').classList.toggle('active', S.repeat !== 'off');
  setIcon($('btn-mute'), S.muted || S.volume === 0 ? 'volume-off' : 'volume');
}

const duration = () => S.durationMs || current()?.duration_ms || 0;

function position() {
  let p = S.pos;
  if (S.playing && !S.buffering) p += performance.now() - S.posAt;
  return current() ? Math.min(p, duration()) : 0;
}
function setPos(ms) { S.pos = ms; S.posAt = performance.now(); }

function tick() {
  const pos = position();
  const dur = duration();
  const pct = dur ? (pos / dur) * 100 : 0;
  if (!seekDrag) {
    $('seek-fill').style.width = pct + '%';
    $('seek-knob').style.left = pct + '%';
  }
  $('time-pos').textContent = fmt(pos);
  $('time-left').textContent = '-' + fmt(dur - pos);
  syncLyrics(pos);
}

/* ───────────── playback (Spotify Connect remote) ─────────────
   The music plays in the official Spotify app on one of the user's devices.
   We send commands and mirror the state Spotify pushes back ("remote" events). */

const NO_DEVICE = 'no-spotify-device';

// which Spotify context a list view corresponds to
function contextOf(v) {
  if (v.kind === 'liked') return 'liked';
  if (v.kind === 'playlist') return `spotify:playlist:${v.id}`;
  // search results, history, queue: send the list itself
  // (Spotify answers 403 when a remote starts a "spotify:search:" context)
  return null;
}

function cacheTracks(list) { (list || []).forEach((t) => S.cache.set(t.uri, t)); }

// the app's own built-in player (Spotify's web player in a hidden window)
const isBuiltin = (d) => /^web player/i.test(d.name || '');
const deviceName = (d) => (isBuiltin(d) ? 'femboy spotify (this app) ♡' : d.name);

// where to start music. Keep going on a device that is playing right now; otherwise
// use our own player. Every app start makes a new web-player device and Spotify can
// keep the previous (dead) one marked active for a while, so an idle "active"
// device is not trusted and a live built-in one wins over it.
function pickDevice() {
  const active = S.devices.find((d) => d.id === S.activeDevice);
  if (active && S.playing) return null;
  const builtins = S.devices.filter(isBuiltin);
  const builtin = builtins.find((d) => d.id !== S.activeDevice) || builtins[0];
  if (builtin) return builtin.id;
  if (active) return null;
  const d = S.devices.find((x) => x.kind === 'computer') || S.devices[0];
  return d ? d.id : null;
}

function handleRemoteError(e) {
  if (String(e).includes(NO_DEVICE)) { showNoDevice(); return; }
  toast(`oopsie: ${e}`);
}

function playFrom(tracks, i, label, ctx = null) {
  S.queue = tracks.slice();
  S.context = label || '';
  S.ctx = ctx;
  cacheTracks(tracks);
  playIndex(i);
}

async function playIndex(i) {
  const t = S.queue[i];
  if (!t) return;
  // optimistic: show it right away, Spotify confirms a moment later
  S.index = i;
  S.playing = true;
  S.buffering = true;
  S.durationMs = t.duration_ms;
  setPos(0);
  setNow(t);
  try {
    await invoke('play', { context: S.ctx, uris: S.queue.map((x) => x.uri), index: i, device: pickDevice() });
  } catch (e) {
    S.playing = false;
    S.buffering = false;
    renderPlayState();
    handleRemoteError(e);
  }
}

const control = (action) => invoke('control', { action }).catch(handleRemoteError);

function next() { if (current()) control('skip_next'); }

function prev() {
  if (!current()) return;
  if (position() > 3000) { seekTo(0); return; }
  control('skip_prev');
}

async function togglePlay() {
  if (!current()) {
    const v = S.view;
    if (v.tracks?.length) playFrom(v.tracks, 0, v.title, contextOf(v));
    else if (S.liked?.length) playFrom(S.liked, 0, 'Liked Songs', 'liked');
    return;
  }
  // the active device is a web player from an earlier app start (ours is the other one)
  const staleActive = S.devices.some((d) => d.id === S.activeDevice && isBuiltin(d))
    && S.devices.filter(isBuiltin).length > 1;
  if (!S.activeDevice || (staleActive && !S.playing)) {
    // nothing (alive) active any more: start the song again on a device
    if (S.index >= 0) return playIndex(S.index);
    if (S.djOn) return playDJ();
    return showNoDevice();
  }
  if (S.playing) {
    setPos(position());
    S.playing = false;
    renderPlayState();
    control('pause');
  } else {
    S.playing = true;
    setPos(S.pos);
    renderPlayState();
    control('resume');
  }
}

function seekTo(ms) {
  setPos(ms);
  invoke('seek', { ms: Math.round(ms) }).catch(handleRemoteError);
  tick();
}

let volTimer;
let volTouched = 0;
function drawVolume(v) {
  $('vol-fill').style.width = v + '%';
  $('vol-knob').style.left = v + '%';
}
function setVolume(v, { send = true } = {}) {
  S.volume = Math.max(0, Math.min(100, Math.round(v)));
  S.muted = false;
  volTouched = performance.now();
  drawVolume(S.volume);
  renderPlayState();
  if (!send) return;
  clearTimeout(volTimer);
  volTimer = setTimeout(() => invoke('set_volume', { volume: S.volume }).catch(() => {}), 120);
}

function setOption(name, value) {
  invoke('set_option', { name, value }).catch(handleRemoteError);
}

/* the currently playing track changed (by us or on another device) */
function setNow(t) {
  const changed = S.now?.uri !== t.uri;
  S.now = t;
  S.index = S.queue.findIndex((x) => x.uri === t.uri);
  if (changed) {
    S.lyrics = null;
    S.lyricsFor = null;
    if (t.uri.startsWith('spotify:track:')) remember(t);
    if (t.id) loadLyrics(t);
    else { S.lyricsFor = t.id; renderLyricsCard(); }
  }
  renderNow();
  if (changed && (S.view.name === 'lyrics' || S.view.name === 'queue')) render();
}

/* ───────────── DJ X (Spotify DJ) ───────────── */

const DJ_URI = 'spotify:playlist:37i9dQZF1EYkqdzj48dyYq';
const DJ_ART = 'art/headphones.png';

async function playDJ() {
  S.queue = [];
  S.index = -1;
  S.ctx = null;
  S.context = 'DJ X';
  S.djOn = true;
  S.playing = true;
  S.buffering = true;
  setPos(0);
  setNow({ uri: 'dj:start', id: '', name: 'DJ X is warming up ♪', artists: 'Spotify DJ', album: '', duration_ms: 0, image: DJ_ART, image_large: DJ_ART });
  toast('DJ X is on the way ♪');
  try {
    await invoke('play_dj', { device: pickDevice() });
  } catch (e) {
    S.playing = false;
    S.djOn = false;
    renderPlayState();
    handleRemoteError(e);
  }
}

// DJ X talking: "spotify:media:000…" = DJ speaking, "spotify:media:<track id>" = DJ introducing that track
async function onDjMedia(uri) {
  const id = uri.split(':').pop();
  const talking = { uri, id: '', name: 'DJ X is talking ♪', artists: 'Spotify DJ', album: '', duration_ms: S.durationMs, image: DJ_ART, image_large: DJ_ART };
  if (/^0+$/.test(id)) return setNow(talking);
  setNow({ ...talking, name: 'DJ X ♪ introducing...' });
  try {
    const info = S.cache.get(`spotify:track:${id}`) || await invoke('track_info', { uri: `spotify:track:${id}` });
    if (info && S.now?.uri === uri) {
      S.now = { ...info, id: '', uri, name: `DJ X ♪ ${info.name}`, artists: `introducing · ${info.artists}` };
      renderNow();
    }
  } catch { /* keep the talking card */ }
}

async function onRemoteTrack(uri) {
  if (uri.startsWith('spotify:media:')) return onDjMedia(uri);
  const known = S.cache.get(uri) || S.queue.find((x) => x.uri === uri);
  if (known) return setNow(known);
  setNow({ uri, id: uri.split(':').pop(), name: '...', artists: '', album: '', duration_ms: S.durationMs });
  try {
    const info = await invoke('track_info', { uri });
    if (info) {
      S.cache.set(uri, info);
      if (S.now?.uri === uri) { S.now = info; renderNow(); }
    }
  } catch { /* keep the placeholder */ }
}

function applyRemote(st) {
  S.devices = st.devices || [];
  S.djOn = !!st.active_device && st.context_uri === DJ_URI;
  S.activeDevice = st.active_device;
  const active = S.devices.find((d) => d.id === st.active_device);
  S.activeDeviceName = active ? deviceName(active) : st.active_device_name;
  S.playing = st.playing;
  S.buffering = st.playing && st.buffering;
  S.durationMs = st.duration_ms || S.durationMs;
  setPos(st.position_ms);
  S.shuffle = st.shuffle;
  S.repeat = st.repeat_track ? 'one' : st.repeat_context ? 'all' : 'off';
  if (st.volume != null && !S.muted && performance.now() - volTouched > 1500 && !$('vol-bar').classList.contains('drag')) {
    S.volume = st.volume;
    drawVolume(S.volume);
  }
  if (st.track_uri && st.track_uri !== S.now?.uri) onRemoteTrack(st.track_uri);
  renderDeviceInfo();
  renderPlayState();
  if (!$('devices-pop').classList.contains('hidden')) renderDevices();
}

/* ───────────── devices ───────────── */

const KIND_ICON = { computer: 'devices', phone: 'devices', tablet: 'devices', speaker: 'volume', tv: 'mini', cast: 'mini', console: 'mini', car: 'volume', watch: 'disc', device: 'devices' };

function renderDeviceInfo() {
  const name = S.activeDeviceName;
  const what = S.djOn ? 'DJ X' : 'Now playing';
  $('right-title').textContent = name ? `${what} • on ${name}` : (S.context ? `${what} • ${S.context}` : what);
  $('btn-devices').classList.toggle('active', !!name);
  $('btn-devices').title = name ? `playing on ${name}` : 'devices';
}

function renderDevices() {
  const list = $('devices-list');
  if (!S.devices.length) {
    list.innerHTML = `<p class="dev-empty">the built-in player is waking up (｡•́︿•̀｡)<br>${S.playerReady ? 'give it a few seconds' : '<button class="chip" data-player-login>sign in to it</button>'}</p>`;
    return;
  }
  // our own player on top
  const devices = [...S.devices.filter(isBuiltin), ...S.devices.filter((d) => !isBuiltin(d))];
  list.innerHTML = devices.map((d) => `
    <button class="dev ${d.active ? 'active' : ''}" data-device="${esc(d.id)}">
      ${icon(isBuiltin(d) ? 'heart' : KIND_ICON[d.kind] || 'devices')}
      <span class="dev-name">${esc(deviceName(d))}</span>
      <span class="dev-meta">${d.active ? (S.playing ? 'playing ♪' : 'connected') : esc(d.kind)}</span>
    </button>`).join('');
}

function toggleDevices(force) {
  const pop = $('devices-pop');
  const show = force ?? pop.classList.contains('hidden');
  pop.classList.toggle('hidden', !show);
  if (show) renderDevices();
}

$('devices-list').addEventListener('click', async (e) => {
  const b = e.target.closest('[data-device]');
  if (e.target.closest('[data-player-login]')) { toggleDevices(false); return playerLogin(); }
  if (!b) return;
  const id = b.dataset.device;
  if (id === S.activeDevice) return toggleDevices(false);
  toggleDevices(false);
  if (S.activeDevice) {
    toast('moving the music ♡');
    invoke('transfer', { device: id }).catch(handleRemoteError);
  } else if (S.index >= 0) {
    try { await invoke('play', { context: S.ctx, uris: S.queue.map((x) => x.uri), index: S.index, device: id }); } catch (err) { handleRemoteError(err); }
  } else {
    invoke('transfer', { device: id }).catch(handleRemoteError);
  }
});
document.addEventListener('click', (e) => {
  if (!e.target.closest('#devices-pop') && !e.target.closest('#btn-devices')) toggleDevices(false);
});

/* ───────────── built-in player ─────────────
   Spotify's web player runs hidden inside the app and plays the music itself.
   It needs its own Spotify sign-in once (a small window opens for it). */

function showNoDevice() {
  if (!S.playerReady) return showPlayerSetup();
  // signed in, but the player hasn't shown up in Spotify Connect yet
  toast('the built-in player is waking up, try again in a few seconds ♡');
}

function showPlayerSetup() { $('nodevice').classList.remove('hidden'); }
const hidePlayerSetup = () => $('nodevice').classList.add('hidden');

async function playerLogin() {
  hidePlayerSetup();
  try {
    await invoke('player_login');
    toast('sign in to Spotify in the little window ♡');
  } catch (e) { toast(`oopsie: ${e}`); }
}

$('nodevice-ok').onclick = playerLogin;
$('nodevice-later').onclick = hidePlayerSetup;

listen('player', () => {
  S.playerReady = true;
  hidePlayerSetup();
  toast('the built-in player is ready, music plays right here now ♡');
});

async function checkPlayer() {
  try {
    S.playerReady = await invoke('player_status');
    if (!S.playerReady) showPlayerSetup();
  } catch (e) { toast(`built-in player: ${e}`); }
}

async function saveCurrent() {
  const t = current();
  if (!t || !t.uri.startsWith('spotify:track:')) return;
  if (S.saved.has(t.id)) return toast('already in your liked songs ♡');
  try {
    await invoke('save_track', { id: t.id });
    S.saved.add(t.id);
    if (S.liked) S.liked.unshift(t);
    renderNow();
    toast('added to liked songs ♡');
  } catch (e) { toast(`couldn't save: ${e}`); }
}

/* ───────────── lyrics ───────────── */

async function loadLyrics(t) {
  try {
    const lines = await invoke('lyrics', { id: t.id });
    if (current()?.id !== t.id) return;
    S.lyrics = lines && lines.length ? lines : null;
  } catch { S.lyrics = null; }
  S.lyricsFor = t.id;
  renderLyricsCard();
  if (S.view.name === 'lyrics') render();
}

function renderLyricsCard() {
  const box = $('lyrics-lines');
  const t = current();
  if (!t) { box.innerHTML = ''; return; }
  if (!S.lyrics) {
    box.innerHTML = `<p class="none">${S.lyricsFor === t.id ? 'no lyrics for this one~' : 'loading lyrics...'}</p>`;
    return;
  }
  box.innerHTML = S.lyrics.map((l, i) => `<p data-line="${i}">${esc(l.words)}</p>`).join('');
  lastLine = -1;
}

let lastLine = -1;
function syncLyrics(pos) {
  if (!S.lyrics) return;
  const synced = S.lyrics.some((l) => l.start_ms > 0);
  if (!synced) return;
  let idx = -1;
  for (let i = 0; i < S.lyrics.length; i++) { if (S.lyrics[i].start_ms <= pos + 150) idx = i; else break; }
  if (idx === lastLine) return;
  lastLine = idx;
  for (const root of [$('lyrics-lines'), $('lyrics-view')]) {
    if (!root) continue;
    root.querySelectorAll('p[data-line]').forEach((p) => {
      const i = +p.dataset.line;
      p.classList.toggle('now', i === idx);
      p.classList.toggle('past', i < idx);
    });
    const el = root.querySelector(`p[data-line="${idx}"]`);
    if (el) {
      const scroller = root.id === 'lyrics-view' ? main : root;
      const top = el.offsetTop - (root.id === 'lyrics-view' ? main.clientHeight * 0.35 : root.clientHeight * 0.3);
      scroller.scrollTo({ top, behavior: 'smooth' });
    }
  }
}

/* ───────────── drag bars (seek / volume) ───────────── */

let seekDrag = false;
function bindBar(bar, onMove, onDone) {
  const ratio = (e) => {
    const r = bar.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
  };
  bar.addEventListener('pointerdown', (e) => {
    bar.setPointerCapture(e.pointerId);
    bar.classList.add('drag');
    onMove(ratio(e), true);
    const move = (ev) => onMove(ratio(ev), true);
    const up = (ev) => {
      bar.classList.remove('drag');
      bar.removeEventListener('pointermove', move);
      bar.removeEventListener('pointerup', up);
      onDone(ratio(ev));
    };
    bar.addEventListener('pointermove', move);
    bar.addEventListener('pointerup', up);
  });
}

bindBar($('seek-bar'), (r) => {
  if (!current()) return;
  seekDrag = true;
  $('seek-fill').style.width = r * 100 + '%';
  $('seek-knob').style.left = r * 100 + '%';
  $('time-pos').textContent = fmt(r * current().duration_ms);
}, (r) => {
  seekDrag = false;
  if (current()) seekTo(r * current().duration_ms);
});

bindBar($('vol-bar'), (r) => setVolume(r * 100, { send: false }), (r) => setVolume(r * 100));

/* ───────────── events ───────────── */

// clicks inside the main panel (event delegation)
main.addEventListener('click', (e) => {
  const el = e.target.closest('[data-filter],[data-open],[data-playlist],[data-index],[data-line],#hero-play,#hero-edit,#list-play,#list-shuffle');
  if (!el) return;
  if (el.dataset.filter) {
    const f = el.dataset.filter;
    if (f === 'liked') return openLiked();
    if (f === 'recent') return openRecent();
    S.filter = f;
    return render();
  }
  if (el.dataset.open === 'liked') return openLiked();
  if (el.dataset.open === 'dj') return playDJ();
  if (el.dataset.playlist) return openPlaylistById(el.dataset.playlist);
  if (el.dataset.line !== undefined && S.lyrics) {
    const ms = S.lyrics[+el.dataset.line]?.start_ms;
    if (ms) seekTo(ms);
    return;
  }
  if (el.dataset.index !== undefined) {
    const i = +el.dataset.index;
    if (S.view.name === 'queue') return playIndex(i);
    return playFrom(S.view.tracks, i, S.view.title, contextOf(S.view));
  }
  if (el.id === 'hero-edit') return openCustomize();
  if (el.id === 'hero-play') return playFeatured();
  if (el.id === 'list-play' || el.id === 'list-shuffle') {
    const isQueue = S.view.name === 'queue';
    const tracks = isQueue ? S.queue : S.view.tracks;
    if (!tracks?.length) return;
    const shuffle = el.id === 'list-shuffle';
    if (shuffle !== S.shuffle) setOption('shuffle', shuffle);
    const start = shuffle ? Math.floor(Math.random() * tracks.length) : 0;
    if (isQueue) return playIndex(start);
    return playFrom(tracks, start, S.view.title, contextOf(S.view));
  }
});

function openPlaylistById(id) {
  const p = S.playlists.find((x) => x.id === id);
  if (p) openPlaylist(p);
}

// a playlist called "...boykisser..." if you have one, otherwise your first playlist
function featuredPlaylist() {
  return S.playlists.find((p) => /boykisser/i.test(p.name)) || S.playlists[0] || null;
}

async function playFeatured() {
  const pl = featuredPlaylist();
  try {
    if (!pl) {
      if (!S.liked) S.liked = await invoke('liked');
      return playFrom(S.liked, 0, 'Liked Songs', 'liked');
    }
    toast(`playing ${pl.name} ♡`);
    const tracks = await invoke('playlist_tracks', { id: pl.id });
    if (tracks.length) playFrom(tracks, S.shuffle ? Math.floor(Math.random() * tracks.length) : 0, pl.name, `spotify:playlist:${pl.id}`);
  } catch (e) { toast(e); }
}

$('side-thumbs').addEventListener('click', (e) => {
  const b = e.target.closest('[data-playlist]');
  if (b) openPlaylistById(b.dataset.playlist);
});

document.querySelectorAll('.side-btn[data-nav]').forEach((b) => b.addEventListener('click', () => {
  const n = b.dataset.nav;
  if (n === 'library') go({ name: 'library' });
  else if (n === 'search') $('search').focus();
  else if (n === 'liked') openLiked();
  else if (n === 'queue') go({ name: 'queue' });
  else if (n === 'recent') openRecent();
  else if (n === 'customize') openCustomize();
  else if (n === 'logout') doLogout();
}));

$('btn-home').onclick = () => go({ name: 'home' });
$('btn-browse').onclick = () => go({ name: 'library' });
$('btn-back').onclick = goBack;
$('btn-forward').onclick = goForward;
$('btn-menu').onclick = async () => {
  const v = DEMO ? 'demo' : await TAURI.app.getVersion().catch(() => '?');
  toast(`femboy spotify v${v} ♡ made with Rust & love`);
};
$('btn-bell').onclick = () => (S.update ? showUpdate() : toast('no new notifications ♡'));

/* ───────────── auto-update (GitHub Releases) ───────────── */

async function checkForUpdate() {
  try {
    const info = await invoke('check_update');
    if (!info) return;
    const isNew = S.update?.version !== info.version;
    S.update = info;
    $('bell-dot').classList.remove('hidden');
    if (isNew) toast(`femboy spotify ${info.version} is out ♡ click the bell`);
  } catch (e) {
    console.warn('update check failed', e); // offline or no release yet: stay quiet
  }
}

function showUpdate() {
  const u = S.update;
  $('update-text').textContent = `version ${u.version} is ready (you have ${u.current})`;
  $('update-notes').textContent = u.notes || '';
  $('update-notes').classList.toggle('hidden', !u.notes);
  $('update').classList.remove('hidden');
}

$('update-later').onclick = () => $('update').classList.add('hidden');
$('update-go').onclick = async () => {
  const btn = $('update-go');
  btn.disabled = true;
  btn.textContent = 'downloading...';
  try {
    await invoke('install_update'); // the app restarts when this succeeds
  } catch (e) {
    toast(`update failed: ${e}`);
    btn.disabled = false;
    btn.textContent = 'update & restart';
  }
};
listen('update-progress', ([done, total]) => {
  if (total) $('update-go').textContent = `downloading ${Math.round((done / total) * 100)}%`;
});
$('btn-friends').onclick = () => toast('friend activity is coming soon~');
$('btn-avatar').onclick = () => toast(S.me ? `logged in as ${S.me.name} ♡ (ribbon in the sidebar = log out)` : 'not logged in');

$('btn-play').onclick = togglePlay;
$('btn-next').onclick = () => next();
$('btn-prev').onclick = prev;
$('btn-shuffle').onclick = () => {
  S.shuffle = !S.shuffle;
  renderPlayState();
  setOption('shuffle', S.shuffle);
  toast(S.shuffle ? 'shuffle on ✧' : 'shuffle off');
};
$('btn-repeat').onclick = () => {
  S.repeat = { off: 'all', all: 'one', one: 'off' }[S.repeat];
  renderPlayState();
  if (S.repeat === 'all') setOption('repeat_context', true);
  else if (S.repeat === 'one') setOption('repeat_track', true);
  else { setOption('repeat_track', false); setOption('repeat_context', false); }
  toast({ off: 'repeat off', all: 'repeat all ↻', one: 'repeat one ♡' }[S.repeat]);
};
$('btn-mute').onclick = () => {
  if (S.muted) setVolume(S.volume);
  else { S.muted = true; invoke('set_volume', { volume: 0 }).catch(() => {}); drawVolume(0); renderPlayState(); }
};
$('right-add').onclick = saveCurrent;
$('np-add').onclick = saveCurrent;
$('btn-lyrics').onclick = () => (S.view.name === 'lyrics' ? goBack() : go({ name: 'lyrics' }));
$('lyrics-open').onclick = () => go({ name: 'lyrics' });
$('btn-queue').onclick = () => (S.view.name === 'queue' ? goBack() : go({ name: 'queue' }));
$('btn-devices').onclick = () => toggleDevices();
$('btn-panel').onclick = () => {
  const hidden = $('app').classList.toggle('no-right');
  $('btn-panel').classList.toggle('active', !hidden);
};
$('btn-mini').onclick = () => $('btn-panel').click();
$('btn-full').onclick = async () => {
  if (!appWindow) return toast('fullscreen works in the app ♡');
  await appWindow.setFullscreen(!(await appWindow.isFullscreen()));
};
$('win-min').onclick = () => appWindow?.minimize();
$('win-max').onclick = () => appWindow?.toggleMaximize();
$('win-close').onclick = () => (appWindow ? appWindow.close() : toast('bye bye~ ♡'));

let searchTimer;
$('search').addEventListener('input', (e) => {
  clearTimeout(searchTimer);
  const q = e.target.value;
  searchTimer = setTimeout(() => doSearch(q), 450);
});
$('search').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') { clearTimeout(searchTimer); doSearch(e.target.value); }
  if (e.key === 'Escape') e.target.blur();
});

document.addEventListener('keydown', (e) => {
  if (e.target.tagName === 'INPUT') return;
  if (e.code === 'Space') { e.preventDefault(); togglePlay(); }
  else if (e.key === 'ArrowRight' && current()) seekTo(Math.min(position() + 10000, current().duration_ms - 1000));
  else if (e.key === 'ArrowLeft' && current()) seekTo(Math.max(position() - 10000, 0));
  else if (e.key === 'ArrowUp') { e.preventDefault(); setVolume(S.volume + 5); }
  else if (e.key === 'ArrowDown') { e.preventDefault(); setVolume(S.volume - 5); }
  else if (e.key === 'n') next();
  else if (e.key === 'p') prev();
  else if (e.key === '/' || (e.ctrlKey && e.key === 'l')) { e.preventDefault(); $('search').focus(); }
  else if (e.altKey && e.key === 'ArrowLeft') goBack();
  else if (e.altKey && e.key === 'ArrowRight') goForward();
});
document.addEventListener('mouseup', (e) => { if (e.button === 3) goBack(); if (e.button === 4) goForward(); });

/* ───────────── customize banner ───────────── */

let pendingBanner;
function openCustomize() {
  pendingBanner = undefined;
  $('banner-title').value = store.get('bannerTitle', '');
  $('banner-file').value = '';
  $('customize').classList.remove('hidden');
}
$('customize').addEventListener('click', (e) => { if (e.target.id === 'customize') $('customize').classList.add('hidden'); });
$('banner-file').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  pendingBanner = await downscale(file, 1800);
});
$('banner-reset').onclick = () => {
  localStorage.removeItem('banner');
  localStorage.removeItem('bannerTitle');
  $('customize').classList.add('hidden');
  if (S.view.name === 'home') render();
  toast('banner reset ♡');
};
$('banner-save').onclick = () => {
  store.set('bannerTitle', $('banner-title').value.trim());
  if (pendingBanner && !store.set('banner', pendingBanner)) toast('that picture is too big, try a smaller one~');
  $('customize').classList.add('hidden');
  if (S.view.name === 'home') render();
  else go({ name: 'home' });
};

function downscale(file, maxW) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const im = new Image();
      im.onload = () => {
        const scale = Math.min(1, maxW / im.width);
        const c = document.createElement('canvas');
        c.width = Math.round(im.width * scale);
        c.height = Math.round(im.height * scale);
        c.getContext('2d').drawImage(im, 0, 0, c.width, c.height);
        resolve(c.toDataURL('image/jpeg', 0.86));
      };
      im.onerror = reject;
      im.src = reader.result;
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

/* ───────────── login / startup ───────────── */

async function doLogout() {
  if (!confirm('log out of femboy spotify? ♡')) return;
  await invoke('logout').catch(toast);
  location.reload();
}

$('btn-login').onclick = async () => {
  const btn = $('btn-login');
  btn.disabled = true;
  btn.textContent = 'waiting for the browser...';
  $('login-error').classList.add('hidden');
  $('login-text').textContent = 'finish signing in in your browser, then come back here ♡';
  try {
    await invoke('login');
    $('login').classList.add('hidden');
    await afterLogin();
  } catch (e) {
    const log = await invoke('log_file').catch(() => '');
    const box = $('login-error');
    box.textContent = `sign-in failed: ${e}${log ? `\n\nlog: ${log}` : ''}`;
    box.style.whiteSpace = 'pre-wrap';
    box.classList.remove('hidden');
    $('login-text').textContent = 'something went wrong (｡•́︿•̀｡) you can try again:';
  } finally {
    btn.disabled = false;
    btn.textContent = 'sign in with Spotify';
  }
};

const afterLogin = () => { checkPlayer(); return loadLibrary(); };

async function loadLibrary() {
  invoke('me').then((me) => {
    S.me = me;
    if (me.avatar) $('avatar-img').src = me.avatar;
    else $('avatar-img').src = PLACEHOLDER;
  }).catch(() => {});
  try {
    S.playlists = await invoke('playlists');
  } catch (e) { toast(`couldn't load playlists: ${e}`); }
  renderSideThumbs();
  if (S.view.name === 'home') render();
  invoke('liked').then((l) => { S.liked = l; cacheTracks(l); l.forEach((t) => S.saved.add(t.id)); }).catch(() => {});
  invoke('remote_state').then(applyRemote).catch(() => {});
}

listen('remote', applyRemote);

async function start() {
  hydrateIcons();
  $('avatar-img').src = PLACEHOLDER;
  drawVolume(S.volume);
  renderPlayState();
  render();
  renderNow();
  setInterval(tick, 250);
  checkForUpdate();
  setInterval(checkForUpdate, 6 * 60 * 60 * 1000);
  try {
    const ok = await invoke('init');
    if (ok) await afterLogin();
    else $('login').classList.remove('hidden');
  } catch (e) {
    toast(`couldn't start: ${e}`);
    $('login').classList.remove('hidden');
  }
}

/* ───────────── demo backend (plain browser preview) ───────────── */

function demoCover(seed, label) {
  const pal = [['#ffc6de', '#c9b2f5'], ['#bfe3ff', '#ffc2dc'], ['#ffe0c2', '#ff9fc6'], ['#d6c5ff', '#9fc9ff'], ['#ffd6ea', '#fff0b8'], ['#c9f1e4', '#d8c4ff']][seed % 6];
  const shapes = ['<path d="M50 74S24 58 20 44c-3-10 3-20 13-20 7 0 12 4 17 10 5-6 10-10 17-10 10 0 16 10 13 20-4 14-30 30-30 30z" fill="#fff" opacity=".9"/>',
    '<path d="M50 14c3 22 9 30 36 36-27 6-33 14-36 36-3-22-9-30-36-36 27-6 33-14 36-36z" fill="#fff" opacity=".9"/>',
    '<circle cx="50" cy="52" r="24" fill="#fff" opacity=".85"/><path d="M34 34 30 16l14 10M66 34l4-18-14 10" fill="#fff" opacity=".85"/>'][seed % 3];
  return 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${pal[0]}"/><stop offset="1" stop-color="${pal[1]}"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/>${shapes}<text x="50" y="94" font-family="sans-serif" font-size="9" text-anchor="middle" fill="#6d3462" opacity=".7">${label}</text></svg>`);
}

const DEMO_PLAYLISTS = ['Boykisser Mix', 'DJ', 'Meme Mix', 'onokami Mix', 'Spit It Out', 'LEGACY Radio', 'Shluzov Mix', 'Daily Mix 1', 'pink noise', 'sleepy girl hours', 'gym but cute', 'night drive ✧', 'hyperpop', 'study ♡']
  .map((name, i) => ({ id: 'pl' + i, name, owner: 'cutie', total: 20 + i * 7, image: demoCover(i, '') }));
const DEMO_TRACKS = [['MURDER EVERY 1 U KNOW!', 'femtanyl, takihasdied', 101000], ['strawberry milk', 'pastel kid', 184000], ['heart.exe', 'glitchbunny', 152000], ['pink sky', 'lavender', 201000], ['catboy summer', 'meowmix', 176000], ['sugar rush', 'candycore', 143000], ['blåhaj', 'shark gang', 165000], ['skirt season', 'thigh highs', 199000]]
  .map(([name, artists, d], i) => ({ id: 't' + i, uri: 'spotify:track:t' + i, name, artists, album: 'cute album vol. ' + (i % 3 + 1), duration_ms: d, image: demoCover(i + 2, ''), image_large: demoCover(i + 2, '') }));
// `var`-free on purpose: listen() runs before this part of the file is evaluated
function demoListen(ev, cb) { (demoListen.cbs ||= []).push([ev, cb]); }
function demoEmit(payload, ev = 'remote') { setTimeout(() => (demoListen.cbs || []).forEach(([e, cb]) => e === ev && cb(payload)), 250); }
async function demoInvoke(cmd, args = {}) {
  await new Promise((r) => setTimeout(r, 120));
  switch (cmd) {
    case 'init': return true;
    case 'player_status': return !location.search.includes('noplayer');
    case 'player_login': setTimeout(() => demoEmit(true, 'player'), 1500); return null;
    case 'me': return { name: 'cutie', avatar: demoCover(4, '') };
    case 'playlists': return DEMO_PLAYLISTS;
    case 'liked': case 'recent': case 'playlist_tracks': return DEMO_TRACKS;
    case 'search': return DEMO_TRACKS.filter((t) => (t.name + t.artists).toLowerCase().includes(args.q.toLowerCase()));
    case 'lyrics': return [{ start_ms: 0, words: 'this is a demo, no real lyrics here' }, { start_ms: 4000, words: 'log in with Spotify to hear the music' }, { start_ms: 8000, words: 'pink pink pink ♡' }, { start_ms: 12000, words: 'la la la~' }];
    // a pretend Spotify Connect device
    case 'remote_state': return demoState();
    case 'check_update': return location.search.includes('update') ? { current: '0.3.0', version: '0.4.0', notes: '- demo update notes\n- even more pink ♡' } : null;
    case 'play_dj': {
      demoRemote.active = args.device || demoRemote.active || demoDevices()[0].id;
      demoRemote.context = DJ_URI;
      demoRemote.uris = DEMO_TRACKS.map((t) => t.uri);
      demoRemote.uri = 'spotify:media:0000000000000000000000';
      demoRemote.playing = true;
      demoRemote.pos = 0;
      demoRemote.at = performance.now();
      demoEmit(demoState());
      setTimeout(() => { demoRemote.uri = 'spotify:media:t3'; demoEmit(demoState()); }, 2500);
      setTimeout(() => { demoRemote.uri = 'spotify:track:t3'; demoEmit(demoState()); }, 5000);
      return null;
    }
    case 'track_info': return DEMO_TRACKS.find((t) => t.uri === args.uri || t.uri === args.uri.replace('spotify:track:', 'spotify:track:t').replace('tt', 't')) || null;
    case 'play': {
      const d = demoDevices();
      demoRemote.active = args.device || demoRemote.active || d[0].id;
      demoRemote.uris = args.uris;
      demoRemote.context = '';
      demoRemote.uri = args.uris[args.index];
      demoRemote.playing = true;
      demoRemote.pos = 0;
      demoRemote.at = performance.now();
      demoEmit(demoState());
      return null;
    }
    case 'control': {
      const r = demoRemote;
      if (!r.active) throw NO_DEVICE;
      r.pos = demoPos();
      r.at = performance.now();
      if (args.action === 'pause') r.playing = false;
      if (args.action === 'resume') r.playing = true;
      if (args.action === 'skip_next' || args.action === 'skip_prev') {
        const i = r.uris.indexOf(r.uri) + (args.action === 'skip_next' ? 1 : -1);
        r.uri = r.uris[(i + r.uris.length) % r.uris.length];
        r.pos = 0;
      }
      demoEmit(demoState());
      return null;
    }
    case 'seek': demoRemote.pos = args.ms; demoRemote.at = performance.now(); demoEmit(demoState()); return null;
    case 'set_option': demoRemote[args.name] = args.value; demoEmit(demoState()); return null;
    case 'set_volume': demoRemote.volume = args.volume; return null;
    case 'transfer': demoRemote.active = args.device; demoEmit(demoState()); return null;
    default: return null;
  }
}

const demoRemote = { active: null, uris: [], uri: null, playing: false, pos: 0, at: 0, volume: 66, shuffle: false, repeat_context: false, repeat_track: false };
function demoDevices() {
  return [{ id: 'laptop', name: 'Demo Laptop', kind: 'computer' }, { id: 'phone', name: 'Demo Phone', kind: 'phone' }]
    .map((d) => ({ ...d, volume: demoRemote.volume, active: d.id === demoRemote.active }));
}
function demoPos() { return demoRemote.pos + (demoRemote.playing ? performance.now() - demoRemote.at : 0); }
function demoState() {
  const r = demoRemote;
  const dev = demoDevices().find((d) => d.active);
  const t = DEMO_TRACKS.find((x) => x.uri === r.uri);
  return {
    devices: demoDevices(), active_device: r.active, active_device_name: dev?.name || null,
    track_uri: r.active ? r.uri : null, context_uri: r.context || '', playing: r.playing && !!r.active, buffering: false,
    position_ms: Math.round(demoPos()), duration_ms: t?.duration_ms || 0,
    shuffle: r.shuffle, repeat_context: r.repeat_context, repeat_track: r.repeat_track,
    volume: r.active ? r.volume : null, next_uris: [],
  };
}

start();
