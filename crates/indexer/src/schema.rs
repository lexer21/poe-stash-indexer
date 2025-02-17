table! {
    stash_records (created_at) {
        created_at -> Timestamp,
        change_id -> Text,
        next_change_id -> Text,
        stash_id -> Text,
        stash_type -> Text,
        items -> Jsonb,
        public -> Bool,
        account_name -> Nullable<Text>,
        last_character_name -> Nullable<Text>,
        stash_name -> Nullable<Text>,
        league -> Nullable<Text>,
        chunk_id -> Nullable<Int8>,
    }
}
