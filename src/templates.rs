use std::ops::Deref;

use askama_axum::Template;

const PLACEHOLDER: &str = "—";
static LAYOUT: Layout = Layout;

#[derive(Default, Template)]
#[template(path = "_layout.html")]
pub struct Layout;

#[derive(Default, Template)]
#[template(path = "_search_bar.html")]
pub struct SearchBar {
    query: Option<String>,
}

impl SearchBar {
    pub fn new(query: impl Into<Option<String>>) -> Self {
        Self {
            query: query.into(),
        }
    }

    pub fn query(&self) -> &str {
        self.query.as_deref().unwrap_or_default()
    }
}

#[derive(Template)]
#[template(path = "recipe.html")]
pub struct Recipe<'r> {
    parent: &'r Layout,
    search_bar: SearchBar,
    recipe: crate::recipe::Recipe,
    title: String,
}

impl From<crate::recipe::Recipe> for Recipe<'static> {
    fn from(recipe: crate::recipe::Recipe) -> Self {
        let title = String::from(recipe.metadata().map_or("Unknown", |md| md.title()));
        Self {
            parent: &LAYOUT,
            search_bar: Default::default(),
            recipe,
            title,
        }
    }
}

impl Deref for Recipe<'_> {
    type Target = Layout;

    fn deref(&self) -> &Self::Target {
        self.parent
    }
}

#[derive(Template)]
#[template(path = "search.html")]
pub struct Search<'s> {
    parent: &'s Layout,
    search_bar: SearchBar,
    results: crate::search::SearchResult,
}

impl Search<'_> {
    pub fn new(query: impl Into<Option<String>>, results: crate::search::SearchResult) -> Self {
        Self {
            parent: &LAYOUT,
            search_bar: SearchBar::new(query),
            results,
        }
    }

    pub fn has_many_categories(&self) -> bool {
        self.results.categories().keys().len() > 1
    }

    pub fn has_many_tags(&self) -> bool {
        self.results.tags().keys().len() > 1
    }

    pub fn is_filterable(&self) -> bool {
        self.has_many_categories() || self.has_many_tags()
    }
}

impl Default for Search<'static> {
    fn default() -> Self {
        Self {
            parent: &LAYOUT,
            search_bar: Default::default(),
            results: Default::default(),
        }
    }
}

impl Deref for Search<'_> {
    type Target = Layout;

    fn deref(&self) -> &Self::Target {
        self.parent
    }
}
