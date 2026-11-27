//! Shared domain models, schemas, and error types for Nodikle.

pub mod document;
pub mod error;
pub mod property;
pub mod view;

pub use document::{
    BacklinkEntry, DocumentDetail, DocumentMeta, ParsedDocument, SaveResult, TaskItem, Wikilink,
};
pub use error::{ErrorCode, IpcError, NodikleError, Result};
pub use property::{PropertyPatch, PropertySchema, PropertyType, PropertyValue};
pub use view::{
    AggregationOp, BadgeAggregation, FilterExpr, FilterOp, ReindexReport, SearchHit, SortDir,
    SortSpec, TagCount, TemplateInfo, VaultStats, ViewGroupSummary, ViewLayout, ViewQueryResult,
    ViewRow, ViewSchema, ViewSource, ViewType,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_maps_to_ipc_error_with_stable_code() {
        let err = NodikleError::OptimisticLock {
            path: "notes/a.md".into(),
            expected: "abc".into(),
            actual: "def".into(),
        };
        let ipc: IpcError = err.into();
        assert_eq!(ipc.code, ErrorCode::ErrOptimisticLock);
        assert!(ipc.message.contains("notes/a.md"));
    }

    #[test]
    fn property_value_round_trip_json() {
        let original = PropertyValue::Number(12500.0);
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: PropertyValue = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, decoded);
        assert_eq!(decoded.as_text().as_deref(), Some("12500"));
    }
}
