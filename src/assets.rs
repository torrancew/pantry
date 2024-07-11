use axum::{
    http::header::CONTENT_TYPE,
    response::{IntoResponse, Response},
};
use mime_guess::Mime;
use rust_embed::{Embed, EmbeddedFile};

#[derive(Embed)]
#[folder = "assets/"]
struct Asset;

pub struct StaticFile {
    mime: Mime,
    contents: EmbeddedFile,
}

impl StaticFile {
    pub fn new(path: impl Into<String>) -> Option<Self> {
        let path = path.into();
        Asset::get(&path).map(|contents| Self {
            contents,
            mime: mime_guess::from_path(path).first_or_octet_stream(),
        })
    }
}

impl IntoResponse for StaticFile {
    fn into_response(self) -> Response {
        ([(CONTENT_TYPE, self.mime.as_ref())], self.contents.data).into_response()
    }
}
