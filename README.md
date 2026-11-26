# ♡ femboy spotify ♡

A cute, pink terminal Spotify client written in Rust. ✧

```
╭────────────────────────────────────────────────────────────────────────────────────────────╮
│♡ f e m b o y   s p o t i f y ♡                                             ✧ hi, cutie ✧ │
╰────────────────────────────────────────────────────────────────────────────────────────────╯
╭ menu ──────────────────╮╭ ♥ liked songs ─────────────────────────────────────────────────────╮
│  ✿ search              ││  #    title                artist               album        time │
│❥ ♥ liked songs         ││                                                                  │
│  ♫ playlists           ││❥ 1    song number 1        some artist          pink album   3:00 │
╰────────────────────────╯│  ♪    song number 2        some artist          pink album   3:00 │
╭ up next ───────────────╮│  3    song number 3        some artist          pink album   3:00 │
│· song number 3         ││                                                                  │
╰────────────────────────╯╰──────────────────────────────────────────────────────────────────╯
╭ now playing ───────────────────────────────────────────────────────────────────────────────╮
│▶ song number 2 — some artist                                                               │
│████████████████████████████████ 1:12 / 3:00                                                │
│vol ♥♥♥♥♥♥♥♡♡♡ 70%                                                                          │
╰────────────────────────────────────────────────────────────────────────────────────────────╯
```

## Features

- plays music right in your terminal (no official app needed) via [librespot](https://github.com/librespot-org/librespot)
- search tracks, liked songs, your playlists
- queue, shuffle, seek, volume
- pastel pink femboy-flag theme with an animated logo ♡

## Requirements

- **Spotify Premium** account (librespot only works with Premium)
- a terminal with true color support (Windows Terminal, iTerm2, kitty, WezTerm, Alacritty, ...)
- [Rust](https://rustup.rs) 1.85+ to build from source
- Linux only: ALSA dev headers (`sudo apt install libasound2-dev pkg-config`)

## Install & run

```sh
git clone https://github.com/ScaraKazuha/femboy-spotify
cd femboy-spotify
cargo run --release
```

On first launch your browser opens a Spotify sign-in page. After you log in,
credentials are cached (in your OS cache dir, `femboy-spotify/`), so next time it starts instantly.

## Keys

| key | action |
|---|---|
| `/` | search |
| `enter` | play track / open playlist |
| `space` | pause / resume |
| `n` / `p` | next / previous |
| `←` / `→` | seek 10 s |
| `+` / `-` | volume |
| `s` | shuffle |
| `tab` | switch between menu and list |
| `↑↓` / `j k` | move |
| `esc` | back to playlists |
| `?` | help |
| `q` | quit |

## По-русски

Милый розовый терминальный клиент Spotify на Rust. Нужен Spotify Premium.
Запуск: `cargo run --release`. При первом запуске откроется браузер для входа в Spotify.

## Disclaimer

This is an unofficial hobby project, not affiliated with or endorsed by Spotify.
It uses librespot, an open-source reverse-engineered client library; using third-party
clients may be against Spotify's Terms of Service. Use at your own risk.

## License

MIT
