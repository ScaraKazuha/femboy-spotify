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

/* ───────────── light / dark theme ───────────── */

const darkQuery = matchMedia('(prefers-color-scheme: dark)');
const themeChoice = () => store.get('theme', 'light'); // light | dark | auto
const themeNow = () => {
  const forced = DEMO && location.search.match(/theme=(light|dark)/); // demo / screenshots
  if (forced && !store.get('themeTouched', false)) return forced[1];
  return themeChoice() === 'auto' ? (darkQuery.matches ? 'dark' : 'light') : themeChoice();
};

function applyTheme({ animate = false } = {}) {
  const root = document.documentElement;
  const next = themeNow();
  if (root.dataset.theme !== next && animate) {
    root.classList.add('theme-switching');
    setTimeout(() => root.classList.remove('theme-switching'), 450);
  }
  root.dataset.theme = next;
  const btn = document.getElementById('btn-theme');
  if (btn) {
    setIcon(btn, next === 'dark' ? 'sun' : 'moon');
    btn.title = next === 'dark' ? 'theme: dark (click to change)' : 'theme: light (click to change)';
  }
}
function setTheme(choice) {
  store.set('theme', choice);
  store.set('themeTouched', true);
  applyTheme({ animate: true });
}
darkQuery.addEventListener('change', () => { if (themeChoice() === 'auto') applyTheme({ animate: true }); });

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
// pictures for things that have none of their own
const PLACEHOLDER = 'art/icons/no-cover.png';
const NO_ARTIST = 'art/icons/no-artist.png';
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
  const view = { name: 'search', kind: 'search', q, title: `"${q}"`, tab: S.view.name === 'search' ? S.view.tab : 'all', tracks: [], loading: true };
  go(view, { push: S.view.name !== 'search' });
  try {
    const r = await invoke('search_all', { q });
    cacheTracks(r.tracks);
    rememberCards([...r.artists, ...r.albums]);
    if (seq !== searchSeq) return;
    view.results = r;
    view.tracks = r.tracks;
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
  else if (v.name === 'queue') main.innerHTML = queueHtml(v);
  else if (v.name === 'lyrics') main.innerHTML = lyricsViewHtml();
  else if (v.name === 'artist') main.innerHTML = artistHtml(v);
  else if (v.name === 'search') main.innerHTML = searchHtml(v);
  else if (v.name === 'recent') main.innerHTML = recentHtml(v);
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
      <button class="art-play" id="hero-play" title="play ${esc(featuredPlaylist()?.name || 'Liked Songs')}">${icon('play')}</button>
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
    `<button class="tile" data-open="liked"><img class="tile-art liked-cover" src="art/icons/liked-cover.png" alt=""><span>Liked Songs</span></button>`,
    `<button class="tile" data-open="dj" title="Spotify DJ"><img class="dj-art" src="art/headphones.png" alt=""><span>DJ X</span></button>`,
  ]
    .concat(S.playlists.slice(0, 6).map((p) => `<button class="tile" data-playlist="${esc(p.id)}"><img src="${img(p.image)}" alt=""><span>${esc(p.name)}</span></button>`))
    .join('');
  const cards = (S.filter === 'playlists' ? S.playlists : S.playlists.slice(6))
    .map(cardHtml).join('');
  const head = (title) => `<div class="section-head"><h2>${esc(title)}</h2><div class="line">${icon('heart')}</div></div>`;
  // Spotify's own shelves: Daily Mixes, Release Radar, top mixes... (from the built-in player)
  const shelves = S.filter === 'all'
    ? (S.home || []).map((s) => `${head(s.title)}<div class="cards shelf">${s.items.map(homeCardHtml).join('')}</div>`).join('')
    : '';
  return `
    ${heroHtml()}
    <div class="chips">${chips}</div>
    ${S.filter === 'all' ? `<div class="tiles">${tiles}</div>` : ''}
    ${shelves}
    ${cards ? `${head('Your playlists')}<div class="cards">${cards}</div>` : ''}
    ${!S.playlists.length ? `${loadingHtml('loading your library...')}` : ''}`;
}

function homeCardHtml(p) {
  const dj = p.id === DJ_URI.split(':').pop();
  return `<button class="card-item" ${dj ? 'data-open="dj"' : `data-playlist="${esc(p.id)}"`}>
      <img class="cov" src="${p.image ? img(p.image) : PLACEHOLDER}" alt="" loading="lazy">
      <div class="t">${esc(p.name)}</div>
      <div class="st">${esc(p.sub || 'Spotify')}</div>
    </button>`;
}

