use std::{
    io::{self, prelude::Write},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct HeadingTagData {
    last_level: u8,
    class_stack: Vec<String>,
}

#[derive(Clone, Default)]
struct HeadingTagger(Arc<Mutex<HeadingTagData>>);

impl comrak::adapters::HeadingAdapter for HeadingTagger {
    fn enter(
        &self,
        output: &mut dyn Write,
        heading: &comrak::adapters::HeadingMeta,
        _source_pos: Option<comrak::nodes::Sourcepos>,
    ) -> io::Result<()> {
        let id = slug::slugify(&heading.content);
        let mut inner = self.0.lock().unwrap();

        if heading.level <= inner.last_level {
            for _ in 0..=(inner.last_level - heading.level) {
                inner.class_stack.pop();
            }
        }

        inner.last_level = heading.level;
        let class_attr = inner.class_stack.join(" ");

        write!(
            output,
            r#"<h{} id="{id}" class="{class_attr}">"#,
            heading.level
        )?;

        inner.class_stack.push(id);
        Ok(())
    }

    fn exit(
        &self,
        output: &mut dyn Write,
        heading: &comrak::adapters::HeadingMeta,
    ) -> io::Result<()> {
        write!(output, "</h{}>", heading.level)
    }
}

pub struct Parser {
    options: comrak::Options<'static>,
    tagger: HeadingTagger,
}

#[allow(clippy::arc_with_non_send_sync)]
impl Default for Parser {
    fn default() -> Self {
        let mut options = comrak::Options::default();
        let cb = Box::new(Parser::resolve_broken_link);

        options.parse = comrak::ParseOptionsBuilder::default()
            .broken_link_callback(Some(Arc::new(Mutex::new(Box::leak(cb)))))
            .build()
            .unwrap();

        Self {
            options,
            tagger: Default::default(),
        }
    }
}

impl Parser {
    pub fn parse(&self, mkd: impl AsRef<str>) -> String {
        use comrak::{PluginsBuilder, RenderPluginsBuilder};

        let plugins = PluginsBuilder::default()
            .render(
                RenderPluginsBuilder::default()
                    .heading_adapter(Some(&self.tagger))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        comrak::markdown_to_html_with_plugins(mkd.as_ref(), &self.options, &plugins)
    }

    fn resolve_broken_link(link: comrak::BrokenLinkReference) -> Option<comrak::ResolvedReference> {
        let url = format!("/recipe/{}", slug::slugify(link.normalized));
        let title = String::from(link.original.trim_start_matches("[").trim_end_matches("]"));

        Some(comrak::ResolvedReference { url, title })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn parse_recipe() {
        /*
         use scraper::Selector;
         use super::*;

        let parser = MarkdownParser::default();

        let pancakes = parser.parse(include_str!("../../data/pancakes.md"));
        let ingredients_section = pancakes
            .select(&Selector::parse("h2#ingredients").unwrap())
            .next()
            .unwrap();

        assert_eq!(
            ingredients_section
                .text()
                .collect::<Vec<_>>()
                .join("")
                .to_lowercase(),
            "ingredients"
        );

        let crawfish = parser.parse(include_str!("../../data/crawfish.md"));
        let ingredients_subsections = crawfish
            .select(&Selector::parse("h3.ingredients").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ingredients_subsections.len(), 1)
        */
    }
}
