//! Icons for the Spotify theme, drawn for this app in Spotify's style: solid shapes
//! and thick rounded lines (24×24, fill = currentColor). Not Spotify's own files.

/// Stroked parts (`{S}` below; 2.3 wide unless a width follows).
const STROKE: &str = r##"fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round""##;

fn strokes(s: &str) -> String {
    let mut parts = s.split("{S}");
    let mut out = parts.next().unwrap_or_default().to_string();
    for p in parts {
        out.push_str(STROKE);
        if !p.starts_with(" stroke-width") {
            out.push_str(r#" stroke-width="2.3""#);
        }
        out.push_str(p);
    }
    out
}

pub fn spot(name: &str) -> Option<String> {
    let s = match name {
        "home" => r##"<path d="M11.1 2.6a1.4 1.4 0 0 1 1.8 0l8.3 7.1c.5.4.2 1.3-.5 1.3H20v8.5a1.5 1.5 0 0 1-1.5 1.5H15v-6.5H9V21H5.5A1.5 1.5 0 0 1 4 19.5V11h-.7c-.7 0-1-.9-.5-1.3z"/>"##,
        "search" | "search-in-list" => r##"<circle cx="10.5" cy="10.5" r="6.4" {S} stroke-width="2.6"/><path d="m15.4 15.4 5 5" {S} stroke-width="2.8"/>"##,
        "browse" => r##"<path d="M3 5.5A1.5 1.5 0 0 1 4.5 4h15A1.5 1.5 0 0 1 21 5.5V9H3z"/><path d="M4.5 11h15v7a2 2 0 0 1-2 2h-11a2 2 0 0 1-2-2z" {S} stroke-width="2"/><path d="M10 14.5h4" {S} stroke-width="2"/>"##,
        "library" => r##"<rect x="3.5" y="3" width="3.2" height="18" rx="1"/><rect x="9" y="3" width="3.2" height="18" rx="1"/><path d="m14.3 4.6 3-1.1 5.7 16.9-3 1.1z"/>"##,
        "plus-circle" | "save-album" => r##"<circle cx="12" cy="12" r="9" {S} stroke-width="2"/><path d="M12 7.5v9M7.5 12h9" {S}/>"##,
        "check-circle" | "album-saved" => r##"<circle cx="12" cy="12" r="10"/><path d="m7.4 12.3 3.1 3.1 6.1-6.3" fill="none" stroke="#000" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>"##,
        "heart" => r##"<path d="M12 21s-8.2-5.1-10-10.2C.8 7 3.2 3.5 6.8 3.5c2.1 0 3.9 1.1 5.2 2.9 1.3-1.8 3.1-2.9 5.2-2.9 3.6 0 6 3.5 4.8 7.3C20.2 15.9 12 21 12 21z"/>"##,
        "bookmark" => r##"<path d="M6 3h12a1 1 0 0 1 1 1v17.2l-7-4.6-7 4.6V4a1 1 0 0 1 1-1z"/>"##,
        "pie" => r##"<path d="M10.8 3.1A9 9 0 1 0 20.9 13.2H10.8z"/><path d="M13 1.6V11h9.4A9.6 9.6 0 0 0 13 1.6z"/>"##,
        "play" => r##"<path d="M7 4.3v15.4a1.1 1.1 0 0 0 1.7.9l12.2-7.7a1.1 1.1 0 0 0 0-1.8L8.7 3.4A1.1 1.1 0 0 0 7 4.3z"/>"##,
        "pause" => r##"<rect x="5.5" y="3.5" width="4.6" height="17" rx="1.3"/><rect x="13.9" y="3.5" width="4.6" height="17" rx="1.3"/>"##,
        "prev" => r##"<rect x="3.5" y="4" width="2.8" height="16" rx="1"/><path d="M20 5.2v13.6a1.1 1.1 0 0 1-1.7.9L8.6 12.9a1.1 1.1 0 0 1 0-1.8l9.7-6.8a1.1 1.1 0 0 1 1.7.9z"/>"##,
        "next" => r##"<rect x="17.7" y="4" width="2.8" height="16" rx="1"/><path d="M4 5.2v13.6a1.1 1.1 0 0 0 1.7.9l9.7-6.8a1.1 1.1 0 0 0 0-1.8L5.7 4.3A1.1 1.1 0 0 0 4 5.2z"/>"##,
        "shuffle" => r##"<path d="M2.5 7h3.6c2 0 3.1.8 4.2 2.5l2.6 4.2c1 1.6 2.2 2.3 4.2 2.3h2.4M2.5 17h3.6c1.5 0 2.5-.5 3.4-1.6M14.2 8.5c.9-1 1.9-1.5 3.4-1.5h2.4" {S}/><path d="M18.7 3.6 22.3 7l-3.6 3.4zM18.7 12.6l3.6 3.4-3.6 3.4z"/>"##,
        "repeat" => r##"<path d="M3.5 12.5V10a4 4 0 0 1 4-4H19M20.5 11.5V14a4 4 0 0 1-4 4H5" {S}/><path d="M17.6 2.6 21.2 6l-3.6 3.4zM6.4 14.6 2.8 18l3.6 3.4z"/>"##,
        "repeat-one" => r##"<path d="M3.5 12.5V10a4 4 0 0 1 4-4H19M20.5 11.5V14a4 4 0 0 1-4 4H5" {S}/><path d="M17.6 2.6 21.2 6l-3.6 3.4zM6.4 14.6 2.8 18l3.6 3.4z"/><path d="M10.6 10.4 12.5 9h1.3v6h-2v-3.4l-.8.5z"/>"##,
        "disc" => r##"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm0 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"/>"##,
        "pen" | "rename" => r##"<path d="M15.2 3.8a2.3 2.3 0 0 1 3.2 0l1.8 1.8a2.3 2.3 0 0 1 0 3.2L9.6 19.4 3.5 20.5l1.1-6.1z"/>"##,
        "delete" => r##"<path d="M9 2.5h6l.8 1.5H20v2.3H4V4h4.2z"/><path d="M5.4 7.6h13.2l-.9 12.2a2 2 0 0 1-2 1.7H8.3a2 2 0 0 1-2-1.7z"/>"##,
        "moon" => r##"<path d="M20.6 14.6A8.6 8.6 0 0 1 9.4 3.4a8.6 8.6 0 1 0 11.2 11.2z"/>"##,
        "sun" => r##"<circle cx="12" cy="12" r="4.8"/><path d="M12 1.8v2.4M12 19.8v2.4M1.8 12h2.4M19.8 12h2.4M4.8 4.8l1.7 1.7M17.5 17.5l1.7 1.7M4.8 19.2l1.7-1.7M17.5 6.5l1.7-1.7" {S}/>"##,
        "plus" => r##"<path d="M12 4v16M4 12h16" {S} stroke-width="2.6"/>"##,
        "minus" => r##"<path d="M4 12h16" {S} stroke-width="2.6"/>"##,
        "create" => r##"<rect x="3" y="3" width="18" height="18" rx="4" {S} stroke-width="2"/><path d="M12 8v8M8 12h8" {S}/>"##,
        "timer" => r##"<circle cx="12" cy="13.5" r="8" {S}/><path d="M12 9.5v4.3l2.6 1.6M9 2.5h6" {S}/>"##,
        "radio" => r##"<circle cx="12" cy="12" r="2.8"/><path d="M7.8 7.8a6 6 0 0 0 0 8.4M16.2 7.8a6 6 0 0 1 0 8.4M4.9 4.9a10 10 0 0 0 0 14.2M19.1 4.9a10 10 0 0 1 0 14.2" {S} stroke-width="2.1"/>"##,
        "user" => r##"<circle cx="12" cy="7.5" r="4.6"/><path d="M3.5 21.5c.6-4.8 4-7.8 8.5-7.8s7.9 3 8.5 7.8z"/>"##,
        "users" => r##"<circle cx="9" cy="8" r="3.8"/><path d="M2 20.5c.5-4 3.4-6.4 7-6.4s6.5 2.4 7 6.4z"/><circle cx="17.2" cy="8.6" r="2.9"/><path d="M17.8 14.2c2.6.4 4 2.2 4.3 5h-4.6c-.2-2-.9-3.6-2-4.6.7-.3 1.5-.4 2.3-.4z"/>"##,
        "follow" => r##"<circle cx="9" cy="8" r="3.8"/><path d="M2 20.5c.5-4 3.4-6.4 7-6.4s6.5 2.4 7 6.4z"/><path d="M19 7.5v7M15.5 11h7" {S} stroke-width="2.2"/>"##,
        "following" => r##"<circle cx="9" cy="8" r="3.8"/><path d="M2 20.5c.5-4 3.4-6.4 7-6.4s6.5 2.4 7 6.4z"/><path d="m15.6 11 2.2 2.2 4.4-4.6" {S} stroke-width="2.2"/>"##,
        "share" => r##"<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><path d="m8.6 10.6 6.8-4M8.6 13.4l6.8 4" {S} stroke-width="2"/>"##,
        "link" => r##"<path d="M10 14a4.2 4.2 0 0 0 6 0l3-3a4.2 4.2 0 0 0-6-6l-1 1M14 10a4.2 4.2 0 0 0-6 0l-3 3a4.2 4.2 0 0 0 6 6l1-1" {S}/>"##,
        "np" => r##"<path fill-rule="evenodd" d="M4 3h16a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm.6 2.6v12.8h14.8V5.6z"/><path d="M9.8 8.4v7.2l5.7-3.6z"/>"##,
        "mic" => r##"<rect x="8.5" y="2" width="7" height="12.2" rx="3.5"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3.5" {S}/>"##,
        "queue" => r##"<rect x="3" y="3.5" width="18" height="7.5" rx="1.6"/><path d="M3.5 15.5h17M3.5 20h11" {S}/>"##,
        "devices" => r##"<path d="M2.5 4.5A1.5 1.5 0 0 1 4 3h11.5A1.5 1.5 0 0 1 17 4.5V7h-2.2V5.2H4.7v9.6H12V17H4a1.5 1.5 0 0 1-1.5-1.5z"/><path d="M6 19h6v2.2H6z"/><rect x="14.3" y="8.8" width="6.7" height="12.2" rx="1.6" {S} stroke-width="2"/><circle cx="17.65" cy="16.3" r="1.6"/>"##,
        "volume" => r##"<path d="M2.5 9.3h3.9L12 4.6v14.8l-5.6-4.7H2.5z"/><path d="M15.5 8.8a4.5 4.5 0 0 1 0 6.4M18.3 6a8.4 8.4 0 0 1 0 12" {S} stroke-width="2.1"/>"##,
        "volume-off" => r##"<path d="M2.5 9.3h3.9L12 4.6v14.8l-5.6-4.7H2.5z"/><path d="m15.6 9.4 5.2 5.2M20.8 9.4l-5.2 5.2" {S} stroke-width="2.1"/>"##,
        "mini" => r##"<rect x="2" y="4" width="20" height="16" rx="2.2" {S} stroke-width="2"/><rect x="11.2" y="11" width="8.2" height="6.4" rx="1.1"/>"##,
        "fullscreen" => r##"<path d="M3.5 9V5A1.5 1.5 0 0 1 5 3.5h4M15 3.5h4A1.5 1.5 0 0 1 20.5 5v4M20.5 15v4a1.5 1.5 0 0 1-1.5 1.5h-4M9 20.5H5A1.5 1.5 0 0 1 3.5 19v-4" {S}/>"##,
        "dots" => r##"<circle cx="5" cy="12" r="2.1"/><circle cx="12" cy="12" r="2.1"/><circle cx="19" cy="12" r="2.1"/>"##,
        "bell" => r##"<path d="M12 2.5A6.6 6.6 0 0 0 5.4 9.1v4.4l-2 3.5h17.2l-2-3.5V9.1A6.6 6.6 0 0 0 12 2.5z"/><path d="M9.3 18.6a2.7 2.7 0 0 0 5.4 0z"/>"##,
        "chevron-left" => r##"<path d="M15 4.5 7.5 12l7.5 7.5" {S} stroke-width="2.6"/>"##,
        "chevron-right" => r##"<path d="M9 4.5 16.5 12 9 19.5" {S} stroke-width="2.6"/>"##,
        "clock" => r##"<path fill-rule="evenodd" d="M12 2a10 10 0 1 1 0 20 10 10 0 0 1 0-20zm-1.1 4.5v6.3l4.4 2.6 1.1-1.9-3.3-1.9v-5z"/>"##,
        "sort" => r##"<path d="M4 6.5h16M6.5 12h11M9.5 17.5h5" {S} stroke-width="2.4"/>"##,
        "menu" => r##"<path d="M4 6.5h16M4 12h16M4 17.5h16" {S} stroke-width="2.4"/>"##,
        "clear-queue" => r##"<path d="M3.5 6.5h11M3.5 12h11M3.5 17.5h7" {S}/><path d="m15.5 14.5 5 5M20.5 14.5l-5 5" {S}/>"##,
        "sparkle" => r##"<path d="M12 2c.9 6.2 2.9 8.2 10 10-7.1 1.8-9.1 3.8-10 10-.9-6.2-2.9-8.2-10-10 7.1-1.8 9.1-3.8 10-10z"/>"##,
        _ => return None,
    };
    Some(strokes(s))
}
