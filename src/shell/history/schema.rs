use super::diesel::prelude::*;

table! {
    history (timestamp) {
        timestamp -> Timestamp,
        command -> VarChar,
    }
}
