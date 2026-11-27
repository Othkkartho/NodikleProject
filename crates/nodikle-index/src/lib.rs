//! SQLite + FTS5 local index, graph, search, and database query engine.

pub mod fts;
pub mod graph;
pub mod migrations;
pub mod query;
pub mod scanner;

pub use scanner::SqliteIndex;

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use nodikle_core::starter_vault_files;
    use nodikle_model::{FilterExpr, FilterOp, ViewSchema};
    use nodikle_storage::{LocalFileStore, MemoryFileStore};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn indexes_starter_vault_resolves_backlinks_and_queries_views() {
        let store = MemoryFileStore::new();
        for (path, content) in starter_vault_files() {
            store
                .write_atomic(path, content.as_bytes(), None)
                .await
                .expect("write starter file");
        }

        let idx = SqliteIndex::open_in_memory("mem://vault").expect("open index");
        let report = idx.reindex_vault(&store, false).await.expect("reindex");
        assert_eq!(report.scanned, 7);
        assert_eq!(report.updated, 7);

        // Incremental second scan skips unchanged files.
        let report2 = idx.reindex_vault(&store, false).await.expect("reindex 2");
        assert_eq!(report2.skipped, 7);
        assert_eq!(report2.updated, 0);

        // Nested tag prefix `#가계부` matches `#가계부/식비` and `#가계부/교통`.
        let finance_docs = idx
            .list_documents(None, Some("가계부"))
            .expect("list nested tag");
        assert_eq!(finance_docs.len(), 3);

        // Backlink lookup on `2026-12-kyoto.md` finds `local-first-design.md` and `kyoto-food-list.md`.
        let kyoto = idx
            .get_document_by_path("trips/2026-12-kyoto.md")
            .expect("get kyoto")
            .expect("kyoto exists");
        let backlinks = idx.get_backlinks(&kyoto.id).expect("backlinks");
        assert_eq!(backlinks.len(), 2);

        // FTS5 trigram search (3+ chars) & short fallback (1-2 chars).
        let hits_3 = idx.search_fts("교토역", None, None, 20).expect("fts3");
        assert_eq!(hits_3.len(), 1);
        let hits_2 = idx.search_fts("교토", None, None, 20).expect("fts2");
        assert!(hits_2.len() >= 2);
        let hits_1 = idx.search_fts("찜", None, None, 20).expect("fts1");
        assert!(hits_1.is_empty());

        // Expense tracker view query + aggregation.
        let expense_yaml = String::from_utf8(
            store
                .read_bytes(".nodikle/views/expenses.yaml")
                .await
                .expect("read expenses yaml"),
        )
        .expect("utf8");
        let expense_schema: ViewSchema =
            serde_yaml::from_str(&expense_yaml).expect("parse expenses yaml");
        let res = idx
            .query_view(&expense_schema, Some("monthly-table"), None, 0, 50)
            .expect("query expenses");
        assert_eq!(res.total_count, 3);
        assert_eq!(res.aggregations.get("amount").copied(), Some(74700.0));
        assert_eq!(res.groups.len(), 2);

        // Runtime filter for 2026-11-27 only.
        let rt_filter = FilterExpr::Condition {
            op: FilterOp::Eq,
            property: "date".into(),
            value: Some(json!("2026-11-27")),
            values: None,
        };
        let filtered = idx
            .query_view(
                &expense_schema,
                Some("monthly-table"),
                Some(&rt_filter),
                0,
                50,
            )
            .expect("filtered");
        assert_eq!(filtered.total_count, 1);
        assert_eq!(filtered.aggregations.get("amount").copied(), Some(11000.0));

        // Trip planner checklist progress.
        let trips_yaml = String::from_utf8(
            store
                .read_bytes(".nodikle/views/trips.yaml")
                .await
                .expect("read trips yaml"),
        )
        .expect("utf8");
        let trips_schema: ViewSchema = serde_yaml::from_str(&trips_yaml).expect("parse trips yaml");
        let trip_res = idx
            .query_view(&trips_schema, Some("trip-board"), None, 0, 50)
            .expect("query trips");
        assert_eq!(trip_res.total_count, 2);
        let kyoto_row = trip_res
            .rows
            .iter()
            .find(|r| r.rel_path == "trips/2026-12-kyoto.md")
            .expect("kyoto row");
        assert_eq!(kyoto_row.task_completed, 2);
        assert_eq!(kyoto_row.task_total, 4);
    }

    #[tokio::test]
    async fn expense_tracker_1000_rows_aggregation_and_incremental_delete() {
        let store = MemoryFileStore::new();
        let mut expected_sum = 0.0;
        for i in 0..1000 {
            let amount = ((i % 50) + 1) * 1000;
            expected_sum += f64::from(amount);
            let cat = if i % 2 == 0 { "식비" } else { "교통" };
            let md = format!(
                "---\ntype: expense\ntitle: \"항목 {i}\"\ndate: 2026-11-{:02}\namount: {amount}\ncategory: {cat}\n---\n메모 {i}\n",
                (i % 28) + 1
            );
            store
                .write_atomic(&format!("finance/item-{i:04}.md"), md.as_bytes(), None)
                .await
                .expect("write");
        }

        let idx = SqliteIndex::open_in_memory("mem://perf").expect("open");
        let r = idx.reindex_vault(&store, false).await.expect("reindex");
        assert_eq!(r.scanned, 1000);

        let schema: ViewSchema = serde_yaml::from_str(
            starter_vault_files()
                .into_iter()
                .find(|(p, _)| *p == ".nodikle/views/expenses.yaml")
                .expect("schema")
                .1,
        )
        .expect("parse");

        let t0 = Instant::now();
        let res = idx
            .query_view(&schema, Some("monthly-table"), None, 0, 100)
            .expect("query 1000");
        assert_eq!(res.total_count, 1000);
        assert_eq!(res.aggregations.get("amount").copied(), Some(expected_sum));
        assert!(t0.elapsed().as_millis() < 1000);

        // Delete one file and verify incremental reindex removes it.
        store
            .delete_entry("finance/item-0000.md")
            .await
            .expect("delete");
        let r_del = idx.reindex_vault(&store, false).await.expect("reindex del");
        assert_eq!(r_del.removed, 1);
        assert_eq!(r_del.skipped, 999);
    }
}
