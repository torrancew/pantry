use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use crate::templates;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use smol::{lock::Mutex, stream::StreamExt};
use thiserror::Error;

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
    recipe_dir: PathBuf,
    recipe_map: Arc<Mutex<BTreeMap<String, crate::recipe::Recipe>>>,
    xapian: crate::search::AsyncIndex,
}

impl AppState {
    const DEFAULT_PAGE_SIZE: u32 = 50;

    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        let recipe_dir = PathBuf::from(path.as_ref());
        let xapian = crate::search::AsyncIndex::new(&recipe_dir).unwrap();
        Self {
            xapian,
            recipe_dir,
            recipe_map: Default::default(),
        }
    }

    pub async fn categorized_recipes(&self) -> BTreeMap<String, Vec<crate::recipe::Recipe>> {
        let mut categorized_recipes = BTreeMap::default();
        for recipe in self.recipe_map.lock().await.values() {
            let category = recipe.metadata().map_or("Unknown", |md| md.category());
            categorized_recipes
                .entry(String::from(category))
                .and_modify(|v: &mut Vec<_>| v.push(recipe.clone()))
                .or_insert(vec![recipe.clone()]);
        }
        categorized_recipes
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

    pub async fn reload(&self) {
        let mut recipes = BTreeMap::default();
        let mut recipe_loader =
            Box::pin(crate::recipe::Recipe::load_all_async(&self.recipe_dir).await);

        while let Some(recipe) = recipe_loader.next().await {
            if let Some(md) = recipe.metadata() {
                recipes.insert(slug::slugify(md.title()), recipe);
            }
        }

        let mut map = self.recipe_map.lock().await;
        *map = recipes;

        let _ = self.xapian.reindex().await;
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

async fn index(State(state): State<AppState>) -> Result<templates::RecipeIndex<'static>> {
    Ok(templates::RecipeIndex::from(
        state.categorized_recipes().await,
    ))
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