// Spotify's home shelves arrive from the built-in player a little after start
function setHome(shelves) {
  S.home = shelves || [];
  S.homeItems = new Map();
  S.home.forEach((s) => s.items.forEach((it) => S.homeItems.set(it.id, { id: it.id, name: it.name, owner: 'Spotify', image: it.image, total: 0 })));
  if (S.view.name === 'home') render();
}
listen('home', setHome);

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
      <button class="card-item" id="create-playlist"><img class="cov" src="art/icons/create.png" alt=""><div class="t">Create playlist</div><div class="st">a new one, just yours</div></button>
      <button class="card-item" data-open="liked"><img class="cov liked-cover" src="art/icons/liked-cover.png" alt=""><div class="t">Liked Songs</div><div class="st">♡</div></button>
      <button class="card-item" data-open="dj"><img class="cov dj-art" src="art/headphones.png" alt=""><div class="t">DJ X</div><div class="st">Spotify DJ ♪</div></button>
      ${S.playlists.map(cardHtml).join('')}
    </div>`;
}

function listHtml(v) {
  const cover = v.kind === 'liked'
    ? `<img class="list-cover liked-cover" src="art/icons/liked-cover.png" alt="">`
    : `<img class="list-cover" src="${img(v.image || v.tracks?.[0]?.image)}" alt="">`;
  const kinds = { liked: 'playlist', playlist: 'playlist', search: 'search', recent: 'history', queue: 'queue', album: 'album', radio: 'song radio' };
  let body;
  if (v.loading) body = loadingHtml();
  else if (!v.tracks?.length) body = `${emptyHtml('nothing here yet~')}`;
  else {
    v.shown = shownTracks(v);
    body = v.shown.length ? tracksHtml(v.shown, v.kind, rowOpts(v)) : '<div class="tracks"><div class="empty small">no songs match (｡•́︿•̀｡)</div></div>';
  }
  const total = (v.tracks || []).reduce((s, t) => s + (t.duration_ms || 0), 0);
  const albumSaved = v.kind === 'album' && S.savedAlbums?.has(v.uri);
  const hasMenu = ['liked', 'playlist', 'album', 'radio'].includes(v.kind);
  return `
    <div class="list-head">${cover}<div>
      <div class="list-kind">${kinds[v.kind] || ''}</div>
      <h1 class="list-title" style="font-size:${(v.title || '').length > 24 ? 40 : 60}px">${esc(v.title || '')}</h1>
      <div class="list-meta">${v.subHtml || esc(v.sub || '')}${v.tracks?.length ? ` · ${v.tracks.length} songs, ${fmtLong(total)}` : ''}</div>
    </div></div>
    ${v.tracks?.length ? `<div class="list-actions">
      <button class="round-play" id="list-play" title="play">${icon('play')}</button>
      <button class="icon-btn" id="list-shuffle" title="shuffle play">${icon('shuffle')}</button>
      ${v.kind === 'album' ? `<button class="icon-btn save-btn ${albumSaved ? 'on' : ''}" id="album-save" title="${albumSaved ? 'remove from Your Library' : 'save to Your Library'}">${icon(albumSaved ? 'album-saved' : 'save-album')}</button>` : ''}
      ${hasMenu ? `<button class="icon-btn" id="list-more" title="more options for ${esc(v.title || '')}">${icon('dots')}</button>` : ''}
      ${listToolsHtml(v)}
    </div>` : ''}
    ${body}`;
}

const loadingHtml = (text = 'loading... ₍ᐢ. .ᐢ₎') => `<div class="empty"><img class="loading-art" src="art/icons/loading.png" alt="">${text}</div>`;
const emptyHtml = (text) => `<div class="empty"><img class="empty-art" src="art/icons/empty.png" alt="">${text}</div>`;
const sectionHead = (title) => `<div class="section-head"><h2>${esc(title)}</h2><div class="line">${icon('heart')}</div></div>`;

// every artist name links to their page (older saved history has names only)
function artistLinks(t) {
  const list = t?.artist_list || [];
  if (!list.length) return esc(t?.artists || '');
  return list.map((a) => `<span class="alink" data-artist="${esc(a.uri)}">${esc(a.name)}</span>`).join(', ');
}

// the tracks behind the rows on screen (row data-index points into this)
function viewTracks() {
  return S.view.shown || S.view.tracks || [];
}

// play row i of what's on screen (sorted / filtered lists send the list itself)
function playRow(i) {
  const v = S.view;
  const list = viewTracks();
  if (!list[i]) return;
  if (v.name === 'queue') return playFrom(list, i, S.context || 'queue', null);
  playFrom(list, i, v.title, isReordered(v) ? null : contextOf(v));
}

function selectRow(i) {
  main.querySelectorAll('.trow.selected').forEach((r) => r.classList.remove('selected'));
  main.querySelector(`.trow[data-index="${i}"]`)?.classList.add('selected');
}

function likeButton(t, i) {
  const saved = S.saved.has(t.id);
  return `<button class="tlike ${saved ? 'on' : ''}" data-like="${i}" title="${saved ? 'remove from Liked Songs' : 'save to Liked Songs'}"><img class="ico art" src="art/icons/${saved ? 'liked' : 'add'}.png" alt="" draggable="false"></button>`;
}

// "7 minutes ago", "2 days ago", "3 weeks ago", then a date — like Spotify's "Date added"
function ago(ms) {
  if (!ms) return '';
  const s = (Date.now() - ms) / 1000;
  const n = (v, unit) => `${v} ${unit}${v === 1 ? '' : 's'} ago`;
  if (s < 60) return 'just now';
  if (s < 3600) return n(Math.floor(s / 60), 'minute');
  if (s < 86400) return n(Math.floor(s / 3600), 'hour');
  if (s < 7 * 86400) return n(Math.floor(s / 86400), 'day');
  if (s < 30 * 86400) return n(Math.floor(s / (7 * 86400)), 'week');
  return new Date(ms).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

// "26 hr 32 min" / "3 min 20 sec"
function fmtLong(ms) {
  const m = Math.round(ms / 60000);
  if (m >= 60) return `${Math.floor(m / 60)} hr ${m % 60} min`;
  const s = Math.round(ms / 1000);
  return m >= 1 ? `${Math.floor(s / 60)} min ${s % 60} sec` : `${s} sec`;
}

// `offset` numbers the rows when one list is split into sections (the queue)
function tracksHtml(tracks, kind, { album = true, dates = false, offset = 0, head = true } = {}) {
  const cur = current();
  const cls = [album ? '' : 'no-album', dates ? 'has-date' : ''].join(' ');
  return `<div class="tracks ${cls}">
    ${head ? `<div class="trow head"><div class="tnum">#</div><div>title</div>${album ? '<div>album</div>' : ''}${dates ? '<div>date added</div>' : ''}<div></div><div class="tdur">${icon('clock', 'head-clock')}</div><div></div></div>` : ''}
    ${tracks.map((t, n) => {
      const i = n + offset;
      const playing = cur && cur.uri === t.uri;
      const albumCell = t.album_uri ? `<span class="alink" data-album="${esc(t.album_uri)}">${esc(t.album)}</span>` : esc(t.album || '');
      return `<div class="trow ${playing ? 'playing' : ''}" data-index="${i}" data-n="${n + 1}">
        <div class="tnum"><span class="n">${playing ? '♪' : n + 1}</span><button class="tplay" data-play="${i}" title="play ${esc(t.name)}">${icon('play')}</button></div>
        <div class="tmain"><img src="${img(t.image)}" alt="" loading="lazy"><div class="ttext"><div class="tname">${esc(t.name)}</div><div class="tartist">${artistLinks(t)}</div></div></div>
        ${album ? `<div class="talbum">${albumCell}</div>` : ''}
        ${dates ? `<div class="tdate">${ago(t.added_at)}</div>` : ''}
        ${likeButton(t, i)}
        <div class="tdur">${fmt(t.duration_ms)}</div>
        <button class="tmore" data-more="${i}" title="more options for ${esc(t.name)}">${icon('dots')}</button>
      </div>`;
    }).join('')}
  </div>`;
}

/* playlist search + sort (what the rows show; rows point into view.shown) */
const SORTS = { custom: 'Custom order', title: 'Title', artist: 'Artist', album: 'Album', added: 'Date added', duration: 'Duration' };
function shownTracks(v) {
  let list = v.tracks || [];
  const q = (v.filterText || '').trim().toLowerCase();
  if (q) list = list.filter((t) => `${t.name} ${t.artists} ${t.album}`.toLowerCase().includes(q));
  // text sorts keep songs with no name / artist / album at the end
  const text = (f) => (a, b) => (!f(a)) - (!f(b)) || (f(a) || '').localeCompare(f(b) || '');
  const by = {
    title: text((t) => t.name),
    artist: text((t) => t.artists),
    album: text((t) => t.album),
    added: (a, b) => (b.added_at || 0) - (a.added_at || 0),
    duration: (a, b) => a.duration_ms - b.duration_ms,
  }[v.sortKey];
  if (by) list = list.slice().sort(by);
  return list;
}
const isReordered = (v) => !!((v.filterText || '').trim() || (v.sortKey && v.sortKey !== 'custom'));

function listToolsHtml(v) {
  if (!['liked', 'playlist', 'album'].includes(v.kind)) return '';
  return `<div class="list-tools">
    <label class="list-find ${v.filterText ? 'open' : ''}" title="search in ${esc(v.title || 'this list')}">
      ${icon('search-in-list')}<input id="list-find" type="text" placeholder="Search in ${v.kind === 'album' ? 'album' : 'playlist'}" value="${esc(v.filterText || '')}" spellcheck="false" autocomplete="off">
    </label>
    ${v.kind !== 'album' ? `<button class="chip sort-btn" id="list-sort" title="sort">${esc(SORTS[v.sortKey || 'custom'])} ${icon('sort')}</button>` : ''}
  </div>`;
}

// re-draw only the rows (keeps the search box focused while typing)
function refreshRows() {
  const v = S.view;
  const box = main.querySelector('.tracks');
  if (!box || !v.tracks) return render();
  v.shown = shownTracks(v);
  const tmp = document.createElement('div');
  tmp.innerHTML = v.shown.length ? tracksHtml(v.shown, v.kind, rowOpts(v)) : '<div class="tracks"><div class="empty small">no songs match (｡•́︿•̀｡)</div></div>';
  box.replaceWith(tmp.firstElementChild);
  hydrateIcons(main);
}
const rowOpts = (v) => ({ album: v.kind !== 'album', dates: v.kind === 'liked' || v.kind === 'playlist' });

/* ───────────── artist & album pages ───────────── */

async function openArtist(uri) {
  if (!uri) return;
  if (S.view.name === 'artist' && S.view.uri === uri) return;
  const view = { name: 'artist', uri, loading: true, tracks: [] };
  go(view);
  try {
    const a = await invoke('artist_page', { uri });
    // (keep view.name: it says which page this is; the artist's name goes to title)
    Object.assign(view, { image: a.image, top: a.top, albums: a.albums, singles: a.singles, loading: false, title: a.name, tracks: a.top });
    cacheTracks(a.top);
  } catch (e) { return fail(e); }
  if (S.view === view) render();
}

async function openAlbum(uri) {
  if (!uri) return;
  if (S.view.kind === 'album' && S.view.uri === uri) return;
  const view = { name: 'list', kind: 'album', uri, title: '', loading: true, tracks: null };
  go(view);
  try {
    const al = await invoke('album_page', { uri });
    cacheTracks(al.tracks);
    Object.assign(view, {
      title: al.name,
      image: al.image,
      subHtml: `${al.artists.map((a) => `<span class="alink" data-artist="${esc(a.uri)}">${esc(a.name)}</span>`).join(', ')}${al.year ? ` · ${al.year}` : ''}`,
      sub: al.artists.map((a) => a.name).join(', '),
      tracks: al.tracks,
      loading: false,
    });
  } catch (e) { return fail(e); }
  if (S.view === view) render();
}

async function openRadio(t) {
  const view = { name: 'list', kind: 'radio', title: `${t.name} Radio`, sub: `songs like ${t.name} by ${t.artists}`, image: t.image_large || t.image, tracks: null, loading: true };
  go(view);
  try { view.tracks = await invoke('song_radio', { id: t.id }); cacheTracks(view.tracks); } catch (e) { return fail(e); }
  view.loading = false;
  if (S.view === view) render();
}

/* ───────────── cards (playlists / albums / artists) ───────────── */

function cardHtml2(c) {
  const round = c.kind === 'artist' ? 'round' : '';
  return `<button class="card-item ${round}" data-card="${esc(c.uri)}">
      <img class="cov" src="${c.image ? img(c.image) : c.kind === 'artist' ? NO_ARTIST : PLACEHOLDER}" alt="" loading="lazy">
      <div class="t">${esc(c.name)}</div>
      <div class="st">${esc(c.sub || c.kind)}</div>
    </button>`;
}

function openCard(uri) {
  if (uri.startsWith('spotify:artist:')) return openArtist(uri);
  if (uri.startsWith('spotify:album:')) return openAlbum(uri);
  if (uri.includes(':collection')) return openLiked();
  if (uri.startsWith('spotify:playlist:')) {
    const id = uri.split(':').pop();
    const known = S.playlists.find((p) => p.id === id) || S.homeItems?.get(id);
    const c = S.cards?.get(uri);
    return openPlaylist(known || { id, name: c?.name || 'playlist', owner: c?.sub, image: c?.image });
  }
}
const rememberCards = (cards) => { S.cards ||= new Map(); cards.forEach((c) => S.cards.set(c.uri, c)); };

/* ───────────── search with tabs ───────────── */

function searchHtml(v) {
  const r = v.results;
  const tabs = [['all', 'All'], ['songs', 'Songs'], ['artists', 'Artists'], ['albums', 'Albums']]
    .map(([k, l]) => `<button class="chip ${v.tab === k ? 'active' : ''}" data-tab="${k}">${l}</button>`).join('');
  let body;
  if (v.loading || !r) body = loadingHtml();
  else if (!r.tracks.length) body = `${emptyHtml(`nothing found for "${esc(v.q)}"`)}`;
  else if (v.tab === 'songs') body = tracksHtml(r.tracks, 'search');
  else if (v.tab === 'artists') body = `<div class="cards">${r.artists.map(cardHtml2).join('')}</div>`;
  else if (v.tab === 'albums') body = `<div class="cards">${r.albums.map(cardHtml2).join('')}</div>`;
  else {
    const top = r.artists[0];
    body = `<div class="search-top">
        ${top ? `<div><h2 class="mini-head">Top result</h2>
          <button class="top-result" data-card="${esc(top.uri)}"><img src="${top.image ? img(top.image) : NO_ARTIST}" alt=""><div class="t">${esc(top.name)}</div><div class="st">artist</div></button></div>` : ''}
        <div class="search-songs"><h2 class="mini-head">Songs</h2>${tracksHtml(r.tracks.slice(0, 4), 'search', { album: false, head: false })}</div>
      </div>
      ${r.artists.length ? `${sectionHead('Artists')}<div class="cards">${r.artists.slice(0, 6).map(cardHtml2).join('')}</div>` : ''}
      ${r.albums.length ? `${sectionHead('Albums')}<div class="cards">${r.albums.slice(0, 6).map(cardHtml2).join('')}</div>` : ''}`;
  }
  return `<div class="search-page"><div class="chips search-tabs">${tabs}</div>${body}</div>`;
}

/* ───────────── recently played (from Spotify) ───────────── */

async function openRecent() {
  const history = store.get('history', []);
  const view = { name: 'recent', title: 'Recently played', cards: S.recentCards || null, tracks: history };
  go(view);
  try {
    const cards = await invoke('recent_spotify');
    rememberCards(cards);
    S.recentCards = cards;
    view.cards = cards;
  } catch (e) { view.cards = []; view.error = String(e); }
  if (S.view === view) render();
}

function recentHtml(v) {
  return `
    <div class="list-head"><div class="list-cover recent-art">${icon('pie')}</div><div>
      <div class="list-kind">history</div>
      <h1 class="list-title">Recently played</h1>
      <div class="list-meta">where you listened lately, on any device</div>
    </div></div>
    ${v.cards === null ? loadingHtml() : v.cards.length ? `<div class="cards">${v.cards.map(cardHtml2).join('')}</div>` : `<div class="empty small">${esc(v.error || 'nothing yet~')}</div>`}
    ${v.tracks.length ? `${sectionHead('Songs you played here')}${tracksHtml(v.tracks, 'recent')}` : ''}`;
}

/* ───────────── queue (like Spotify's) ───────────── */

async function openQueue() {
  const view = { name: 'queue', title: 'Queue', tracks: [] };
  go(view);
  await refreshQueue();
}

// fetch info for queued uris we haven't seen yet, then draw
async function refreshQueue() {
  const v = S.view;
  if (v.name !== 'queue') return;
  const uris = [...(S.queueUris || []), ...(S.nextUris || [])];
  const missing = uris.filter((u) => !S.cache.has(u));
  if (missing.length) {
    try { cacheTracks(await invoke('tracks_info', { uris: missing })); } catch { /* show what we have */ }
  }
  if (S.view === v) render();
}

function queueHtml(v) {
  const queued = (S.queueUris || []).map((u) => S.cache.get(u)).filter(Boolean);
  const rest = (S.nextUris || []).filter((u) => !(S.queueUris || []).includes(u)).map((u) => S.cache.get(u)).filter(Boolean);
  v.tracks = [...queued, ...rest];
  v.shown = null;
  const now = current();
  return `
    <div class="list-head"><div class="list-cover recent-art">${icon('bookmark')}</div><div>
      <div class="list-kind">queue</div><h1 class="list-title">Queue</h1>
      <div class="list-meta">${S.activeDeviceName ? `playing on ${esc(S.activeDeviceName)}` : 'nothing is playing'}</div>
    </div></div>
    ${now ? `${sectionHead('Now playing')}
      <div class="tracks now-row"><div class="trow playing"><div class="tnum"><span class="n">♪</span></div>
        <div class="tmain"><img src="${img(now.image)}" alt=""><div class="ttext"><div class="tname">${esc(now.name)}</div><div class="tartist">${artistLinks(now)}</div></div></div>
        <div class="talbum">${now.album_uri ? `<span class="alink" data-album="${esc(now.album_uri)}">${esc(now.album || '')}</span>` : esc(now.album || '')}</div>
        <div></div><div class="tdur">${fmt(now.duration_ms || 0)}</div><div></div></div></div>` : ''}
    ${queued.length ? `<div class="queue-head">${sectionHead('Next in queue')}<button class="chip icon-chip" id="queue-clear">${icon('clear-queue')}Clear queue</button></div>
      ${tracksHtml(queued, 'queue', { head: false })}` : ''}
    ${rest.length ? `${sectionHead(`Next from: ${S.context || 'what\'s playing'}`)}${tracksHtml(rest, 'queue', { head: false, offset: queued.length })}` : ''}
    ${!now && !v.tracks.length ? `${emptyHtml('play something and it shows up here~')}` : ''}`;
}

async function clearQueue() {
  try {
    await invoke('clear_queue');
    S.queueUris = [];
    render();
    toast('queue cleared');
  } catch (e) { handleRemoteError(e); }
}

/* ───────────── follow artists, save albums ───────────── */

async function loadFollows() {
  try {
    const [artists, coll] = await Promise.all([invoke('collection_set', { set: 'artist' }), invoke('collection_set', { set: 'collection' })]);
    S.followed = new Set(artists);
    S.savedAlbums = new Set(coll.filter((u) => u.startsWith('spotify:album:')));
  } catch { S.followed ||= new Set(); S.savedAlbums ||= new Set(); }
}

async function toggleFollow(uri) {
  if (!uri) return;
  const artist = uri.startsWith('spotify:artist:');
  const set = artist ? (S.followed ||= new Set()) : (S.savedAlbums ||= new Set());
  const on = !set.has(uri);
  const apply = (v) => { if (v) set.add(uri); else set.delete(uri); render(); };
  apply(on);
  try {
    await invoke('set_followed', { uri, on });
    toast(artist ? (on ? 'following ♡' : 'unfollowed') : (on ? 'saved to Your Library ♡' : 'removed from Your Library'));
  } catch (e) { apply(!on); toast(`couldn't update: ${e}`); }
}

