-- This file should undo anything in `up.sql`
ALTER TABLE stash_records DROP chunk_id;

DROP INDEX IF EXISTS stash_records_chunk_id_idx;
