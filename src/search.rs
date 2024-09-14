use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
};

use smol::channel;
use thiserror::Error;
use xapian::StemStrategy;
use xapian_rs as xapian;

use crate::recipe::Recipe;

#[derive(Clone)]
pub struct AsyncIndex {
    rx: channel::Receiver<Result<Response, Error>>,
    tx: channel::Sender<Request>,
    #[allow(dead_code)]
    thread: Arc<thread::JoinHandle<()>>,
}

impl AsyncIndex {
    pub fn new(recipe_dir: impl AsRef<Path>) -> io::Result<Self> {
        let (tx, requester) = channel::bounded(1);
        let (responder, rx) = channel::bounded(1);
        let recipe_dir = PathBuf::from(recipe_dir.as_ref());

        let thread = Arc::new(
            thread::Builder::new()
                .name(String::from("xapian-rs"))
                .spawn(move || Indexer::new(recipe_dir, requester, responder).serve())?,
        );

        Ok(Self { rx, tx, thread })
    }

    pub async fn reindex(&self) -> Result<(), Error> {
        self.tx.send(Request::Reindex).await.unwrap();
        let response = self.rx.recv().await.unwrap()?;
        match response {
            Response::Reindex => Ok(()),
            _ => Err(Error::InvalidResponse(response)),
        }
    }

