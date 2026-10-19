/// Changes refer to a base entry through its containing alignment position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChange {
    Unchanged,
    Added {
        content: String,
    },
    Modified {
        content: String,
    },
    Deleted,
    Moved {
        destination: String,
        content: String,
    },
}