/* ───────────── your own playlists ───────────── */

// small pink dialog asking for a name (or a yes/no when `confirmOnly`)
function askName({ title, value = '', ok = 'save', confirmOnly = false, text = '' }) {
  return new Promise((resolve) => {
    const box = $('askbox');
    $('ask-title').textContent = title;
    $('ask-text').textContent = text;
    $('ask-text').classList.toggle('hidden', !text);
    const input = $('ask-input');
    input.classList.toggle('hidden', confirmOnly);
    input.value = value;
    $('ask-ok').textContent = ok;
    box.classList.remove('hidden');
    if (!confirmOnly) setTimeout(() => { input.focus(); input.select(); }, 30);
    const done = (result) => {
      box.classList.add('hidden');
      $('ask-ok').onclick = $('ask-cancel').onclick = input.onkeydown = null;
      resolve(result);
    };
    $('ask-ok').onclick = () => done(confirmOnly ? true : input.value.trim() || null);
    $('ask-cancel').onclick = () => done(null);
    input.onkeydown = (e) => { if (e.key === 'Enter') $('ask-ok').click(); if (e.key === 'Escape') done(null); };
  });
}

async function reloadPlaylists() {
  try { S.playlists = await invoke('playlists'); } catch { return; }
  renderSideThumbs();
}

