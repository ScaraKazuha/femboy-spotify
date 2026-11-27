//! The main panel's pages as HTML, built from the state. Pure: no DOM, no state
//! changes (except remembering what the rows show in `view.shown`).

use crate::icons::{icon, icon_cls};
use crate::state::{State, View};
use crate::types::*;
use crate::util::{ago, esc, fmt, fmt_long, store_get};

pub const PLACEHOLDER: &str = "art/icons/no-cover.png";
pub const NO_ARTIST: &str = "art/icons/no-artist.png";
pub const DJ_URI: &str = "spotify:playlist:37i9dQZF1EYkqdzj48dyYq";
pub const DJ_ART: &str = "art/headphones.png";

pub fn img(url: &Option<String>) -> String {
    esc(url.as_deref().filter(|u| !u.is_empty()).unwrap_or(PLACEHOLDER))
}

pub fn loading_html(text: &str) -> String {
    format!(r#"<div class="empty"><img class="loading-art" src="art/icons/loading.png" alt="">{text}</div>"#)
}

pub fn loading() -> String {
    loading_html("loading... ₍ᐢ. .ᐢ₎")
}

pub fn empty_html(text: &str) -> String {
    format!(r#"<div class="empty"><img class="empty-art" src="art/icons/empty.png" alt="">{text}</div>"#)
}

pub fn section_head(title: &str) -> String {
    format!(r#"<div class="section-head"><h2>{}</h2><div class="line">{}</div></div>"#, esc(title), icon("heart"))
}

/// every artist name links to their page (older saved history has names only)
pub fn artist_links(t: &Track) -> String {
    if t.artist_list.is_empty() {
        return esc(&t.artists);
    }
    t.artist_list
        .iter()
        .map(|a| format!(r#"<span class="alink" data-artist="{}">{}</span>"#, esc(&a.uri), esc(&a.name)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn like_button(s: &State, t: &Track, i: usize) -> String {
    let saved = s.saved.contains(&t.id);
    format!(
        r#"<button class="tlike {}" data-like="{i}" title="{}"><img class="ico art" src="art/icons/{}.png" alt="" draggable="false"></button>"#,
        if saved { "on" } else { "" },
        if saved { "remove from Liked Songs" } else { "save to Liked Songs" },
        if saved { "liked" } else { "add" },
    )
}

pub struct RowOpts {
    pub album: bool,
    pub dates: bool,
    pub offset: usize,
    pub head: bool,
    /// long lists draw only the rows near the screen: (first, end, row height);
    /// the rest is empty space of the same height, so scrolling looks the same
    pub window: Option<(usize, usize, f64)>,
    pub selected: Option<usize>,
}

impl Default for RowOpts {
    fn default() -> Self {
        RowOpts { album: true, dates: false, offset: 0, head: true, window: None, selected: None }
    }
}

/// Lists longer than this draw only the rows around what's on screen.
pub const WINDOW_FROM: usize = 100;
/// rows drawn when a long list opens (more than a tall screen shows)
pub const WINDOW_FIRST: usize = 40;
/// a row's height until the real one is measured
pub const ROW_H: f64 = 60.0;

pub fn row_opts(v: &View, len: usize) -> RowOpts {
    let window = (len > WINDOW_FROM).then(|| {
        let (a, b) = if v.win.1 == 0 { (0, WINDOW_FIRST) } else { v.win };
        let b = b.min(len);
        (a.min(b), b, if v.row_h > 0.0 { v.row_h } else { ROW_H })
    });
    RowOpts {
        album: v.kind != "album",
        dates: v.kind == "liked" || v.kind == "playlist",
        window,
        selected: v.selected,
        ..Default::default()
    }
}

fn pad(class: &str, rows: usize, h: f64) -> String {
    format!(r#"<div class="trow-pad {class}" style="height:{}px"></div>"#, rows as f64 * h)
}

/// Song rows like Spotify's: ▶ on hover, liked mark, time, "•••".
/// `offset` numbers the rows when one list is split into sections (the queue).
pub fn tracks_html(s: &State, tracks: &[Track], o: &RowOpts) -> String {
    let cur = s.now.as_ref().map(|t| t.uri.clone());
    let cls = format!("{} {}", if o.album { "" } else { "no-album" }, if o.dates { "has-date" } else { "" });
    let mut html = format!(r#"<div class="tracks {cls}">"#);
    if o.head {
        html.push_str(&format!(
            r#"<div class="trow head"><div class="tnum">#</div><div>title</div>{}{}<div></div><div class="tdur">{}</div><div></div></div>"#,
            if o.album { "<div>album</div>" } else { "" },
            if o.dates { "<div>date added</div>" } else { "" },
            icon_cls("clock", "head-clock"),
        ));
    }
    let (first, end) = match o.window {
        Some((a, b, h)) => {
            html.push_str(&pad("top", a, h));
            (a, b)
        }
        None => (0, tracks.len()),
    };
    for (n, t) in tracks.iter().enumerate().take(end).skip(first) {
        let i = n + o.offset;
        let playing = cur.as_deref() == Some(t.uri.as_str());
        let album_cell = match &t.album_uri {
            Some(u) => format!(r#"<span class="alink" data-album="{}">{}</span>"#, esc(u), esc(&t.album)),
            None => esc(&t.album),
        };
        html.push_str(&format!(
            r#"<div class="trow {playing_cls}{selected_cls}" data-index="{i}" data-n="{num}">
        <div class="tnum"><span class="n">{numtxt}</span><button class="tplay" data-play="{i}" title="play {name}">{play}</button></div>
        <div class="tmain"><img src="{img}" alt="" loading="lazy"><div class="ttext"><div class="tname">{name}</div><div class="tartist">{artists}</div></div></div>
        {album}{date}{like}
        <div class="tdur">{dur}</div>
        <button class="tmore" data-more="{i}" title="more options for {name}">{dots}</button>
      </div>"#,
            playing_cls = if playing { "playing" } else { "" },
            selected_cls = if o.selected == Some(i) { " selected" } else { "" },
            num = n + 1,
            numtxt = if playing { "♪".to_string() } else { (n + 1).to_string() },
            name = esc(&t.name),
            play = icon("play"),
            img = img(&t.image),
            artists = artist_links(t),
            album = if o.album { format!(r#"<div class="talbum">{album_cell}</div>"#) } else { String::new() },
            date = if o.dates { format!(r#"<div class="tdate">{}</div>"#, ago(t.added_at)) } else { String::new() },
            like = like_button(s, t, i),
            dur = fmt(t.duration_ms),
            dots = icon("dots"),
        ));
    }
    if let Some((_, b, h)) = o.window {
        html.push_str(&pad("bottom", tracks.len() - b, h));
    }
    html.push_str("</div>");
    html
}

// ---------- playlist search + sort ----------

pub const SORTS: [(&str, &str); 6] = [
    ("custom", "Custom order"),
    ("title", "Title"),
    ("artist", "Artist"),
    ("album", "Album"),
    ("added", "Date added"),
    ("duration", "Duration"),
];

pub fn sort_label(key: &str) -> &'static str {
    SORTS.iter().find(|(k, _)| *k == key).map(|(_, l)| *l).unwrap_or("Custom order")
}

fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let r = js_sys::JsString::from(a).locale_compare(b, &js_sys::Array::new(), &js_sys::Object::new());
    r.cmp(&0)
}

/// What the rows show: the list filtered by the search box and sorted.
pub fn shown_tracks(v: &View) -> Vec<Track> {
    let mut list = v.tracks.clone().unwrap_or_default();
    let q = v.filter_text.trim().to_lowercase();
    if !q.is_empty() {
        list.retain(|t| format!("{} {} {}", t.name, t.artists, t.album).to_lowercase().contains(&q));
    }
    // text sorts keep songs with no name / artist / album at the end
    let text = |a: &str, b: &str| a.is_empty().cmp(&b.is_empty()).then_with(|| locale_cmp(a, b));
    match v.sort_key.as_str() {
        "title" => list.sort_by(|a, b| text(&a.name, &b.name)),
        "artist" => list.sort_by(|a, b| text(&a.artists, &b.artists)),
        "album" => list.sort_by(|a, b| text(&a.album, &b.album)),
        "added" => list.sort_by(|a, b| b.added_at.unwrap_or(0.0).total_cmp(&a.added_at.unwrap_or(0.0))),
        "duration" => list.sort_by(|a, b| a.duration_ms.total_cmp(&b.duration_ms)),
        _ => {}
    }
    list
}

pub fn is_reordered(v: &View) -> bool {
    !v.filter_text.trim().is_empty() || (!v.sort_key.is_empty() && v.sort_key != "custom")
}

fn list_tools_html(v: &View) -> String {
    if !["liked", "playlist", "album"].contains(&v.kind.as_str()) {
        return String::new();
    }
    let sort = if v.kind != "album" {
        format!(
            r#"<button class="chip sort-btn" id="list-sort" title="sort">{} {}</button>"#,
            esc(sort_label(if v.sort_key.is_empty() { "custom" } else { &v.sort_key })),
            icon("sort")
        )
    } else {
        String::new()
    };
    format!(
        r#"<div class="list-tools">
    <label class="list-find {open}" title="search in {title}">
      {ico}<input id="list-find" type="text" placeholder="Search in {what}" value="{val}" spellcheck="false" autocomplete="off">
    </label>
    {sort}
  </div>"#,
        open = if v.filter_text.is_empty() { "" } else { "open" },
        title = esc(if v.title.is_empty() { "this list" } else { &v.title }),
        ico = icon("search-in-list"),
        what = if v.kind == "album" { "album" } else { "playlist" },
        val = esc(&v.filter_text),
    )
}

/// Only the rows (used while typing in the list's search box).
pub fn rows_html(s: &State, shown: &[Track]) -> String {
    if shown.is_empty() {
        return r#"<div class="tracks"><div class="empty small">no songs match (｡•́︿•̀｡)</div></div>"#.into();
    }
    tracks_html(s, shown, &row_opts(&s.view, shown.len()))
}

// ---------- pages ----------

pub fn featured_playlist(s: &State) -> Option<Playlist> {
    s.playlists
        .iter()
        .find(|p| p.name.to_lowercase().contains("boykisser"))
        .or_else(|| s.playlists.first())
        .cloned()
}

fn hero_html(s: &State) -> String {
    let custom: Option<String> = store_get("banner", None);
    let banner_title: String = store_get("bannerTitle", String::new());
    // default look: the Boykisser Mix artwork (title & play button are painted into the picture)
    if custom.is_none() && banner_title.is_empty() {
        let name = featured_playlist(s).map(|p| p.name).unwrap_or_else(|| "Liked Songs".into());
        return format!(
            r#"<section class="hero art-hero" style="background-image:url('art/boykisser-banner.jpg')">
      <button class="art-play" id="hero-play" title="play {}">{}</button>
      <button class="icon-btn hero-edit" id="hero-edit" title="customize banner">{}</button>
    </section>"#,
            esc(&name),
            icon("play"),
            icon("pen")
        );
    }
    let title = if banner_title.is_empty() {
        s.playlists.first().map(|p| p.name.clone()).unwrap_or_else(|| "Liked Songs".into())
    } else {
        banner_title.clone()
    };
    let pl = s.playlists.first();
    let art = match &custom {
        Some(c) => format!(r#"<div class="hero-art" style="background-image:url('{}')"></div>"#, esc(c)),
        None => {
            let covers: Vec<&Playlist> = s.playlists.iter().filter(|p| p.image.is_some()).take(2).collect();
            let small = covers
                .get(1)
                .map(|p| format!(r#"<div class="sticker small" style="background-image:url('{}')"></div>"#, img(&p.image)))
                .unwrap_or_default();
            let big_img = covers.first().map(|p| img(&p.image)).unwrap_or_else(|| PLACEHOLDER.into());
            format!(r#"<div class="hero-stickers">{small}<div class="sticker big" style="background-image:url('{big_img}')"></div></div>"#)
        }
    };
    let deco: String = [
        ('s', 3, 10, 22), ('s', 30, 6, 16), ('s', 58, 70, 28), ('s', 88, 12, 20), ('s', 46, 22, 14), ('s', 93, 70, 26),
        ('h', 2, 52, 30), ('h', 52, 14, 40), ('h', 40, 78, 26), ('h', 85, 36, 30),
    ]
    .iter()
    .enumerate()
    .map(|(i, (k, x, y, sz))| {
        format!(
            r#"<span class="{k}" style="left:{x}%;top:{y}%;width:{sz}px;height:{sz}px;animation-delay:{:.2}s">{}</span>"#,
            i as f64 * 0.37,
            icon(if *k == 's' { "sparkle" } else { "heart" })
        )
    })
    .collect();
    let sub = match pl {
        Some(p) if banner_title.is_empty() => format!("{} songs", p.total),
        _ => String::new(),
    };
    format!(
        r#"<section class="hero">
    {art}
    <div class="hero-deco">{deco}</div>
    <div class="hero-content">
      <h1 class="hero-title">{}</h1>
      <div class="hero-row">
        <button class="hero-play" id="hero-play" title="play">{}</button>
        <span class="hero-sub">{sub}</span>
      </div>
    </div>
    <button class="icon-btn hero-edit" id="hero-edit" title="customize banner">{}</button>
  </section>"#,
        esc(&title),
        icon("play"),
        icon("pen")
    )
}

fn card_html(p: &Playlist) -> String {
    format!(
        r#"<button class="card-item" data-playlist="{}">
      <img class="cov" src="{}" alt="" loading="lazy">
      <div class="t">{}</div>
      <div class="st">{} · {} songs</div>
    </button>"#,
        esc(&p.id),
        img(&p.image),
        esc(&p.name),
        esc(if p.owner.is_empty() { "playlist" } else { &p.owner }),
        p.total
    )
}

fn home_card_html(p: &ShelfItem) -> String {
    let dj = DJ_URI.ends_with(&format!(":{}", p.id));
    format!(
        r#"<button class="card-item" {attr}>
      <img class="cov" src="{img}" alt="" loading="lazy">
      <div class="t">{name}</div>
      <div class="st">{sub}</div>
    </button>"#,
        attr = if dj { r#"data-open="dj""#.to_string() } else { format!(r#"data-playlist="{}""#, esc(&p.id)) },
        img = if p.image.is_empty() { PLACEHOLDER.to_string() } else { esc(&p.image) },
        name = esc(&p.name),
        sub = esc(if p.sub.is_empty() { "Spotify" } else { &p.sub }),
    )
}

fn home_html(s: &State) -> String {
    let chips: String = [("all", "All"), ("playlists", "Playlists"), ("liked", "Liked"), ("recent", "Recent")]
        .iter()
        .map(|(k, l)| format!(r#"<button class="chip {}" data-filter="{k}">{l}</button>"#, if s.filter == *k { "active" } else { "" }))
        .collect();
    let mut tiles = vec![
        r#"<button class="tile" data-open="liked"><img class="tile-art liked-cover" src="art/icons/liked-cover.png" alt=""><span>Liked Songs</span></button>"#.to_string(),
        r#"<button class="tile" data-open="dj" title="Spotify DJ"><img class="dj-art" src="art/headphones.png" alt=""><span>DJ X</span></button>"#.to_string(),
    ];
    tiles.extend(s.playlists.iter().take(6).map(|p| {
        format!(r#"<button class="tile" data-playlist="{}"><img src="{}" alt=""><span>{}</span></button>"#, esc(&p.id), img(&p.image), esc(&p.name))
    }));
    let cards: String = if s.filter == "playlists" {
        s.playlists.iter().map(card_html).collect()
    } else {
        s.playlists.iter().skip(6).map(card_html).collect()
    };
    // Spotify's own shelves: Daily Mixes, Release Radar, top mixes... (from the built-in player)
    let shelves: String = if s.filter == "all" {
        s.home
            .iter()
            .map(|sh| format!(r#"{}<div class="cards shelf">{}</div>"#, section_head(&sh.title), sh.items.iter().map(home_card_html).collect::<String>()))
            .collect()
    } else {
        String::new()
    };
    format!(
        r#"{hero}
    <div class="chips">{chips}</div>
    {tiles}
    {shelves}
    {cards}
    {loading}"#,
        hero = hero_html(s),
        tiles = if s.filter == "all" { format!(r#"<div class="tiles">{}</div>"#, tiles.join("")) } else { String::new() },
        cards = if cards.is_empty() { String::new() } else { format!(r#"{}<div class="cards">{cards}</div>"#, section_head("Your playlists")) },
        loading = if s.playlists.is_empty() { loading_html("loading your library...") } else { String::new() },
    )
}

fn library_html(s: &State) -> String {
    format!(
        r#"<div class="list-head"><div>
      <div class="list-kind">library</div>
      <h1 class="list-title">Your playlists</h1>
      <div class="list-meta">{} playlists</div>
    </div></div>
    <div class="cards">
      <button class="card-item" id="create-playlist"><img class="cov" src="art/icons/create.png" alt=""><div class="t">Create playlist</div><div class="st">a new one, just yours</div></button>
      <button class="card-item" data-open="liked"><img class="cov liked-cover" src="art/icons/liked-cover.png" alt=""><div class="t">Liked Songs</div><div class="st">♡</div></button>
      <button class="card-item" data-open="dj"><img class="cov dj-art" src="art/headphones.png" alt=""><div class="t">DJ X</div><div class="st">Spotify DJ ♪</div></button>
      {}
    </div>"#,
        s.playlists.len(),
        s.playlists.iter().map(card_html).collect::<String>()
    )
}

fn list_html(s: &State, v: &View) -> String {
    let cover = if v.kind == "liked" {
        r#"<img class="list-cover liked-cover" src="art/icons/liked-cover.png" alt="">"#.to_string()
    } else {
        let im = v.image.clone().or_else(|| v.tracks.as_ref().and_then(|t| t.first()).and_then(|t| t.image.clone()));
        format!(r#"<img class="list-cover" src="{}" alt="">"#, img(&im))
    };
    let kind_label = match v.kind.as_str() {
        "liked" | "playlist" => "playlist",
        "search" => "search",
        "recent" => "history",
        "queue" => "queue",
        "album" => "album",
        "radio" => "song radio",
        _ => "",
    };
    let tracks = v.tracks.clone().unwrap_or_default();
    let body = if v.loading {
        loading()
    } else if tracks.is_empty() {
        empty_html("nothing here yet~")
    } else {
        let shown = v.shown.clone().unwrap_or_default();
        if shown.is_empty() {
            r#"<div class="tracks"><div class="empty small">no songs match (｡•́︿•̀｡)</div></div>"#.to_string()
        } else {
            tracks_html(s, &shown, &row_opts(v, shown.len()))
        }
    };
    let total: f64 = tracks.iter().map(|t| t.duration_ms).sum();
    let album_saved = v.kind == "album" && s.saved_albums.contains(&v.uri);
    let has_menu = ["liked", "playlist", "album", "radio"].contains(&v.kind.as_str());
    let meta_tail = if tracks.is_empty() { String::new() } else { format!(" · {} songs, {}", tracks.len(), fmt_long(total)) };
    let actions = if tracks.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="list-actions">
      <button class="round-play" id="list-play" title="play">{play}</button>
      <button class="icon-btn" id="list-shuffle" title="shuffle play">{shuffle}</button>
      {save}
      {more}
      {tools}
    </div>"#,
            play = icon("play"),
            shuffle = icon("shuffle"),
            save = if v.kind == "album" {
                format!(
                    r#"<button class="icon-btn save-btn {}" id="album-save" title="{}">{}</button>"#,
                    if album_saved { "on" } else { "" },
                    if album_saved { "remove from Your Library" } else { "save to Your Library" },
                    icon(if album_saved { "album-saved" } else { "save-album" })
                )
            } else {
                String::new()
            },
            more = if has_menu {
                format!(r#"<button class="icon-btn" id="list-more" title="more options for {}">{}</button>"#, esc(&v.title), icon("dots"))
            } else {
                String::new()
            },
            tools = list_tools_html(v),
        )
    };
    format!(
        r#"<div class="list-head">{cover}<div>
      <div class="list-kind">{kind_label}</div>
      <h1 class="list-title" style="font-size:{size}px">{title}</h1>
      <div class="list-meta">{sub}{meta_tail}</div>
    </div></div>
    {actions}
    {body}"#,
        size = if v.title.chars().count() > 24 { 40 } else { 60 },
        title = esc(&v.title),
        sub = v.sub_html.clone().unwrap_or_else(|| esc(&v.sub)),
    )
}

pub fn card_html2(c: &Card) -> String {
    let artist = c.kind == "artist";
    let im = match &c.image {
        Some(i) if !i.is_empty() => esc(i),
        _ => (if artist { NO_ARTIST } else { PLACEHOLDER }).to_string(),
    };
    format!(
        r#"<button class="card-item {round}" data-card="{uri}">
      <img class="cov" src="{im}" alt="" loading="lazy">
      <div class="t">{name}</div>
      <div class="st">{sub}</div>
    </button>"#,
        round = if artist { "round" } else { "" },
        uri = esc(&c.uri),
        name = esc(&c.name),
        sub = esc(if c.sub.is_empty() { &c.kind } else { &c.sub }),
    )
}

fn search_html(s: &State, v: &View) -> String {
    let tabs: String = [("all", "All"), ("songs", "Songs"), ("artists", "Artists"), ("albums", "Albums")]
        .iter()
        .map(|(k, l)| format!(r#"<button class="chip {}" data-tab="{k}">{l}</button>"#, if v.tab == *k { "active" } else { "" }))
        .collect();
    let body = match &v.results {
        None => loading(),
        Some(_) if v.loading => loading(),
        Some(r) if r.tracks.is_empty() => empty_html(&format!("nothing found for \"{}\"", esc(&v.q))),
        Some(r) => match v.tab.as_str() {
            "songs" => tracks_html(s, &r.tracks, &RowOpts::default()),
            "artists" => format!(r#"<div class="cards">{}</div>"#, r.artists.iter().map(card_html2).collect::<String>()),
            "albums" => format!(r#"<div class="cards">{}</div>"#, r.albums.iter().map(card_html2).collect::<String>()),
            _ => {
                let top = r.artists.first().map(|t| {
                    format!(
                        r#"<div><h2 class="mini-head">Top result</h2>
          <button class="top-result" data-card="{}"><img src="{}" alt=""><div class="t">{}</div><div class="st">artist</div></button></div>"#,
                        esc(&t.uri),
                        t.image.as_deref().filter(|i| !i.is_empty()).map(esc).unwrap_or_else(|| NO_ARTIST.into()),
                        esc(&t.name)
                    )
                });
                let first4: Vec<Track> = r.tracks.iter().take(4).cloned().collect();
                format!(
                    r#"<div class="search-top">
        {}
        <div class="search-songs"><h2 class="mini-head">Songs</h2>{}</div>
      </div>
      {}
      {}"#,
                    top.unwrap_or_default(),
                    tracks_html(s, &first4, &RowOpts { album: false, head: false, ..Default::default() }),
                    if r.artists.is_empty() { String::new() } else { format!(r#"{}<div class="cards">{}</div>"#, section_head("Artists"), r.artists.iter().take(6).map(card_html2).collect::<String>()) },
                    if r.albums.is_empty() { String::new() } else { format!(r#"{}<div class="cards">{}</div>"#, section_head("Albums"), r.albums.iter().take(6).map(card_html2).collect::<String>()) },
                )
            }
        },
    };
    format!(r#"<div class="search-page"><div class="chips search-tabs">{tabs}</div>{body}</div>"#)
}

fn recent_html(s: &State, v: &View) -> String {
    let cards = match &v.cards {
        None => loading(),
        Some(c) if !c.is_empty() => format!(r#"<div class="cards">{}</div>"#, c.iter().map(card_html2).collect::<String>()),
        Some(_) => format!(r#"<div class="empty small">{}</div>"#, esc(v.error.as_deref().unwrap_or("nothing yet~"))),
    };
    let tracks = v.tracks.clone().unwrap_or_default();
    format!(
        r#"<div class="list-head"><div class="list-cover recent-art">{}</div><div>
      <div class="list-kind">history</div>
      <h1 class="list-title">Recently played</h1>
      <div class="list-meta">where you listened lately, on any device</div>
    </div></div>
    {cards}
    {}"#,
        icon("pie"),
        if tracks.is_empty() { String::new() } else { format!("{}{}", section_head("Songs you played here"), tracks_html(s, &tracks, &RowOpts::default())) }
    )
}

/// The queue page; also returns the rows' tracks (queued songs, then the rest).
pub fn queue_html(s: &State) -> (String, Vec<Track>) {
    let queued: Vec<Track> = s.queue_uris.iter().filter_map(|u| s.cache.get(u).cloned()).collect();
    let rest: Vec<Track> = s
        .next_uris
        .iter()
        .filter(|u| !s.queue_uris.contains(u))
        .filter_map(|u| s.cache.get(u).cloned())
        .collect();
    let mut rows = queued.clone();
    rows.extend(rest.iter().cloned());
    let now = s.now.as_ref().map(|t| {
        let album = match &t.album_uri {
            Some(u) => format!(r#"<span class="alink" data-album="{}">{}</span>"#, esc(u), esc(&t.album)),
            None => esc(&t.album),
        };
        format!(
            r#"{}<div class="tracks now-row"><div class="trow playing"><div class="tnum"><span class="n">♪</span></div>
        <div class="tmain"><img src="{}" alt=""><div class="ttext"><div class="tname">{}</div><div class="tartist">{}</div></div></div>
        <div class="talbum">{album}</div>
        <div></div><div class="tdur">{}</div><div></div></div></div>"#,
            section_head("Now playing"),
            img(&t.image),
            esc(&t.name),
            artist_links(t),
            fmt(t.duration_ms)
        )
    });
    let html = format!(
        r#"<div class="list-head"><div class="list-cover recent-art">{bookmark}</div><div>
      <div class="list-kind">queue</div><h1 class="list-title">Queue</h1>
      <div class="list-meta">{meta}</div>
    </div></div>
    {now}
    {queued}
    {rest}
    {empty}"#,
        bookmark = icon("bookmark"),
        meta = match &s.active_device_name {
            Some(n) => format!("playing on {}", esc(n)),
            None => "nothing is playing".into(),
        },
        now = now.clone().unwrap_or_default(),
        queued = if queued.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="queue-head">{}<button class="chip icon-chip" id="queue-clear">{}Clear queue</button></div>{}"#,
                section_head("Next in queue"),
                icon("clear-queue"),
                tracks_html(s, &queued, &RowOpts { head: false, ..Default::default() })
            )
        },
        rest = if rest.is_empty() {
            String::new()
        } else {
            let from = if s.context.is_empty() { "what's playing".to_string() } else { s.context.clone() };
            format!(
                "{}{}",
                section_head(&format!("Next from: {from}")),
                tracks_html(s, &rest, &RowOpts { head: false, offset: queued.len(), ..Default::default() })
            )
        },
        empty = if now.is_none() && rows.is_empty() { empty_html("play something and it shows up here~") } else { String::new() },
    );
    (html, rows)
}

fn artist_html(s: &State, v: &View) -> String {
    if v.loading {
        return loading();
    }
    let cards = |list: &[AlbumCard]| -> String {
        list.iter()
            .map(|al| {
                format!(
                    r#"<button class="card-item" data-album="{}">
      <img class="cov" src="{}" alt="" loading="lazy">
      <div class="t">{}</div>
      <div class="st">{}</div>
    </button>"#,
                    esc(&al.uri),
                    img(&al.image),
                    esc(&al.name),
                    if al.year > 0 { al.year.to_string() } else { String::new() }
                )
            })
            .collect()
    };
    let following = s.followed.contains(&v.uri);
    let mut meta = vec![];
    if !v.albums.is_empty() {
        meta.push(format!("{} albums", v.albums.len()));
    }
    if !v.singles.is_empty() {
        meta.push(format!("{} singles & EPs", v.singles.len()));
    }
    format!(
        r#"<div class="list-head artist-head"><img class="list-cover artist-cover" src="{img}" alt=""><div>
      <div class="list-kind">artist</div>
      <h1 class="list-title" style="font-size:{size}px">{title}</h1>
      <div class="list-meta">{meta}</div>
    </div></div>
    <div class="list-actions">
      {play}
      <button class="chip icon-chip follow-btn {fcls}" id="artist-follow">{ficon}{flabel}</button>
    </div>
    {popular}
    {albums}
    {singles}"#,
        img = v.image.as_deref().filter(|i| !i.is_empty()).map(esc).unwrap_or_else(|| NO_ARTIST.into()),
        size = if v.title.chars().count() > 24 { 40 } else { 60 },
        title = esc(&v.title),
        meta = meta.join(" · "),
        play = if v.top.is_empty() {
            String::new()
        } else {
            format!(
                r#"<button class="round-play" id="list-play" title="play {}">{}</button>
      <button class="icon-btn" id="list-shuffle" title="shuffle play">{}</button>"#,
                esc(&v.title),
                icon("play"),
                icon("shuffle")
            )
        },
        fcls = if following { "active" } else { "" },
        ficon = icon(if following { "following" } else { "follow" }),
        flabel = if following { "Following" } else { "Follow" },
        popular = if v.top.is_empty() { String::new() } else { format!("{}{}", section_head("Popular"), tracks_html(s, &v.top, &RowOpts::default())) },
        albums = if v.albums.is_empty() { String::new() } else { format!(r#"{}<div class="cards">{}</div>"#, section_head("Albums"), cards(&v.albums)) },
        singles = if v.singles.is_empty() { String::new() } else { format!(r#"{}<div class="cards">{}</div>"#, section_head("Singles and EPs"), cards(&v.singles)) },
    )
}

fn lyrics_view_html(s: &State) -> String {
    let Some(t) = &s.now else { return empty_html("play something first~") };
    match &s.lyrics {
        None => format!(
            r#"<div class="empty"><div class="big">♪</div>{}</div>"#,
            if s.lyrics_for.as_deref() == Some(t.id.as_str()) { "no lyrics for this song (｡•́︿•̀｡)" } else { "loading lyrics..." }
        ),
        Some(lines) => format!(
            r#"<div class="lyrics-view" id="lyrics-view">{}</div>"#,
            lines
                .iter()
                .enumerate()
                .map(|(i, l)| format!(r#"<p data-line="{i}" data-ms="{}">{}</p>"#, l.start_ms, esc(&l.words)))
                .collect::<String>()
        ),
    }
}

/// The main panel for the current page. Also fills `view.shown` / queue rows.
pub fn page_html(s: &mut State) -> String {
    let name = s.view.name.clone();
    match name.as_str() {
        "home" => home_html(s),
        "library" => library_html(s),
        "list" => {
            if s.view.tracks.is_some() && !s.view.loading {
                s.view.shown = Some(shown_tracks(&s.view));
            }
            let v = s.view.clone();
            list_html(s, &v)
        }
        "queue" => {
            let (html, rows) = queue_html(s);
            s.view.tracks = Some(rows);
            s.view.shown = None;
            html
        }
        "lyrics" => lyrics_view_html(s),
        "artist" => {
            let v = s.view.clone();
            artist_html(s, &v)
        }
        "search" => {
            let v = s.view.clone();
            search_html(s, &v)
        }
        "recent" => {
            let v = s.view.clone();
            recent_html(s, &v)
        }
        _ => String::new(),
    }
}
