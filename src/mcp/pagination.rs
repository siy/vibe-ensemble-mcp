use super::{
    tools::extract_optional_param,
    types::PaginationCursor,
};
use crate::error::{AppError, Result};
use serde_json::Value;

/// Extract pagination cursor from tool arguments
pub fn extract_cursor(args: &Option<Value>) -> Result<PaginationCursor> {
    let cursor_str: Option<String> = extract_optional_param(args, "cursor")?;
    PaginationCursor::from_cursor_string(cursor_str)
        .map_err(AppError::BadRequest)
}

/// Optional helper to paginate a vector with the extracted cursor
/// This is useful for in-memory pagination after database queries
pub fn paginate_vec<T: Clone>(items: Vec<T>, cursor: PaginationCursor) -> super::types::PaginationResult<T> {
    cursor.paginate(items)
}