async function createPlaylist() {
  const n = S.playlists.filter((p) => p.owner === 'you').length + 1;
  const name = await askName({ title: 'new playlist ♡', value: `My Playlist #${n}`, ok: 'create' });
  if (!name) return;
  try {
    const id = await invoke('playlist_create', { name });
    toast(`made ${name} ♡`);
    await reloadPlaylists();
    openPlaylist(S.playlists.find((p) => p.id === id) || { id, name, owner: 'you', editable: true });
  } catch (e) { toast(`couldn't make the playlist: ${e}`); }
}

async function renamePlaylist(p) {
  const name = await askName({ title: 'rename playlist', value: p.name, ok: 'rename' });
  if (!name || name === p.name) return;
  try {
    await invoke('playlist_rename', { playlist: p.id, name });
    p.name = name;
    if (S.view.kind === 'playlist' && S.view.id === p.id) S.view.title = name;
    await reloadPlaylists();
    render();
    toast('renamed ♡');
  } catch (e) { toast(`couldn't rename: ${e}`); }
}

async function removePlaylist(p) {
  const own = p.owner === 'you';
  const ok = await askName({
    title: own ? `delete ${p.name}?` : `remove ${p.name}?`,
    text: own ? 'It disappears from Your Library.' : 'It leaves Your Library; you can follow it again later.',
    ok: own ? 'delete' : 'remove',
    confirmOnly: true,
  });
  if (!ok) return;
  try {
    await invoke('playlist_unfollow', { playlist: p.id });
    await reloadPlaylists();
    toast(own ? 'deleted' : 'removed from Your Library');
    go({ name: 'home' });
  } catch (e) { toast(`couldn't remove it: ${e}`); }
}

/* "•••" next to a playlist's / album's play button */
function openListMenu(btn) {
  const v = S.view;
  const items = [];
  const link = (u) => `https://open.spotify.com/${u.replace(/^spotify:/, '').replace(':', '/')}`;
  if (v.tracks?.length) {
    items.push({ icon: 'queue', label: 'Add to queue', run: () => addAllToQueue(v.tracks) });
    items.push({ icon: 'shuffle', label: 'Shuffle play', run: () => $('list-shuffle')?.click() });
  }
  if (v.kind === 'playlist') {
    const p = S.playlists.find((x) => x.id === v.id);
    if (p?.owner === 'you') items.push('-', { icon: 'rename', label: 'Rename', run: () => renamePlaylist(p) }, { icon: 'delete', label: 'Delete', run: () => removePlaylist(p) });
    else if (p) items.push('-', { icon: 'minus', label: 'Remove from Your Library', run: () => removePlaylist(p) });
    items.push('-', { icon: 'link', label: 'Copy link to playlist', run: () => copyText(`https://open.spotify.com/playlist/${v.id}`, 'playlist link') });
  }
  if (v.kind === 'album') {
    const saved = S.savedAlbums?.has(v.uri);
    items.push('-', { icon: saved ? 'album-saved' : 'save-album', label: saved ? 'Remove from Your Library' : 'Save to Your Library', run: () => toggleFollow(v.uri) });
    const firstArtist = v.tracks?.[0]?.artist_list?.[0];
    if (firstArtist) items.push({ icon: 'user', label: 'Go to artist', run: () => openArtist(firstArtist.uri) });
    items.push('-', { icon: 'link', label: 'Copy link to album', run: () => copyText(link(v.uri), 'album link') });
  }
  if (items.length) showMenu(items, btn);
}

async function addAllToQueue(tracks) {
  const list = tracks.slice(0, 50);
  toast(`adding ${list.length} songs to the queue...`);
  for (const t of list) {
    try { await invoke('add_to_queue', { uri: t.uri }); } catch (e) { return handleRemoteError(e); }
  }
  toast('added to queue ♪');
}

function openSortMenu(btn) {
  const v = S.view;
  showMenu(Object.entries(SORTS).map(([k, label]) => ({
    icon: (v.sortKey || 'custom') === k ? 'check-circle' : '',
    label,
    run: () => { v.sortKey = k; render(); },
  })), btn);
}

function artistHtml(v) {
  if (v.loading) return loadingHtml();
  const cards = (list) => list.map((al) => `<button class="card-item" data-album="${esc(al.uri)}">
      <img class="cov" src="${al.image ? img(al.image) : PLACEHOLDER}" alt="" loading="lazy">
      <div class="t">${esc(al.name)}</div>
      <div class="st">${al.year || ''}</div>
    </button>`).join('');
  return `
    <div class="list-head artist-head"><img class="list-cover artist-cover" src="${v.image ? img(v.image) : NO_ARTIST}" alt=""><div>
      <div class="list-kind">artist</div>
      <h1 class="list-title" style="font-size:${v.title.length > 24 ? 40 : 60}px">${esc(v.title)}</h1>
      <div class="list-meta">${[v.albums.length && `${v.albums.length} albums`, v.singles.length && `${v.singles.length} singles & EPs`].filter(Boolean).join(' · ')}</div>
    </div></div>
    <div class="list-actions">
      ${v.top.length ? `<button class="round-play" id="list-play" title="play ${esc(v.title)}">${icon('play')}</button>
      <button class="icon-btn" id="list-shuffle" title="shuffle play">${icon('shuffle')}</button>` : ''}
      <button class="chip icon-chip follow-btn ${S.followed?.has(v.uri) ? 'active' : ''}" id="artist-follow">${icon(S.followed?.has(v.uri) ? 'following' : 'follow')}${S.followed?.has(v.uri) ? 'Following' : 'Follow'}</button>
    </div>
    ${v.top.length ? `${sectionHead('Popular')}${tracksHtml(v.top, 'artist')}` : ''}
    ${v.albums.length ? `${sectionHead('Albums')}<div class="cards">${cards(v.albums)}</div>` : ''}
    ${v.singles.length ? `${sectionHead('Singles and EPs')}<div class="cards">${cards(v.singles)}</div>` : ''}`;
}

