use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use notify::{Result, Watcher};
use tokio::sync::{mpsc::{channel, Receiver}, Mutex};

#[derive(Clone)]
pub struct AsyncWatcher {
    _inner: Arc<notify::RecommendedWatcher>,
    channel: Arc<Mutex<Receiver<notify::Result<Event>>>>,
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
        let (tx, rx) = channel(1);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                tx.blocking_send(res.map(|e| Event::new(e).unwrap().into_iter().next().unwrap())).unwrap();
            },
            notify::Config::default(),
        )?;

        watcher.watch(path.as_ref(), notify::RecursiveMode::Recursive)?;

        Ok(Self {
            _inner: Arc::new(watcher),
            channel: Arc::new(Mutex::new(rx)),
        })
    }

    pub async fn next(&mut self) -> Option<notify::Result<Event>> {
        self.channel.lock().await.recv().await
    }
}

