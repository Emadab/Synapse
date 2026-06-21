//! # synapse-search
//!
//! Tantivy-backed full-text + faceted search over notes. The query language is
//! Anki-flavoured: bare words match note text (AND by default), and `tag:`,
//! `deck:` and `note:` filter by facet (e.g. `tag:verb deck:Spanish hola`).
//! The index lives in RAM and is rebuilt from `NoteIndexRow`s; the application
//! keeps it in sync by rebuilding on the relevant `DomainEvent`s. SQLite stays
//! the transactional source of truth.
//!
//! `is:` selectors (due/new/suspended) depend on live scheduling state and are
//! intersected at the storage layer — not in the index — so they are not
//! handled here yet.

use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::NoteIndexRow;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, TEXT};
use tantivy::{doc, Index, IndexReader, TantivyDocument};

fn err(e: tantivy::TantivyError) -> CoreError {
    CoreError::Other(Box::new(e))
}

/// In-memory note search index.
pub struct NoteIndex {
    index: Index,
    reader: IndexReader,
    parser: QueryParser,
    note_id: Field,
    text: Field,
    tag: Field,
    deck: Field,
    notetype: Field,
}

impl NoteIndex {
    pub fn new() -> CoreResult<Self> {
        let mut builder = Schema::builder();
        let note_id = builder.add_i64_field("note_id", STORED);
        let text = builder.add_text_field("text", TEXT);
        let tag = builder.add_text_field("tag", TEXT);
        let deck = builder.add_text_field("deck", TEXT);
        let notetype = builder.add_text_field("note", TEXT);
        let schema = builder.build();

        let index = Index::create_in_ram(schema);
        let reader = index.reader().map_err(err)?;
        let mut parser = QueryParser::for_index(&index, vec![text]);
        // Space-separated terms are ANDed, matching Anki.
        parser.set_conjunction_by_default();

        Ok(Self {
            index,
            reader,
            parser,
            note_id,
            text,
            tag,
            deck,
            notetype,
        })
    }

    /// Replace the entire index contents with `rows`.
    pub fn rebuild(&self, rows: &[NoteIndexRow]) -> CoreResult<()> {
        let mut writer = self.index.writer(15_000_000).map_err(err)?;
        writer.delete_all_documents().map_err(err)?;
        for row in rows {
            writer
                .add_document(doc!(
                    self.note_id => row.note_id,
                    self.text => row.text.clone(),
                    self.tag => row.tags.clone(),
                    self.deck => row.deck.clone(),
                    self.notetype => row.notetype.clone(),
                ))
                .map_err(err)?;
        }
        writer.commit().map_err(err)?;
        self.reader.reload().map_err(err)?;
        Ok(())
    }

    /// Note ids matching `query`, best-match first, capped at `limit`. Invalid
    /// query syntax degrades to a plain text search; an unparseable query
    /// yields no results rather than an error.
    pub fn search(&self, query: &str, limit: usize) -> CoreResult<Vec<i64>> {
        let parsed = self
            .parser
            .parse_query(query)
            .or_else(|_| self.parser.parse_query(&sanitize(query)));
        let Ok(parsed) = parsed else {
            return Ok(vec![]);
        };

        let searcher = self.reader.searcher();
        let hits = searcher
            .search(&parsed, &TopDocs::with_limit(limit))
            .map_err(err)?;

        let mut ids = Vec::with_capacity(hits.len());
        for (_score, address) in hits {
            let doc: TantivyDocument = searcher.doc(address).map_err(err)?;
            if let Some(id) = doc.get_first(self.note_id).and_then(|v| v.as_i64()) {
                ids.push(id);
            }
        }
        Ok(ids)
    }
}

/// Strip query operators so a malformed query can still match as plain text.
fn sanitize(query: &str) -> String {
    query.replace([':', '"', '(', ')', '+', '-', '*'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, text: &str, tags: &str, deck: &str) -> NoteIndexRow {
        NoteIndexRow {
            note_id: id,
            text: text.into(),
            tags: tags.into(),
            deck: deck.into(),
            notetype: "Basic".into(),
        }
    }

    fn index() -> NoteIndex {
        let index = NoteIndex::new().unwrap();
        index
            .rebuild(&[
                row(1, "hola means hello", "spanish greeting", "Spanish"),
                row(2, "bonjour means hello", "french greeting", "French"),
            ])
            .unwrap();
        index
    }

    #[test]
    fn full_text_search() {
        let index = index();
        assert_eq!(index.search("hola", 10).unwrap(), vec![1]);
        // "hello" is in both notes.
        let both = index.search("hello", 10).unwrap();
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn faceted_search() {
        let index = index();
        assert_eq!(index.search("tag:french", 10).unwrap(), vec![2]);
        assert_eq!(index.search("deck:Spanish", 10).unwrap(), vec![1]);
        // Facet + text, ANDed.
        assert_eq!(index.search("tag:spanish hola", 10).unwrap(), vec![1]);
    }

    #[test]
    fn unsupported_syntax_degrades_gracefully() {
        let index = index();
        // `is:due` isn't a known field; should not error.
        assert!(index.search("is:due hola", 10).is_ok());
    }
}