function lyricsViewHtml() {
  const t = current();
  if (!t) return emptyHtml('play something first~');
  if (!S.lyrics) return `<div class="empty"><div class="big">♪</div>${S.lyricsFor === t.id ? 'no lyrics for this song (｡•́︿•̀｡)' : 'loading lyrics...'}</div>`;
  return `<div class="lyrics-view" id="lyrics-view">${S.lyrics.map((l, i) => `<p data-line="${i}" data-ms="${l.start_ms}">${esc(l.words)}</p>`).join('')}</div>`;
}

function updateSidebar() {
  const v = S.view;
  const active = v.name === 'library' ? 'library' : v.name === 'queue' ? 'queue' : v.kind === 'liked' ? 'liked' : v.name === 'recent' ? 'recent' : '';
  document.querySelectorAll('.side-btn[data-nav]').forEach((b) => b.classList.toggle('active', b.dataset.nav === active));
  $('btn-home').classList.toggle('on', v.name === 'home');
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
  const coverHtml = cover ? `<img src="${esc(cover)}" alt="">` : `<img class="cover-placeholder" src="art/icons/no-cover.png" alt="">`;
  $('right-cover').innerHTML = coverHtml;
  $('np-cover').innerHTML = t?.image ? `<img src="${esc(t.image)}" alt="">` : `<img class="cover-placeholder" src="art/icons/no-cover.png" alt="">`;
  $('right-name').textContent = t ? t.name : 'nothing yet~';
  $('right-artist').innerHTML = t ? artistLinks(t) : 'pick a song ♡';
  $('np-name').textContent = t ? t.name : '—';
  $('np-artist').innerHTML = t ? artistLinks(t) : '';
  for (const id of ['right-name', 'np-name']) $(id).classList.toggle('alink', !!t?.album_uri);
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
  const list = viewTracks();
  document.querySelectorAll('.trow[data-index]').forEach((row) => {
    const tr = list[+row.dataset.index]?.uri === t?.uri;
    row.classList.toggle('playing', !!tr);
    const n = row.querySelector('.tnum .n');
    if (n) n.textContent = tr ? '♪' : row.dataset.n;
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
  if (v.kind === 'album') return v.uri;
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

// pickDevice(), but first makes sure our own player is in the device list: Spotify
// only pushes the list when playback changes, so right after start it can be missing.
async function chooseDevice() {
  for (let i = 0; S.playerReady && !S.devices.some(isBuiltin) && i < 4; i++) {
    if (i) await new Promise((r) => setTimeout(r, 1500));
    try {
      const st = await invoke('refresh_devices');
      S.devices = st.devices || [];
      S.activeDevice = st.active_device;
    } catch { break; }
  }
  return pickDevice();
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
    await invoke('play', { context: S.ctx, uris: S.queue.map((x) => x.uri), index: i, device: await chooseDevice() });
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

let volTouched = 0;
let volSentAt = 0;
function drawVolume(v) {
  $('vol-fill').style.width = v + '%';
  $('vol-knob').style.left = v + '%';
}

// Our own player is turned up/down right on its page (instant); other devices get a
// Spotify Connect command (goes through Spotify, so it is sent less often).
const onBuiltin = () => {
  const d = S.devices.find((x) => x.id === S.activeDevice) || (!S.activeDevice && S.devices.find(isBuiltin));
  return !!d && isBuiltin(d);
};
function sendVolume(v) {
  if (onBuiltin()) {
    // (the player reports its new level to Spotify by itself, so other apps stay in step)
    invoke('player_volume', { volume: v }).catch(() => {});
  } else {
    invoke('set_volume', { volume: v }).catch(() => {});
  }
  volSentAt = performance.now();
}

// `live`: called while dragging; sends at most every 40 ms (own player) / 200 ms (others)
function setVolume(v, { live = false } = {}) {
  S.volume = Math.max(0, Math.min(100, Math.round(v)));
  S.muted = false;
  volTouched = performance.now();
  store.set('volume', S.volume);
  drawVolume(S.volume);
  renderPlayState();
  const gap = onBuiltin() ? 40 : 200;
  if (live && performance.now() - volSentAt < gap) return;
  sendVolume(S.volume);
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
    checkSleepOnTrackChange(t.uri);
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

// DJ X's voice only plays in the official Spotify app, so DJ X goes there when that
// app is open on this computer (Femboy Spotify still drives it); otherwise our player.
async function djDevice() {
  const here = await invoke('local_spotify').catch(() => null);
  if (here?.running && here.name) {
    try {
      const st = await invoke('refresh_devices');
      S.devices = st.devices || [];
      S.activeDevice = st.active_device;
    } catch { /* use the list we have */ }
    const app = S.devices.find((d) => !isBuiltin(d) && d.name.toLowerCase() === here.name.toLowerCase());
    if (app) return { id: app.id, voice: true };
  }
  return { id: await chooseDevice(), voice: false };
}

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
  try {
    const { id, voice } = await djDevice();
    toast(voice ? 'DJ X is on in your Spotify app, with his voice ♪' : 'DJ X is on the way ♪');
    if (!voice && !S.djVoiceHint) {
      S.djVoiceHint = true;
      setTimeout(() => toast('DJ X only talks in the Spotify app: open it to hear his voice ♡'), 3600);
    }
    await invoke('play_dj', { device: id });
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
  // the queue screen follows what Spotify says comes next
  const qKey = JSON.stringify([st.queue_uris, st.next_uris]);
  S.queueUris = st.queue_uris || [];
  S.nextUris = st.next_uris || [];
  if (qKey !== S.queueKey) { S.queueKey = qKey; if (S.view.name === 'queue') refreshQueue(); }
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

const saveCurrent = () => toggleSaved(current());

// Save to / remove from Liked Songs. The heart updates at once; Spotify confirms after.
async function toggleSaved(t, want) {
  if (!t?.id || !t.uri?.startsWith('spotify:track:')) return;
  const save = want ?? !S.saved.has(t.id);
  const apply = (on) => {
    if (on) { S.saved.add(t.id); if (S.liked && !S.liked.some((x) => x.id === t.id)) S.liked.unshift(t); }
    else { S.saved.delete(t.id); if (S.liked) S.liked = S.liked.filter((x) => x.id !== t.id); }
    if (S.view.kind === 'liked') { S.view.tracks = S.liked; render(); } else refreshSavedMarks();
    renderNow();
  };
  apply(save);
  try {
    await invoke('set_saved', { id: t.id, saved: save });
    toast(save ? 'added to Liked Songs ♡' : 'removed from Liked Songs');
  } catch (e) {
    apply(!save);
    toast(`couldn't update Liked Songs: ${e}`);
  }
}

// swap the hearts on the rows on screen (with a little pop)
function refreshSavedMarks() {
  const list = viewTracks();
  main.querySelectorAll('.tlike[data-like]').forEach((b) => {
    const t = list[+b.dataset.like];
    if (!t) return;
    const on = S.saved.has(t.id);
    if (b.classList.contains('on') === on) return;
    b.classList.toggle('on', on);
    b.dataset.tip = on ? 'remove from Liked Songs' : 'save to Liked Songs';
    const im = b.querySelector('img');
    im.src = `art/icons/${on ? 'liked' : 'add'}.png`;
    im.classList.remove('pop'); void im.offsetWidth; im.classList.add('pop');
  });
}

/* ───────────── song menu ("•••" / right click) ───────────── */

const menuEl = document.createElement('div');
menuEl.className = 'menu hidden';
document.body.append(menuEl);
const subEl = document.createElement('div');
subEl.className = 'menu sub hidden';
document.body.append(subEl);
let menuItems = [];
let menuOwner = null;

function closeMenu() {
  menuEl.classList.add('hidden');
  subEl.classList.add('hidden');
  menuOwner?.classList.remove('menu-open');
  menuOwner = null;
}

function menuHtml(items) {
  return items.map((it, i) => {
    if (it === '-') return '<div class="menu-sep"></div>';
    const pic = it.art ? `<img class="ico art" src="art/icons/${it.art}.png" alt="" draggable="false">` : it.icon ? icon(it.icon) : '<span class="ico"></span>';
    return `<button class="menu-item ${it.sub ? 'has-sub' : ''} ${it.on ? 'on' : ''}" data-mi="${i}" ${it.disabled ? 'disabled' : ''}>
      ${pic}<span class="mi-label">${esc(it.label)}</span>${it.sub ? icon('chevron-right', 'mi-arrow') : ''}</button>`;
  }).join('');
}

// place a menu at a point (or next to an element), kept inside the window
function placeMenu(el, x, y, { flipX = 0 } = {}) {
  el.style.left = '0px';
  el.style.top = '0px';
  el.classList.remove('hidden');
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  let left = x;
  if (left + w > innerWidth - 8) left = flipX ? flipX - w : innerWidth - w - 8;
  const top = Math.max(8, Math.min(y, innerHeight - h - 8));
  el.style.left = `${Math.max(8, left)}px`;
  el.style.top = `${top}px`;
}

function showMenu(items, at, owner) {
  closeMenu();
  menuItems = items;
  menuEl.innerHTML = menuHtml(items);
  hydrateIcons(menuEl);
  menuOwner = owner || null;
  menuOwner?.classList.add('menu-open');
  if (at instanceof Element) {
    const r = at.getBoundingClientRect();
    placeMenu(menuEl, r.right - 240, r.bottom + 6);
  } else placeMenu(menuEl, at.x, at.y);
  menuEl.classList.remove('open'); void menuEl.offsetWidth; menuEl.classList.add('open');
}

function openSub(btn, items) {
  subEl.innerHTML = menuHtml(items);
  subEl._items = items;
  const r = btn.getBoundingClientRect();
  placeMenu(subEl, r.right + 4, r.top - 6, { flipX: r.left - 4 });
  subEl.classList.remove('open'); void subEl.offsetWidth; subEl.classList.add('open');
}

function runItem(it, btn) {
  if (!it || it === '-' || it.disabled) return;
  if (it.sub) return openSub(btn, typeof it.sub === 'function' ? it.sub() : it.sub);
  closeMenu();
  it.run?.();
}
menuEl.addEventListener('click', (e) => {
  const b = e.target.closest('[data-mi]');
  if (b) runItem(menuItems[+b.dataset.mi], b);
});
menuEl.addEventListener('mouseover', (e) => {
  const b = e.target.closest('[data-mi]');
  if (!b) return;
  const it = menuItems[+b.dataset.mi];
  if (it?.sub) openSub(b, typeof it.sub === 'function' ? it.sub() : it.sub);
  else subEl.classList.add('hidden');
});
subEl.addEventListener('click', (e) => {
  const b = e.target.closest('[data-mi]');
  if (b) runItem(subEl._items[+b.dataset.mi], b);
});
document.addEventListener('mousedown', (e) => {
  if (!e.target.closest('.menu')) closeMenu();
}, true);
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeMenu(); });
window.addEventListener('blur', closeMenu);
window.addEventListener('resize', closeMenu);
main.addEventListener('scroll', closeMenu, { passive: true });

async function copyText(text, what) {
  try { await navigator.clipboard.writeText(text); toast(`${what} copied ♡`); } catch { toast(`couldn't copy the ${what}`); }
}

function openTrackMenu(t, at, { index = -1, owner } = {}) {
  if (!t?.uri?.startsWith('spotify:track:')) return;
  const v = S.view;
  const mine = S.playlists.filter((p) => p.editable);
  const inMyPlaylist = v.kind === 'playlist' && mine.some((p) => p.id === v.id);
  const saved = S.saved.has(t.id);
  const artists = t.artist_list || [];
  const link = `https://open.spotify.com/track/${t.id}`;
  const items = [
    { icon: 'plus', label: 'Add to playlist', sub: () => (mine.length ? mine.map((p) => ({ label: p.name, run: () => addToPlaylist(p, t) })) : [{ label: 'no playlists you can edit yet', disabled: true }]) },
    inMyPlaylist && { icon: 'minus', label: 'Remove from this playlist', run: () => removeFromPlaylist(v, t, index) },
    { art: saved ? 'unlike' : 'add', label: saved ? 'Remove from Liked Songs' : 'Save to Liked Songs', run: () => toggleSaved(t, !saved) },
    { icon: 'queue', label: 'Add to queue', run: () => addToQueue(t) },
    { icon: 'timer', label: sleep.until || sleep.endOfTrack ? `Sleep timer (${sleepLabel()})` : 'Sleep timer', sub: sleepItems },
    '-',
    { icon: 'radio', label: 'Go to song radio', run: () => openRadio(t) },
    artists.length > 1
      ? { icon: 'user', label: 'Go to artist', sub: artists.map((a) => ({ label: a.name, run: () => openArtist(a.uri) })) }
      : { icon: 'user', label: 'Go to artist', disabled: !artists.length, run: () => openArtist(artists[0]?.uri) },
    { icon: 'disc', label: 'Go to album', disabled: !t.album_uri, run: () => openAlbum(t.album_uri) },
    '-',
    { icon: 'share', label: 'Share', sub: [
      { icon: 'link', label: 'Copy song link', run: () => copyText(link, 'song link') },
      { icon: 'link', label: 'Copy Spotify URI', run: () => copyText(t.uri, 'Spotify URI') },
    ] },
  ].filter(Boolean);
  const row = owner || (index >= 0 ? main.querySelector(`.trow[data-index="${index}"]`) : null);
  showMenu(items, at, row);
}

async function addToPlaylist(p, t) {
  try {
    await invoke('playlist_add', { playlist: p.id, uri: t.uri });
    p.total = (p.total || 0) + 1;
    if (S.view.kind === 'playlist' && S.view.id === p.id && S.view.tracks) { S.view.tracks.push(t); render(); }
    toast(`added to ${p.name} ♡`);
  } catch (e) { toast(`couldn't add it: ${e}`); }
}

async function removeFromPlaylist(v, t, index) {
  try {
    await invoke('playlist_remove', { playlist: v.id, uri: t.uri });
    const p = S.playlists.find((x) => x.id === v.id);
    if (v.tracks) {
      const before = v.tracks.length;
      v.tracks = v.tracks.filter((x) => x.uri !== t.uri);
      if (p) p.total = Math.max(0, (p.total || before) - (before - v.tracks.length));
    }
    if (S.view === v) render();
    toast(`removed from ${v.title}`);
  } catch (e) { toast(`couldn't remove it: ${e}`); }
}

async function addToQueue(t) {
  try {
    await invoke('add_to_queue', { uri: t.uri });
    cacheTracks([t]);
    toast('added to queue ♪');
  } catch (e) { handleRemoteError(e); }
}

/* sleep timer: pause after some minutes, or when this song ends */
const sleep = { timer: 0, until: 0, endOfTrack: null };
function sleepLabel() {
  if (sleep.endOfTrack) return 'end of song';
  const left = Math.max(0, Math.round((sleep.until - Date.now()) / 60000));
  return left ? `${left} min left` : 'less than a minute';
}
function clearSleep() {
  clearTimeout(sleep.timer);
  Object.assign(sleep, { timer: 0, until: 0, endOfTrack: null });
}
function setSleep(min) {
  clearSleep();
  if (min === 'end') {
    sleep.endOfTrack = current()?.uri || null;
    toast('music stops when this song ends ☾');
    return;
  }
  sleep.until = Date.now() + min * 60000;
  sleep.timer = setTimeout(() => { clearSleep(); if (S.playing) control('pause'); toast('sleep timer: good night ☾'); }, min * 60000);
  toast(`music stops in ${min >= 60 ? `${min / 60} hour` : `${min} minutes`} ☾`);
}
function sleepItems() {
  const items = [5, 10, 15, 30, 45, 60].map((m) => ({ label: m === 60 ? '1 hour' : `${m} minutes`, run: () => setSleep(m) }));
  items.push({ label: 'End of song', run: () => setSleep('end') });
  if (sleep.until || sleep.endOfTrack) items.push('-', { label: 'Turn off timer', run: () => { clearSleep(); toast('sleep timer off'); } });
  return items;
}
// "end of song": pause as soon as the next one starts
function checkSleepOnTrackChange(uri) {
  if (sleep.endOfTrack && uri !== sleep.endOfTrack) {
    clearSleep();
    control('pause');
    toast('sleep timer: good night ☾');
  }
}

main.addEventListener('contextmenu', (e) => {
  const row = e.target.closest('.trow[data-index]');
  if (!row) return;
  e.preventDefault();
  openTrackMenu(viewTracks()[+row.dataset.index], { x: e.clientX, y: e.clientY }, { index: +row.dataset.index });
});

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

bindBar($('vol-bar'), (r) => setVolume(r * 100, { live: true }), (r) => setVolume(r * 100));

/* ───────────── events ───────────── */

// clicks inside the main panel (event delegation)
main.addEventListener('click', (e) => {
  const el = e.target.closest('[data-artist],[data-album],[data-like],[data-more],[data-play],[data-card],[data-tab],[data-filter],[data-open],[data-playlist],[data-index],[data-line],#hero-play,#hero-edit,#list-play,#list-shuffle,#list-more,#list-sort,#album-save,#artist-follow,#queue-clear,#create-playlist,.list-find');
  if (!el) return;
  if (el.classList.contains('list-find')) { el.classList.add('open'); el.querySelector('input').focus(); return; }
  if (el.dataset.artist) return openArtist(el.dataset.artist);
  if (el.dataset.album) return openAlbum(el.dataset.album);
  if (el.dataset.play !== undefined) return playRow(+el.dataset.play);
  if (el.dataset.card) return openCard(el.dataset.card);
  if (el.dataset.tab) { S.view.tab = el.dataset.tab; return render(); }
  if (el.id === 'list-more') return openListMenu(el);
  if (el.id === 'list-sort') return openSortMenu(el);
  if (el.id === 'album-save') return toggleFollow(S.view.uri);
  if (el.id === 'artist-follow') return toggleFollow(S.view.uri);
  if (el.id === 'queue-clear') return clearQueue();
  if (el.id === 'create-playlist') return createPlaylist();
  if (el.dataset.like !== undefined) return toggleSaved(viewTracks()[+el.dataset.like]);
  if (el.dataset.more !== undefined) return openTrackMenu(viewTracks()[+el.dataset.more], el, { index: +el.dataset.more });
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
  // like Spotify: one click selects a song, a double click (or its ▶) plays it
  if (el.dataset.index !== undefined) return selectRow(+el.dataset.index);
  if (el.id === 'hero-edit') return openCustomize();
  if (el.id === 'hero-play') return playFeatured();
  if (el.id === 'list-play' || el.id === 'list-shuffle') {
    const tracks = viewTracks();
    if (!tracks?.length) return;
    const shuffle = el.id === 'list-shuffle';
    if (shuffle !== S.shuffle) setOption('shuffle', shuffle);
    const start = shuffle ? Math.floor(Math.random() * tracks.length) : 0;
    return playRow(start);
  }
});
main.addEventListener('dblclick', (e) => {
  const row = e.target.closest('.trow[data-index]');
  if (row && !e.target.closest('button,.alink')) playRow(+row.dataset.index);
});
main.addEventListener('input', (e) => {
  if (e.target.id !== 'list-find') return;
  S.view.filterText = e.target.value;
  refreshRows();
});
main.addEventListener('keydown', (e) => {
  if (e.target.id === 'list-find' && e.key === 'Escape') { S.view.filterText = ''; render(); }
});

function openPlaylistById(id) {
  const p = S.playlists.find((x) => x.id === id) || S.homeItems?.get(id);
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
  else if (n === 'search') showMenu([
    { icon: 'create', label: 'Create playlist', run: createPlaylist },
    { icon: 'search', label: 'Search', run: () => $('search').focus() },
  ], { x: b.getBoundingClientRect().right + 8, y: b.getBoundingClientRect().top });
  else if (n === 'liked') openLiked();
  else if (n === 'queue') openQueue();
  else if (n === 'recent') openRecent();
  else if (n === 'customize') openCustomize();
  else if (n === 'logout') doLogout();
}));

$('btn-theme').onclick = () => {
  const c = themeChoice();
  showMenu([
    { icon: 'sun', label: 'Light', on: c === 'light', run: () => setTheme('light') },
    { icon: 'moon', label: 'Dark', on: c === 'dark', run: () => setTheme('dark') },
    { icon: 'devices', label: 'Same as Windows', on: c === 'auto', run: () => setTheme('auto') },
  ], $('btn-theme'));
};
$('btn-home').onclick = () => go({ name: 'home' });
$('btn-browse').onclick = () => go({ name: 'library' });
$('btn-back').onclick = goBack;
$('btn-forward').onclick = goForward;
$('btn-menu').onclick = async () => {
  const v = DEMO ? 'demo' : await TAURI.app.getVersion().catch(() => '?');
  const auto = await invoke('autostart_get').catch(() => false);
  showMenu([
    { icon: 'sparkle', label: `femboy spotify v${v} ♡`, disabled: true },
    '-',
    { icon: 'devices', label: 'Start with Windows', on: auto, run: async () => {
      try {
        await invoke('autostart_set', { on: !auto });
        toast(!auto ? 'starts with Windows now ♡' : "won't start with Windows");
      } catch (e) { toast(`couldn't change it: ${e}`); }
    } },
    { icon: 'bell', label: 'Check for updates', run: async () => { await checkForUpdate(); if (!S.update) toast('you have the newest version ♡'); } },
  ], { x: $('btn-menu').getBoundingClientRect().left, y: $('btn-menu').getBoundingClientRect().bottom + 6 });
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
  else { S.muted = true; sendVolume(0); drawVolume(0); renderPlayState(); }
};
$('right-add').onclick = saveCurrent;
$('np-add').onclick = saveCurrent;
// now playing: artist names open the artist, the song name opens its album,
// right click gives the same song menu as in lists
for (const id of ['np-artist', 'right-artist']) {
  $(id).addEventListener('click', (e) => { const a = e.target.closest('[data-artist]'); if (a) openArtist(a.dataset.artist); });
}
for (const id of ['np-name', 'right-name']) $(id).addEventListener('click', () => openAlbum(current()?.album_uri));
for (const sel of ['.np', '.right-meta']) {
  document.querySelector(sel).addEventListener('contextmenu', (e) => {
    e.preventDefault();
    openTrackMenu(current(), { x: e.clientX, y: e.clientY });
  });
}
$('btn-lyrics').onclick = () => (S.view.name === 'lyrics' ? goBack() : go({ name: 'lyrics' }));
$('lyrics-open').onclick = () => go({ name: 'lyrics' });
$('btn-queue').onclick = () => (S.view.name === 'queue' ? goBack() : openQueue());
$('btn-devices').onclick = () => toggleDevices();
// the now-playing side panel (Spotify's "Now playing view" button)
$('btn-np').onclick = () => {
  const hidden = $('app').classList.toggle('no-right');
  $('btn-np').classList.toggle('active', !hidden);
};
// the headphones buddy is the DJ X button: start DJ, or let the DJ switch it up
$('btn-panel').onclick = () => {
  if (!S.djOn) return playDJ();
  if (!S.playing) return togglePlay();
  toast('DJ X is switching up the vibe ♪');
  invoke('dj_switch').catch(handleRemoteError);
};
$('btn-mini').onclick = () => $('btn-np').click();
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
  btn.textContent = 'waiting for Spotify...';
  $('login-error').classList.add('hidden');
  $('login-text').textContent = 'sign in in the Spotify window that just opened ♡';
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

const afterLogin = () => { checkPlayer(); loadFollows(); return loadLibrary(); };

async function loadLibrary() {
  invoke('me').then((me) => {
    S.me = me;
    if (me.avatar) $('avatar-img').src = me.avatar;
    else $('avatar-img').src = NO_ARTIST;
  }).catch(() => {});
  try {
    S.playlists = await invoke('playlists');
  } catch (e) { toast(`couldn't load playlists: ${e}`); }
  renderSideThumbs();
  if (S.view.name === 'home') render();
  invoke('liked').then((l) => { S.liked = l; cacheTracks(l); l.forEach((t) => S.saved.add(t.id)); }).catch(() => {});
  invoke('remote_state').then(applyRemote).catch(() => {});
  invoke('spotify_home').then((h) => h && setHome(h)).catch(() => {});
}

listen('remote', applyRemote);

/* ───────────── pink tooltips ─────────────
   The browser's own `title` tooltip is a black system box. Every `title` (in the
   HTML or set later by code) moves to data-tip + aria-label, and we draw our own. */

const tipEl = document.createElement('div');
tipEl.className = 'tip';
tipEl.setAttribute('role', 'tooltip');
document.body.append(tipEl);
let tipFor = null;
let tipTimer = 0;

function adoptTitle(el) {
  const t = el.getAttribute('title');
  if (t == null) return;
  el.removeAttribute('title');
  el.dataset.tip = t;
  if (t) el.setAttribute('aria-label', t);
  if (tipFor === el) showTip(el);
}
function adoptTitles(root) {
  if (root.nodeType !== 1) return;
  if (root.hasAttribute('title')) adoptTitle(root);
  root.querySelectorAll('[title]').forEach(adoptTitle);
}
new MutationObserver((muts) => {
  for (const m of muts) {
    if (m.type === 'attributes') adoptTitle(m.target);
    else m.addedNodes.forEach(adoptTitles);
  }
}).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['title'] });

