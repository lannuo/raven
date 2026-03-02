mod fallible_options;
mod language_model;
pub mod merge_from;

pub use fallible_options::*;
pub use language_model::*;
pub use merge_from::MergeFrom as MergeFromTrait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseStatus {
    /// Settings were parsed successfully
    Success,
    /// Settings failed to parse
    Failed { error: String },
}
