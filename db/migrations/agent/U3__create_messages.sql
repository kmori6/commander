ALTER TABLE tasks
  DROP CONSTRAINT IF EXISTS tasks_source_message_id_fkey;

DROP TABLE IF EXISTS messages;