function showTip(el) {
  const text = el.dataset.tip;
  if (!text || !el.isConnected) return hideTip();
  tipEl.textContent = text;
  const r = el.getBoundingClientRect();
  const w = tipEl.offsetWidth;
  const h = tipEl.offsetHeight;
  const below = r.top - h - 10 < 4;
  const x = Math.min(Math.max(6, r.left + r.width / 2 - w / 2), innerWidth - w - 6);
  const y = below ? r.bottom + 10 : r.top - h - 10;
  tipEl.style.left = `${Math.round(x)}px`;
  tipEl.style.top = `${Math.round(y)}px`;
  tipEl.classList.toggle('below', below);
  tipEl.classList.add('show');
}
function hideTip() {
  clearTimeout(tipTimer);
  tipFor = null;
  tipEl.classList.remove('show');
}
document.addEventListener('mouseover', (e) => {
  const el = e.target.closest?.('[data-tip]');
  if (el === tipFor) return;
  hideTip();
  if (!el) return;
  tipFor = el;
  tipTimer = setTimeout(() => showTip(el), 380);
});
document.addEventListener('mouseout', (e) => {
  if (tipFor && !tipFor.contains(e.relatedTarget)) hideTip();
});
['mousedown', 'wheel', 'keydown'].forEach((ev) => document.addEventListener(ev, hideTip, { passive: true }));
window.addEventListener('blur', hideTip);