    pub async fn query(
        &self,
        query: &str,
        start: u32,
        size: u32,
    ) -> Result<
        (
            BTreeMap<String, usize>,
            Vec<crate::recipe::Recipe>,
            BTreeMap<String, usize>,
        ),
        Error,
    > {
        self.tx
            .send(Request::Search {
                query: String::from(query),
                start,
                size,
            })
            .await
            .unwrap();

        let response = self.rx.recv().await.unwrap()?;
        match response {
            Response::Search {
                categories,
                recipes,
                tags,
            } => Ok((categories, recipes, tags)),
            _ => Err(Error::InvalidResponse(response)),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Categorizer(Arc<RwLock<BTreeMap<String, usize>>>);

impl Categorizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn facets(&self) -> BTreeMap<String, usize> {
        self.0.read().unwrap().clone()
    }

    pub fn reset(&self) {
        self.0.write().unwrap().clear()
    }
}

impl xapian::MatchSpy for Categorizer {
    fn observe(&self, doc: &xapian::Document, _weight: f64) {
        if let Some(Ok(category)) = doc.value(1) {
            self.0
                .write()
                .unwrap()
                .entry(category)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Tagger(Arc<RwLock<BTreeMap<String, usize>>>);

impl Tagger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn facets(&self) -> BTreeMap<String, usize> {
        self.0.read().unwrap().clone()
    }

    pub fn reset(&self) {
        self.0.write().unwrap().clear()
    }
}

impl xapian::MatchSpy for Tagger {
    fn observe(&self, doc: &xapian::Document, _weight: f64) {
        if let Some(Ok(value)) = doc.value::<String>(2) {
            let tags = value.split(',').collect::<Vec<_>>();
            for tag in tags {
                self.0
                    .write()
                    .unwrap()
                    .entry(String::from(tag))
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("xapian is shutting down: {0}")]
    ChannelRx(#[from] channel::RecvError),
    #[error("xapian is shutting down: {0}")]
    ChannelTx(#[from] channel::SendError<Request>),
    #[error("invalid response: {0:?}")]
    InvalidResponse(Response),
}

pub struct Indexer {
    db: xapian::WritableDatabase,
    term_generator: xapian::TermGenerator,
    recipe_dir: PathBuf,
    searcher: Searcher,
    requests: channel::Receiver<Request>,
    responses: channel::Sender<Result<Response, Error>>,
}

impl Indexer {
    pub fn new(
        recipe_dir: impl AsRef<Path>,
        requests: channel::Receiver<Request>,
        responses: channel::Sender<Result<Response, Error>>,
    ) -> Self {
        let db = xapian::WritableDatabase::inmemory();
        let recipe_dir = PathBuf::from(recipe_dir.as_ref());
        let mut term_generator = xapian::TermGenerator::default();
        let stemmer = xapian::Stem::for_language("en");
        let searcher = Searcher::new(db.read_only(), &stemmer);

        term_generator.set_database(&db);
        term_generator.set_stemmer(&stemmer);
        term_generator.set_stemming_strategy(xapian::StemStrategy::All);

        Self {
            db,
            term_generator,
            requests,
            recipe_dir,
            searcher,
            responses,
        }
    }

    pub fn index_recipe(&mut self, id: impl AsRef<Path>, recipe: &crate::recipe::Recipe) {
        let mut doc = xapian::Document::default();
        self.term_generator.set_document(&doc);
        doc.set_data(serde_json::to_string(recipe).unwrap());

        let id = id.as_ref().to_string_lossy();
        let idterm = format!("I:{id}");
        doc.add_boolean_term(&idterm);

        if let Some(slug) = recipe.metadata().map(|md| md.slug()) {
            let slugterm = format!("Q:{slug}");
            doc.add_boolean_term(&slugterm)
        }

        if let Some(title) = recipe.metadata().map(|md| md.title()) {
            self.term_generator.index_text(title, None, "");
            self.term_generator.index_text(title, None, "S:");
            self.term_generator.increase_termpos(None);
        }

        if let Some(category) = recipe.metadata().map(|md| md.category()) {
            self.term_generator.index_text(category, None, "XC:");
            self.term_generator.increase_termpos(None);
            doc.set_value(1, category.as_ref());
        }

        if let Some(sources) = recipe.metadata().map(|md| md.sources()) {
            for src in sources {
                self.term_generator.index_text(src.name(), None, "XS:");
                self.term_generator.increase_termpos(None);

                if let Some(domain) = src.url().domain() {
                    self.term_generator.index_text(domain, None, "XD:");
                    self.term_generator.increase_termpos(None);
                }
            }
        }

        if let Some(tags) = recipe.metadata().map(|md| md.tags()) {
            let tag_value = Vec::from_iter(tags.clone()).join(",");
            doc.set_value(2, tag_value);
            for tag in tags {
                self.term_generator.index_text(tag, None, "XT:");
                self.term_generator.increase_termpos(None);
            }
        }

        self.term_generator
            .index_text(recipe.description(), None, "D:");
        self.term_generator.increase_termpos(None);

        if let Some(ingredients) = recipe.ingredients() {
            self.term_generator.index_text(ingredients, None, "XI:");
            self.term_generator.increase_termpos(None);
        }

        if let Some(directions) = recipe.directions() {
            self.term_generator.index_text(directions, None, "XP:");
            self.term_generator.increase_termpos(None);
        }

        self.db.replace_document_by_term(&idterm, doc);
    }

    #[allow(dead_code, unused_variables)]
    fn remove_recipe(&mut self, path: impl AsRef<Path>) {
        let id = path.as_ref().to_string_lossy();
        let idterm = format!("I:{id}");
        //self.db.delete_document_by_term(idterm)
        todo!()
    }

    fn handle_request(&mut self, req: &Request) -> Result<Response, Error> {
        use Request::*;
        let recipe_dir = self.recipe_dir.clone();
        match req {
            &Reindex => {
                for (path, recipe) in Recipe::load_all(&recipe_dir) {
                    self.index_recipe(path, &recipe);
                }
                Ok(Response::Reindex)
            }
            Search { query, size, start } => {
                let mset = self.searcher.search(query, *start, *size);
                Ok(Response::Search {
                    categories: self.searcher.categories().collect(),
                    recipes: mset
                        .matches()
                        .map(|m| {
                            let doc = m.document();
                            serde_json::from_slice(&doc.data()).unwrap()
                        })
                        .collect(),
                    tags: self.searcher.tags().collect(),
                })
            }
        }
    }

    pub fn serve(&mut self) {
        while let Ok(req) = self.requests.recv_blocking() {
            let response = self.handle_request(&req);
            if self.responses.send_blocking(response).is_err() {
                break;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Request {
    Reindex,
    Search {
        query: String,
        size: u32,
        start: u32,
    },
}

#[derive(Clone, Debug)]
pub enum Response {
    Reindex,
    Search {
        categories: BTreeMap<String, usize>,
        recipes: Vec<crate::recipe::Recipe>,
        tags: BTreeMap<String, usize>,
    },
}

pub struct Searcher {
    db: xapian::Database,
    enquire: xapian::Enquire,
    query_parser: xapian::QueryParser,
    categorizer: Categorizer,
    tagger: Tagger,
}

impl Searcher {
    pub fn new(db: xapian::Database, stemmer: &xapian::Stem) -> Self {
        let categorizer = Categorizer::new();
        let tagger = Tagger::new();
        let mut enquire = xapian::Enquire::new(&db);
        enquire.add_matchspy(&categorizer);
        enquire.add_matchspy(&tagger);

        let mut query_parser = xapian::QueryParser::default();
        query_parser.set_stemmer(stemmer);
        query_parser.set_stemming_strategy(StemStrategy::All);

        query_parser.add_prefix("desc", "D:");
        query_parser.add_prefix("description", "D:");
        query_parser.add_prefix("ingredient", "XI:");
        query_parser.add_prefix("ingredients", "XI:");
        query_parser.add_prefix("step", "XP:");
        query_parser.add_prefix("steps", "XP:");
        query_parser.add_prefix("direction", "XP:");
        query_parser.add_prefix("directions", "XP:");
        query_parser.add_prefix("name", "S:");
        query_parser.add_prefix("title", "S:");
        query_parser.add_prefix("source", "XS:");
        query_parser.add_prefix("category", "XC:");
        query_parser.add_prefix("tag", "XT:");
        query_parser.add_boolean_prefix::<_, &str>("slug", "Q:", None);
        query_parser.add_boolean_prefix::<_, &str>("site", "XD:", None);

        Searcher {
            db,
            categorizer,
            tagger,
            enquire,
            query_parser,
        }
    }

    pub fn categories(&self) -> impl Iterator<Item = (String, usize)> {
        self.categorizer.facets().into_iter()
    }

    pub fn tags(&self) -> impl Iterator<Item = (String, usize)> {
        self.tagger.facets().into_iter()
    }

    fn search(&mut self, query: impl AsRef<str>, start: u32, size: u32) -> xapian::MSet {
        self.categorizer.reset();
        self.tagger.reset();
        let query = self.query_parser.parse_query(query, None, "");
        self.enquire.set_query(query, None);
        self.enquire
            .mset(start, size, self.db.doc_count(), None, None)
    }
}
