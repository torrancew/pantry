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
    channel: Pin<Box<smol::channel::Receiver<Event>>>,
}

pub enum Event {
    Update(Vec<PathBuf>),
    Remove(Vec<PathBuf>),
}

impl Event {
    pub fn new(ev: notify::Event) -> Option<impl IntoIterator<Item = Self>> {
        use notify::{
            event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode},
            EventKind::{Access, Create, Modify, Remove},
        };

        match ev.kind {
            Access(AccessKind::Close(_)) | Create(CreateKind::File) => {
                Some(vec![Event::Update(ev.paths)])
            }
            Modify(ModifyKind::Name(RenameMode::Both)) => Some(vec![
                Event::Remove(vec![ev.paths.first().unwrap().clone()]),
                Event::Update(vec![ev.paths.get(1).unwrap().clone()]),
            ]),
            Remove(RemoveKind::File) => Some(vec![Event::Remove(ev.paths)]),
            _ => None,
        }
    }
}

impl AsyncWatcher {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let (tx, rx) = channel::bounded(1);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res: Result<notify::Event>| {
                if let Ok(Some(events)) = res.map(Event::new) {
                    for ev in events {
                        tx.send_blocking(ev).unwrap();
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
    type Item = Event;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.channel.poll_next(cx)
    }
}
