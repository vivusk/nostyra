// The desktop schema, unchanged: every `crates/storage-db/migrations/NNN_*.sql`
// file as text, in file-name order (= rusqlite_migration's order; `user_version`
// counts applied files on both editions).
const files = import.meta.glob("../../../../../../crates/storage-db/migrations/*.sql", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** Migration SQL texts, oldest first. */
export const MIGRATIONS: readonly string[] = Object.keys(files)
  .sort()
  .map((path) => files[path]);
