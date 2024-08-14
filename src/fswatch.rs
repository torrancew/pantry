use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use notify::{Result, Watcher};
use smol::{
    channel,
    stream::{Stream, StreamExt},
};

#[derive(Clone)]
pub struct AsyncWatcher {
    _inner: Arc<notify::RecommendedWatcher>,
    channel: Pin<Box<smol::channel::Receiver<notify::Event>>>,
}

pub enum Event {
    Update(PathBuf),
    Remove(PathBuf),
}

impl AsyncWatcher {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let (tx, rx) = channel::bounded(1);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res: Result<notify::Event>| {
                use notify::{
                    event::{AccessKind, RemoveKind},
                    EventKind::{Access, Remove},
                };
                if let Ok(ev) = res {
                    match ev.kind {
                        Remove(RemoveKind::File) | Access(AccessKind::Close(_)) => {
                            tx.send_blocking(ev).unwrap();
                        }
                        _ => (),
                    }
                }
            },
            notify::Config::default(),
        )?;

        watcher.watch(path.as_ref(), notify::RecursiveMode::Recursive)?;

        Ok(Self {
            _inner: Arc::new(watcher),
            channel: Box::pin(rx),
        })
    }
}

impl Stream for AsyncWatcher {
    type Item = notify::Event;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.channel.poll_next(cx)
    }
}
