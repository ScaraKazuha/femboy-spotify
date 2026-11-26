//! ♡ femboy spotify ♡ — a cute pink terminal Spotify client.

mod api;
mod app;
mod auth;
mod player;
mod theme;
mod ui;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use api::Api;
use app::{Action, App, Msg};
use player::Player;

const START_VOLUME: u16 = 70;

#[tokio::main]
async fn main() -> Result<()> {
    let session = auth::connect().await?;
    let api = Api::new(session.clone());
    let (player, player_events) = Player::new(session, START_VOLUME)?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, api, player, player_events).await;
    ratatui::restore();

    if let Err(e) = &result {
        eprintln!("femboy spotify crashed (｡•́︿•̀｡): {e:#}");
    } else {
        println!("bye bye~ ♡");
    }
    result
}

async fn run(
    terminal: &mut DefaultTerminal,
    api: Api,
    player: Player,
    mut player_events: mpsc::UnboundedReceiver<player::Event>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let mut app = App::new(START_VOLUME);
    let mut keys = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(150));

    // greet the user and show their liked songs right away
    {
        let (api, tx) = (api.clone(), tx.clone());
        tokio::spawn(async move {
            if let Ok(name) = api.display_name().await {
                let _ = tx.send(Msg::User(name));
            }
        });
    }
    perform(Action::LoadLiked, &api, &player, &tx);

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let action = tokio::select! {
            _ = tick.tick() => {
                app.tick = app.tick.wrapping_add(1);
                None
            }
            ev = keys.next() => match ev {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => app.on_key(key),
                Some(Err(e)) => return Err(e.into()),
                None => break,
                _ => None,
            },
            Some(msg) = rx.recv() => {
                app.on_msg(msg);
                None
            }
            Some(ev) = player_events.recv() => app.on_player_event(ev),
        };

        if let Some(action) = action {
            perform(action, &api, &player, &tx);
        }
        if app.should_quit {
            break;
        }
    }
    player.pause();
    Ok(())
}

/// Runs side effects. Network calls go to background tasks so the UI never freezes.
fn perform(action: Action, api: &Api, player: &Player, tx: &mpsc::UnboundedSender<Msg>) {
    match action {
        Action::Play(uri) => {
            if let Err(e) = player.load(&uri) {
                let _ = tx.send(Msg::Error(e.to_string()));
            }
        }
        Action::Pause => player.pause(),
        Action::Resume => player.play(),
        Action::Seek(ms) => player.seek(ms),
        Action::Volume(v) => player.set_volume(v),
        Action::Search(q) => spawn(api, tx, move |api| async move {
            let tracks = api.search(&q).await?;
            Ok(Msg::Tracks { title: format!("results for \"{q}\""), tracks })
        }),
        Action::LoadLiked => spawn(api, tx, |api| async move {
            Ok(Msg::Tracks { title: "♥ liked songs".into(), tracks: api.liked().await? })
        }),
        Action::LoadPlaylists => spawn(api, tx, |api| async move {
            Ok(Msg::Playlists(api.playlists().await?))
        }),
        Action::LoadPlaylist { id, name } => spawn(api, tx, move |api| async move {
            Ok(Msg::Tracks { title: format!("♫ {name}"), tracks: api.playlist_tracks(&id).await? })
        }),
    }
}

fn spawn<F, Fut>(api: &Api, tx: &mpsc::UnboundedSender<Msg>, job: F)
where
    F: FnOnce(Api) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Msg>> + Send + 'static,
{
    let (api, tx) = (api.clone(), tx.clone());
    tokio::spawn(async move {
        let msg = job(api).await.unwrap_or_else(|e| Msg::Error(format!("{e:#}")));
        let _ = tx.send(msg);
    });
}