async function start() {
  adoptTitles(document.body);
  hydrateIcons();
  applyTheme();
  $('avatar-img').src = NO_ARTIST;
  drawVolume(S.volume);
  renderPlayState();
  render();
  renderNow();
  setInterval(tick, 250);
  checkForUpdate();
  setInterval(checkForUpdate, 6 * 60 * 60 * 1000);
  await connectSaved();
}

// Log in with the saved login. If Spotify can't be reached yet (e.g. right after
// Windows starts, before the network is up) keep trying instead of asking to sign
// in again: the saved login is fine.
async function connectSaved(attempt = 0) {
  try {
    const ok = await invoke('init');
    if (ok) { if (attempt) toast('connected to Spotify ♡'); await afterLogin(); }
    else $('login').classList.remove('hidden');
  } catch (e) {
    if (attempt === 0) toast('waiting for the internet to reach Spotify...');
    setTimeout(() => connectSaved(attempt + 1), Math.min(30000, 5000 * (attempt + 1)));
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
  .map(([name, artists, d], i) => ({ id: 't' + i, uri: 'spotify:track:t' + i, name, artists, artist_list: artists.split(', ').map((a) => ({ name: a, uri: 'spotify:artist:' + a.replace(/\W/g, '') })), album: 'cute album vol. ' + (i % 3 + 1), album_uri: 'spotify:album:a' + (i % 3 + 1), duration_ms: d, image: demoCover(i + 2, ''), image_large: demoCover(i + 2, '') }));
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
    case 'remote_state': case 'refresh_devices': return demoState();
    case 'artist_page': return { uri: args.uri, name: 'pastel kid', image: demoCover(5, ''), top: DEMO_TRACKS.slice(0, 5), albums: [1, 2].map((n) => ({ uri: 'spotify:album:a' + n, name: `cute album vol. ${n}`, year: 2026 - n, image: demoCover(n, '') })), singles: [{ uri: 'spotify:album:s1', name: 'strawberry milk', year: 2026, image: demoCover(3, '') }] };
    case 'album_page': return { uri: args.uri, name: 'cute album vol. 1', artists: [{ name: 'pastel kid', uri: 'spotify:artist:demo' }], year: 2025, image: demoCover(1, ''), tracks: DEMO_TRACKS.slice(0, 4) };
    case 'song_radio': return DEMO_TRACKS.slice().reverse();
    case 'set_saved': case 'playlist_add': case 'playlist_remove': case 'add_to_queue': case 'set_followed':
    case 'playlist_rename': case 'playlist_unfollow': case 'clear_queue': case 'dj_switch': return null;
    case 'local_spotify': return { name: 'DEMO-PC', running: false };
    case 'autostart_get': return true;
    case 'autostart_set': return null;
    case 'playlist_create': return 'pl0';
    case 'collection_set': return [];
    case 'tracks_info': return DEMO_TRACKS.filter((t) => args.uris.includes(t.uri));
    case 'recent_spotify': return DEMO_PLAYLISTS.slice(0, 6).map((p) => ({ uri: 'spotify:playlist:' + p.id, kind: 'playlist', name: p.name, sub: 'playlist', image: p.image }));
    case 'search_all': {
      const tracks = DEMO_TRACKS.filter((t) => (t.name + t.artists).toLowerCase().includes(args.q.toLowerCase()));
      return { tracks, artists: [{ uri: 'spotify:artist:pastelkid', kind: 'artist', name: 'pastel kid', sub: 'artist', image: demoCover(5, '') }], albums: [{ uri: 'spotify:album:a1', kind: 'album', name: 'cute album vol. 1', sub: 'pastel kid', image: demoCover(1, '') }] };
    }
    case 'spotify_home': return [
      { title: 'Made For cutie', items: ['Daily Mix 1', 'Daily Mix 2', 'Release Radar', 'Discover Weekly', 'Daily Mix 3'].map((n, i) => ({ id: 'mix' + i, name: n, sub: 'cute songs, mixed for you', image: demoCover(i + 3, n) })) },
      { title: 'Your top mixes', items: ['Hyperpop Mix', 'Pastel Mix', 'Night Mix'].map((n, i) => ({ id: 'top' + i, name: n, sub: 'Spotify', image: demoCover(i + 1, n) })) },
    ];
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

// demo page with "?shot": a song is already playing (for the README screenshot)
if (DEMO && location.search.includes('shot')) setTimeout(() => playFrom(DEMO_TRACKS, 1, 'Boykisser Mix', null), 400);
