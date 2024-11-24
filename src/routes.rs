use std::collections::BTreeMap;

use crate::templates;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
enum Error {
    #[error("content not found")]
    NotFound,
    #[error("xapian error: {0}")]
    Xapian(#[from] crate::search::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => (StatusCode::NOT_FOUND, "Content not found!"),
            Error::Xapian(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Search index is unavailable!",
            ),
        }
        .into_response()
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
pub struct AppState {
    xapian: crate::search::AsyncIndex,
}

impl AppState {
    const DEFAULT_PAGE_SIZE: u32 = 50;

    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        let xapian = crate::search::AsyncIndex::new(&path).unwrap();
        Self { xapian }
    }

    pub async fn query(
        &self,
        query: impl AsRef<str>,
        start: impl Into<Option<u32>>,
        size: impl Into<Option<u32>>,
    ) -> Result<
        (
            BTreeMap<String, usize>,
            Vec<crate::recipe::Recipe>,
            BTreeMap<String, usize>,
        ),
        crate::search::Error,
    > {
        self.xapian
            .query(
                query.as_ref(),
                start.into().unwrap_or(0),
                size.into().unwrap_or(Self::DEFAULT_PAGE_SIZE),
            )
            .await
    }

    pub async fn recipe(&self, slug: impl AsRef<str>) -> Option<crate::recipe::Recipe> {
        let (_, recipes, _) = self
            .query(format!("slug:{}", slug.as_ref()), 0, 1)
            .await
            .ok()?;
        recipes.first().cloned()
    }

    pub async fn reload(&self, paths: Option<Vec<std::path::PathBuf>>) {
        if let Some(ref paths) = paths {
            info!(
                "Reloading entries: {:?}",
                paths
                    .iter()
                    .map(|p| p.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        } else {
            info!("Reloading all entries");
        }
        let _ = self.xapian.reindex(paths).await;
    }

    pub async fn remove(&self, paths: Vec<std::path::PathBuf>) {
        info!(
            "Removing entries: {}",
            paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(",")
        );
        let _ = self.xapian.remove(paths).await;
    }
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
    start: Option<u32>,
    size: Option<u32>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/assets/*file", get(asset_handler))
        .route("/", get(index))
        .route("/recipe/:id", get(recipe))
        .route("/search", get(search))
        .with_state(state)
}

async fn asset_handler(Path(file): Path<String>) -> Result<crate::assets::StaticFile> {
    crate::assets::StaticFile::new(file).ok_or(Error::NotFound)
}

async fn index() -> impl IntoResponse {
    Redirect::temporary("/search")
}

async fn recipe(
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> Result<templates::Recipe<'static>> {
    state
        .recipe(slug)
        .await
        .map(templates::Recipe::from)
        .ok_or(Error::NotFound)
}

async fn search(
    params: Option<Query<SearchParams>>,
    State(state): State<AppState>,
) -> Result<templates::Search<'static>> {
    if let Some(Query(SearchParams { query, start, size })) = params {
        let (categories, recipes, tags) = state.query(&query, start, size).await?;
        Ok(templates::Search::new(query, recipes, categories, tags))
    } else {
        Ok(templates::Search::default())
    }
}
