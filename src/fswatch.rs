use std::{path::Path, pin::Pin, sync::Arc};

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

impl AsyncWatcher {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let (tx, rx) = channel::bounded(1);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res: Result<notify::Event>| {
                use notify::{event::AccessKind, EventKind};
                if let Ok(ev) = res {
                    if let EventKind::Access(AccessKind::Close(_)) = ev.kind {
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
    type Item = notify::Event;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.channel.poll_next(cx)
    }
}
