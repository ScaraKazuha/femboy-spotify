# ♡ femboy spotify ♡

A cute, pastel-pink Spotify client written in Rust. ✧

![femboy spotify](docs/screenshot.png)

*(screenshot from demo mode: with your account you see your real playlists and album covers)*

## Features

- plays music right inside the app (no official Spotify app needed) via [librespot](https://github.com/librespot-org/librespot)
- home screen with a big custom banner, shortcut tiles, and "made for you" cards
- liked songs, playlists, recently played, search
- now-playing panel with big cover art and **time-synced lyrics** (click a line to jump to it)
- queue, shuffle, repeat (all / one), seek, volume, add to liked songs
- custom banner: click ✎ on the banner (or the cat in the sidebar) to set your own picture and title
- frameless pastel window, keyboard shortcuts
- bonus: a terminal version (`femboy-spotify-tui`) for terminal lovers

## Requirements

- **Spotify Premium** account (librespot only works with Premium)
- [Rust](https://rustup.rs) 1.85+ to build from source
- Windows 10/11 (uses WebView2, already installed on Windows 11)
- Linux: `sudo apt install libwebkit2gtk-4.1-dev libasound2-dev pkg-config build-essential`

## Install & run

```sh
git clone https://github.com/ScaraKazuha/femboy-spotify
cd femboy-spotify
cargo run --release
```

On first launch, click **sign in with Spotify**. Your browser opens the Spotify login page.
After that, your login is saved (in your OS cache dir, `femboy-spotify/`), so next time the app starts right away.
To log out, click the ribbon 🎀 at the bottom of the sidebar.

Terminal version:

```sh
cargo run --release --bin femboy-spotify-tui
```

## Keys

| key | action |
|---|---|
| `space` | play / pause |
| `←` / `→` | seek 10 s |
| `↑` / `↓` | volume |
| `n` / `p` | next / previous |
| `/` or `ctrl+L` | search |
| `alt+←` / `alt+→` | back / forward |

## Hacking on the UI

The interface is plain HTML/CSS/JS in [`ui/`](ui). Open `ui/index.html` through any local web server
to get **demo mode** with fake data, no Spotify login needed. The Rust side lives in `src/`:
`auth.rs` (login), `api.rs` (Spotify Web API + lyrics), `player.rs` (librespot playback), `main.rs` (Tauri commands).

## По-русски

Милый пастельно-розовый клиент Spotify на Rust: обложки, тексты песен, очередь, свой баннер.
Нужен Spotify Premium. Запуск: `cargo run --release`. При первом запуске нажмите
«sign in with Spotify», в браузере откроется вход в Spotify.

## Disclaimer

This is an unofficial hobby project, not affiliated with or endorsed by Spotify.
It uses librespot, an open-source reverse-engineered client library; using third-party
clients may be against Spotify's Terms of Service. Use at your own risk.

## License

MIT
