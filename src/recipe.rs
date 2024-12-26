use crate::markdown;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display},
    io::{self, Read},
    path::{Path, PathBuf},
};

use axum::{
    http::header,
    response::{IntoResponse, Response},
};
use recipe_scraper::SchemaOrgRecipe;
use serde::{Deserialize, Serialize};
use url::Url;
use yaml_front_matter::YamlFrontMatter;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Category(String);

impl AsRef<str> for Category {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<String> for Category {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Source {
    Book { title: String, author: String },
    Url { name: String, url: Url },
}

impl Source {
    pub fn name(&self) -> &str {
        match self {
            Self::Book { title, .. } => title.as_str(),
            Self::Url { name, .. } => name.as_str(),
        }
    }

    pub fn attribution(&self) -> &str {
        match self {
            Self::Book { author, .. } => author.as_str(),
            Self::Url { url, .. } => url.as_str(),
        }
    }

    pub fn url(&self) -> Option<Url> {
        match self {
            Self::Url { url, .. } => Some(url.clone()),
            Self::Book { title, author } => url::Url::parse_with_params(
                "https://www.google.com/search",
                &[
                    ("tbm", "bks"),
                    (
                        "q",
                        &format!(r#"intitle:"{title}" AND inauthor:"{author}""#),
                    ),
                ],
            )
            .ok(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct MetaData {
    title: String,
    category: Category,
    #[serde(default)]
    sources: Vec<Source>,
    #[serde(default)]
    tags: BTreeSet<String>,
}

impl MetaData {
    pub fn category(&self) -> &Category {
        &self.category
    }

    pub fn slug(&self) -> String {
        slug::slugify(self.title())
    }

    pub fn sources(&self) -> &Vec<Source> {
        &self.sources
    }

    pub fn tags(&self) -> &BTreeSet<String> {
        &self.tags
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Recipe {
    metadata: Option<MetaData>,
    contents: String,
}

impl From<SchemaOrgRecipe> for Recipe {
    fn from(recipe: SchemaOrgRecipe) -> Self {
        let metadata = MetaData {
            title: recipe.name().clone(),
            category: String::from("Imported").into(),
            sources: Default::default(),
            tags: Default::default(),
        }
        .into();

        let mut markdown = String::with_capacity(2048);

        markdown.push_str(recipe.description());
        markdown.push_str("\n\n");

        if let Some(prep_time) = recipe.prep_time().as_ref().and_then(|d| d.human_readable()) {
            markdown.push_str("**Prep Time:** ");
            markdown.push_str(&prep_time);
            markdown.push_str("\n\n");
        }

        if let Some(cook_time) = recipe.cook_time().as_ref().and_then(|d| d.human_readable()) {
            markdown.push_str("**Cook Time:** ");
            markdown.push_str(&cook_time);
            markdown.push_str("\n\n");
        }

        if let Some(total_time) = recipe
            .total_time()
            .as_ref()
            .and_then(|d| d.human_readable())
        {
            markdown.push_str("**Total Time:** ");
            markdown.push_str(&total_time);
            markdown.push_str("\n\n");
        }

        let ingredients = recipe
            .ingredients()
            .clone()
            .into_iter()
            .map(|i| format!("- {i}\n"))
            .collect::<Vec<_>>();

        if !ingredients.is_empty() {
            markdown.push_str("## Ingredients\n\n");
            markdown.push_str(&ingredients.join(""));
            markdown.push('\n');
        }

        if let Some(directions) = recipe.directions().clone() {
            markdown.push_str("## Directions\n\n");
            if let Some(sections) = directions.sections() {
                for section in sections {
                    markdown.push_str(&format!("### {}\n\n", section.name()));
                    let steps = section
                        .clone()
                        .into_iter()
                        .map(|step| format!("- {step}\n"))
                        .collect::<Vec<_>>();
                    markdown.push_str(&steps.join(""));
                    markdown.push('\n');
                }
            } else if let Some(directions) = directions.directions() {
                markdown.push_str(
                    &directions
                        .into_iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(""),
                )
            }

            markdown.push_str("\n\n");
        }

        Self {
            metadata,
            ..Self::parse(markdown)
        }
    }
}

impl Recipe {
    pub fn contents(&self) -> &str {
        &self.contents
    }

    pub fn from_reader(mut r: impl Read) -> io::Result<Self> {
        let mut input = String::new();
        r.read_to_string(&mut input)?;
        Ok(Self::parse(input))
    }

    pub fn load_all(path: impl AsRef<Path>) -> impl Iterator<Item = (PathBuf, Self)> {
        walkdir::WalkDir::new(path)
            .follow_links(false)
            .same_file_system(true)
            .into_iter()
            .filter_entry(|entry| !is_hidden(entry))
            .filter_map(|res| {
                res.ok().and_then(|entry| {
                    let path = entry.path();
                    let file_name = entry.file_name().to_string_lossy();
                    if file_name.starts_with('_') || path.is_dir() {
                        None
                    } else {
                        std::fs::File::open(path)
                            .and_then(|f| Self::from_reader(f).map(|r| (PathBuf::from(path), r)))
                            .ok()
                    }
                })
            })
    }

    fn as_html(&self) -> scraper::Html {
        scraper::Html::parse_fragment(self.contents())
    }

    pub fn description(&self) -> String {
        let sel_p = scraper::Selector::parse("p").unwrap();

        self.as_html()
            .select(&sel_p)
            .map(|p| p.text().collect::<Vec<_>>().join("\n"))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn list_field(&self, selector: impl AsRef<str>) -> Option<String> {
        let html = self.as_html();
        if let Some(map) = parse_sectioned_list(&html, selector.as_ref()) {
            Some(
                map.iter()
                    .map(|(section, list)| format!("{section}\n{}", list.join("\n")))
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        } else {
            parse_unified_list(&html, selector.as_ref()).map(|list| list.join("\n"))
        }
    }

    pub fn category(&self) -> Option<&Category> {
        self.metadata().map(|md| md.category())
    }

    pub fn directions(&self) -> Option<String> {
        self.list_field("directions")
    }

    pub fn ingredients(&self) -> Option<String> {
        self.list_field("ingredients")
    }

    pub fn metadata(&self) -> Option<&MetaData> {
        self.metadata.as_ref()
    }

    pub fn parse(s: impl AsRef<str>) -> Self {
        let input_str = s.as_ref();
        let (metadata, contents) = if let Ok(doc) = YamlFrontMatter::parse(input_str) {
            (Some(doc.metadata), doc.content)
        } else {
            (None, String::from(input_str))
        };

        Self {
            metadata,
            contents: markdown::Parser::default().parse(contents),
        }
    }

    pub fn sources(&self) -> Vec<Source> {
        self.metadata()
            .map(|md| md.sources().clone())
            .unwrap_or_default()
    }

    pub fn tags(&self) -> BTreeSet<String> {
        self.metadata()
            .map(|md| md.tags().clone())
            .unwrap_or_default()
    }

    pub fn title(&self) -> Option<&str> {
        self.metadata().map(|md| md.title())
    }
}

impl IntoResponse for Recipe {
    fn into_response(self) -> Response {
        ([(header::CONTENT_TYPE, "text/html")], self.contents).into_response()
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.') || s.starts_with('_'))
        .unwrap_or(false)
}

fn parse_sectioned_list(
    html: &scraper::Html,
    class: impl AsRef<str>,
) -> Option<BTreeMap<String, Vec<String>>> {
    let class = class.as_ref();
    let sel_li = scraper::Selector::parse("li").unwrap();
    let sel_sections = scraper::Selector::parse(&format!("h3.{class} + ul")).unwrap();
    let sections = html.select(&sel_sections);
    (sections.clone().count() != 0).then(|| {
        sections
            .map(|list| {
                let name = list
                    .prev_siblings()
                    .nth(1)
                    .unwrap()
                    .value()
                    .as_element()
                    .unwrap()
                    .attr("id")
                    .map(String::from)
                    .unwrap();

                let items = list
                    .select(&sel_li)
                    .map(|i| i.text().collect::<Vec<_>>().join(""))
                    .collect::<Vec<_>>();

                (name, items)
            })
            .collect::<BTreeMap<_, _>>()
    })
}

fn parse_unified_list(html: &scraper::Html, id: impl AsRef<str>) -> Option<Vec<String>> {
    let id = id.as_ref();
    let sel_li = scraper::Selector::parse("li").unwrap();
    let sel_unified = scraper::Selector::parse(&format!("h2#{id} + ul")).unwrap();
    html.select(&sel_unified).next().map(|list| {
        list.select(&sel_li)
            .map(|item| {
                item.text()
                    .filter_map(|txt| (txt.split_whitespace().count() != 0).then_some(txt.trim()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect::<Vec<_>>()
    })
}
