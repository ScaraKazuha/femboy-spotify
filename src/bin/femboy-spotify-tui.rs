//! Terminal version: `cargo run --release --bin femboy-spotify-tui`

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    femboy_spotify::tui::main().await
}
