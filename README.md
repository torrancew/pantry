# Pantry

*The lightweight, open-source recipe manager*

Pantry is a small webapp which provides a simple recipe book with search capabilities and a minimal UI.

## Background

## Features

Pantry currently supports the following features:
  - Browser Screen Wake API (when served via HTTPS)
  - Full-text search with basic faceting/filtering (currently powered by Xapian)
  - Simple data format
  - Out-of-band editing

The following are explicitly not in-scope for Pantry at this time:
  - Recipe scaling
  - Unit Conversions
  - Logins/User Management
  - Data sync management (sync the files out-of-band -- we use Syncthing)
  - In-band editing (use the Markdown editor of your choice -- we like Obsidian)

## Recipe Support

Recipes are expected to be in a simple Markdown format, with optional YAML frontmatter.

The following should be expressed via YAML:
  - **title** (string)
  - **category** (string)
  - sources (array of objects with `name` and `url` sub-fields)
  - tags (array of strings)

While the frontmatter is optional, the bold items are **required** if frontmatter is present.

The markdown has the following expectations:
  - The first `<p>` tag represents the description
  - `<h2>` tags are used to denote the primary sections (currently: `Ingredients` and `Directions`)
    - `Directions` has several aliases: `Steps`, `Instructions`
  - `<h3>` tags can be used to break primary sections down into sub-sections
  - Ingredients and individual steps are represented as `<li>` elements within a <ul> 

When indexing, Pantry will use this format to support field-based search for:
  - `category`
  - `description`
  - `direction`
  - `ingredient`
  - `source`
  - `site`
  - `title`
  - `tag`

## Architecture

Pantry was written to replace a Trello board my family has used to curate
Recipes for many years. The format of our data in Trello heavily guided the
architecture of Pantry.

Pantry is built as on async Rust, and uses `smol` as its async runtime. `axum`
provides HTTP server functionality, and `xapian` (via `xapian-rs` FFI bindings)
is used for full-text search. Filesystem notifications are used to trigger
reloading the data, and recipe markdown is rendered almost directly for display
(some minor annotations are added to simplify locating sections for parsing and
styling). Since Xapian does not natively support the Async rust model,
a dedicated thread is spawned, and bounded channels are used to communicate with it.

## Acknowledgements
