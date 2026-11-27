// Hand-drawn little icon set ♡ (24×24, stroke = currentColor)
const ICONS = {
  dots: '<circle cx="5" cy="12" r="1.8" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1.8" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1.8" fill="currentColor" stroke="none"/>',
  'chevron-left': '<path d="M15 4.5 7.5 12l7.5 7.5"/>',
  'chevron-right': '<path d="M9 4.5 16.5 12 9 19.5"/>',
  home: '<path fill="currentColor" stroke="none" d="M2.6 11.2 11.2 3.9a1.3 1.3 0 0 1 1.6 0l8.6 7.3c.5.4.2 1.3-.5 1.3H20v7a1.5 1.5 0 0 1-1.5 1.5h-4v-6h-5v6h-4A1.5 1.5 0 0 1 4 19.5v-7h-.9c-.7 0-1-.9-.5-1.3z"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/>',
  browse: '<rect x="3" y="4" width="18" height="5.5" rx="1.5"/><path d="M4.5 9.5v8.5a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V9.5M10 13.5h4"/>',
  bell: '<path d="M6 16.5V11a6 6 0 1 1 12 0v5.5l1.8 1.8H4.2z"/><path d="M10 20.5a2 2 0 0 0 4 0"/>',
  users: '<circle cx="9" cy="8" r="3.4"/><path d="M2.8 20c.4-3.6 3-5.6 6.2-5.6s5.8 2 6.2 5.6"/><circle cx="17" cy="8.8" r="2.6"/><path d="M17.2 14.3c2.4.3 3.9 2 4.2 4.6"/>',
  minimize: '<path d="M5 12.5h14"/>',
  maximize: '<rect x="5.5" y="5.5" width="13" height="13" rx="1.5"/>',
  close: '<path d="M6 6l12 12M18 6 6 18"/>',
  library: '<path d="M5 4v16M10 4v16M14.2 4.6 19 19.4"/>',
  'plus-circle': '<circle cx="12" cy="12" r="9"/><path d="M12 7.5v9M7.5 12h9"/>',
  'check-circle': '<circle cx="12" cy="12" r="9" fill="currentColor"/><path d="m7.8 12.3 2.8 2.8 5.6-5.8" stroke="#fff"/>',
  heart: '<path d="M12 20.5s-7.6-4.6-9.5-9.3C1.1 7.7 3.3 4 7 4c2.2 0 3.9 1.3 5 3 1.1-1.7 2.8-3 5-3 3.7 0 5.9 3.7 4.5 7.2-1.9 4.7-9.5 9.3-9.5 9.3z"/>',
  bookmark: '<path d="M6.5 3.5h11v17L12 16.6l-5.5 3.9z"/>',
  pie: '<circle cx="12" cy="12" r="9"/><path d="M12 3v9h9"/>',
  cat: '<path d="M4.2 9.4 5 3.2l4.4 3.4h5.2L19 3.2l.8 6.2c1.1 1.3 1.7 2.8 1.7 4.4 0 4.3-4.3 7-9.5 7s-9.5-2.7-9.5-7c0-1.6.6-3.1 1.7-4.4z"/><circle cx="8.8" cy="12.6" r=".9" fill="currentColor"/><circle cx="15.2" cy="12.6" r=".9" fill="currentColor"/><path d="M10.6 15.6c.5.5.9.6 1.4.2.5.4.9.3 1.4-.2"/>',
  ribbon: '<path fill="currentColor" d="M12 11.5 3.5 6.5v10zM12 11.5l8.5-5v10z"/><circle cx="12" cy="11.5" r="2.3" fill="currentColor"/><path d="m11 13.5-3 7.5M13 13.5l3 7.5"/>',
  shuffle: '<path d="M3 6.5h3.5c4.5 0 6.5 11 11 11H21M3 17.5h3.5c1.6 0 2.8-1.4 3.8-3.2M13.7 9.7c1-1.8 2.2-3.2 3.8-3.2H21M18 3.5l3 3-3 3M18 14.5l3 3-3 3"/>',
  prev: '<path fill="currentColor" d="M6 5h2.6v14H6zM19.5 5.6v12.8a.8.8 0 0 1-1.2.7L9.4 12.7a.8.8 0 0 1 0-1.4l8.9-6.4a.8.8 0 0 1 1.2.7z"/>',
  next: '<path fill="currentColor" d="M15.4 5H18v14h-2.6zM4.5 5.6v12.8a.8.8 0 0 0 1.2.7l8.9-6.4a.8.8 0 0 0 0-1.4L5.7 4.9a.8.8 0 0 0-1.2.7z"/>',
  // optically centred: the box sits 1px right of centre (a triangle looks left-heavy)
  play: '<path fill="currentColor" d="M6.7 5.3v13.4a1 1 0 0 0 1.5.9l10.7-6.7a1 1 0 0 0 0-1.8L8.2 4.4A1 1 0 0 0 6.7 5.3z"/>',
  pause: '<rect x="6.5" y="5" width="3.8" height="14" rx="1.2" fill="currentColor"/><rect x="13.7" y="5" width="3.8" height="14" rx="1.2" fill="currentColor"/>',
  repeat: '<path d="M4 11.5v-1.5a3.5 3.5 0 0 1 3.5-3.5H19M16 3.5l3 3-3 3M20 12.5V14a3.5 3.5 0 0 1-3.5 3.5H5M8 20.5l-3-3 3-3"/>',
  'repeat-one': '<path d="M4 11.5v-1.5a3.5 3.5 0 0 1 3.5-3.5H19M16 3.5l3 3-3 3M20 12.5V14a3.5 3.5 0 0 1-3.5 3.5H5M8 20.5l-3-3 3-3"/><path d="M11.3 10.3l1.2-.8v5" stroke-width="1.8"/>',
  disc: '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="2.6"/>',
  pen: '<path d="M14.5 4.5l5 5L9 20H4v-5z"/><path d="M12.5 6.5l5 5"/>',
  // now playing view: a screen with a play triangle, like Spotify's
  np: '<rect x="3" y="4" width="18" height="16" rx="3"/><path fill="currentColor" stroke="none" d="M9.6 8.7v6.6a.7.7 0 0 0 1.1.6l5-3.3a.7.7 0 0 0 0-1.2l-5-3.3a.7.7 0 0 0-1.1.6z"/>',
  // lyrics: a tilted microphone like Spotify's
  mic: '<circle cx="15" cy="9" r="5.5"/><path d="M11.3 5.3l7.4 7.4"/><path fill="currentColor" stroke="none" d="M10.4 11.2l2.4 2.4-7.4 7a1.6 1.6 0 0 1-2.3-2.3z"/>',
  queue: '<path d="M4 6.5h16M4 12h16M4 17.5h10"/>',
  devices: '<rect x="3" y="4" width="11" height="16" rx="2"/><rect x="16.5" y="8" width="4.5" height="12" rx="1.2"/><path d="M7.5 16.5h2"/>',
  volume: '<path d="M4 9.5v5h3.5l5 4v-13l-5 4z" fill="currentColor"/><path d="M16 8.8a4.6 4.6 0 0 1 0 6.4M18.7 6.2a8.3 8.3 0 0 1 0 11.6"/>',
  'volume-off': '<path d="M4 9.5v5h3.5l5 4v-13l-5 4z" fill="currentColor"/><path d="m16 9.5 5 5M21 9.5l-5 5"/>',
  mini: '<rect x="3" y="5" width="18" height="14" rx="2"/><rect x="12" y="11.5" width="6.5" height="5" rx="1" fill="currentColor"/>',
  fullscreen: '<path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/>',
  sparkle: '<path fill="currentColor" stroke="none" d="M12 1.5c.6 5.2 2.3 7.6 9.5 10.5-7.2 2.9-8.9 5.3-9.5 10.5-.6-5.2-2.3-7.6-9.5-10.5C9.7 9.1 11.4 6.7 12 1.5z"/>',
};

function icon(name, cls = '') {
  return `<svg class="ico ${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`;
}

function hydrateIcons(root = document) {
  root.querySelectorAll('[data-icon]').forEach((el) => {
    if (!el.querySelector(':scope > svg.ico')) el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon));
  });
}

function setIcon(el, name) {
  el.dataset.icon = name;
  const svg = el.querySelector(':scope > svg.ico');
  if (svg) svg.outerHTML = icon(name);
  else el.insertAdjacentHTML('afterbegin', icon(name));
}
