#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use yamldb::{QueryOp, Record, YamlDb};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("yamldb-{}-{}", std::process::id(), name))
    }

    #[test]
    fn test_create_and_read() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice").set("age", 30);
        db.create(record).unwrap();

        let record = db.read("user1").unwrap();
        assert_eq!(record.get_str("name"), Some("Alice"));
        assert_eq!(record.get_i64("age"), Some(30));
    }

    #[test]
    fn test_update_field() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice");
        db.create(record).unwrap();

        db.update_field(
            "user1",
            "name",
            serde_yaml::Value::String("Bob".to_string()),
        )
        .unwrap();
        let record = db.read("user1").unwrap();
        assert_eq!(record.get_str("name"), Some("Bob"));
    }

    #[test]
    fn test_delete() {
        let mut db = YamlDb::memory();
        let record = Record::new("user1");
        db.create(record).unwrap();
        db.delete("user1").unwrap();
        assert!(!db.exists("user1"));
    }

    #[test]
    fn test_query_eq() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("city", "Beijing");
        db.create(record).unwrap();

        let result = db.query(&QueryOp::eq("city", "Beijing"));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_query_and() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("city", "Beijing").set("age", 30);
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("city", "Beijing").set("age", 25);
        db.create(r2).unwrap();

        let result = db.query(&QueryOp::and(vec![
            QueryOp::eq("city", "Beijing"),
            QueryOp::gte("age", serde_yaml::Value::Number(28.into())),
        ]));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_upsert() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice");
        db.upsert(record).unwrap();

        let mut record = Record::new("user1");
        record.set("name", "Bob");
        db.upsert(record).unwrap();

        let record = db.read("user1").unwrap();
        assert_eq!(record.get_str("name"), Some("Bob"));
    }

    #[test]
    fn test_count_and_exists() {
        let mut db = YamlDb::memory();
        let record = Record::new("user1");
        db.create(record).unwrap();

        assert_eq!(db.count(), 1);
        assert!(db.exists("user1"));
        assert!(!db.exists("user2"));
    }

    #[test]
    fn test_query_ne() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("city", "Beijing");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("city", "Shanghai");
        db.create(r2).unwrap();

        let result = db.query(&QueryOp::ne("city", "Beijing"));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_query_contains() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice Smith");
        db.create(record).unwrap();

        let result = db.query(&QueryOp::contains("name", "Alice"));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_query_starts_with() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice Smith");
        db.create(record).unwrap();

        let result = db.query(&QueryOp::starts_with("name", "Alice"));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_query_ends_with() {
        let mut db = YamlDb::memory();
        let mut record = Record::new("user1");
        record.set("name", "Alice Smith");
        db.create(record).unwrap();

        let result = db.query(&QueryOp::ends_with("name", "Smith"));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_query_or() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("city", "Beijing");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("city", "Shanghai");
        db.create(r2).unwrap();

        let result = db.query(&QueryOp::or(vec![
            QueryOp::eq("city", "Beijing"),
            QueryOp::eq("city", "Shanghai"),
        ]));
        assert_eq!(result.count(), 2);
    }

    #[test]
    fn test_query_not() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("city", "Beijing");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("city", "Shanghai");
        db.create(r2).unwrap();

        let result = db.query(&QueryOp::negate(QueryOp::eq("city", "Beijing")));
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_search() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("name", "Alice Smith");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("name", "Bob Jones");
        db.create(r2).unwrap();

        let result = db.search("name", "alice");
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_search_all() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("name", "Alice");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("name", "Bob");
        db.create(r2).unwrap();

        let result = db.search_all("alice");
        assert_eq!(result.count(), 1);
    }

    #[test]
    fn test_stats() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("name", "Alice").set("age", 30);
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("name", "Bob").set("city", "Beijing");
        db.create(r2).unwrap();

        let stats = db.stats();
        assert_eq!(stats.total_records, 2);
        assert!(stats.unique_keys.contains(&"name".to_string()));
        assert!(stats.unique_keys.contains(&"age".to_string()));
        assert!(stats.unique_keys.contains(&"city".to_string()));
    }

    #[test]
    fn test_read_many() {
        let mut db = YamlDb::memory();
        let mut r1 = Record::new("user1");
        r1.set("name", "Alice");
        db.create(r1).unwrap();

        let mut r2 = Record::new("user2");
        r2.set("name", "Bob");
        db.create(r2).unwrap();

        let records = db.read_many(&["user1", "user3"]);
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_delete_many() {
        let mut db = YamlDb::memory();
        let r1 = Record::new("user1");
        db.create(r1).unwrap();

        let r2 = Record::new("user2");
        db.create(r2).unwrap();

        let count = db.delete_many(&["user1", "user2"]).unwrap();
        assert_eq!(count, 2);
        assert_eq!(db.count(), 0);
    }

    #[test]
    fn test_query_result_page() {
        let mut db = YamlDb::memory();
        for i in 0..10 {
            let mut record = Record::new(format!("user{}", i));
            record.set("index", i);
            db.create(record).unwrap();
        }

        let result = db.query(&QueryOp::and(vec![]));
        let page = result.page(2, 3);
        assert_eq!(page.len(), 3);
    }

    #[test]
    fn test_query_result_page_zero_is_empty() {
        let mut db = YamlDb::memory();
        let record = Record::new("user1");
        db.create(record).unwrap();

        let result = db.query(&QueryOp::and(vec![]));
        assert!(result.page(0, 10).is_empty());
        assert!(result.page(1, 0).is_empty());
    }

    #[test]
    fn test_read_all_is_sorted_by_id() {
        let mut db = YamlDb::memory();
        db.create(Record::new("user2")).unwrap();
        db.create(Record::new("user1")).unwrap();

        let ids: Vec<&str> = db
            .read_all()
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(ids, vec!["user1", "user2"]);
    }

    #[test]
    fn test_memory_export_yaml_writes_records() {
        let path = temp_path("memory-export.yaml");
        let mut db = YamlDb::memory();
        db.create(Record::new("user2")).unwrap();
        db.create(Record::new("user1")).unwrap();

        db.export_yaml(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("id: user1"));
        assert!(content.contains("id: user2"));
        assert!(content.find("id: user1") < content.find("id: user2"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_yaml_file_with_nested_values() {
        let path = temp_path("load-yq.yaml");
        std::fs::write(
            &path,
            r#"- id: user1
  name: "Alice: Smith"
  active: true
  tags:
    - admin
    - ops
"#,
        )
        .unwrap();

        let mut db = YamlDb::new(&path);
        db.load().unwrap();

        let record = db.read("user1").unwrap();
        assert_eq!(record.get_str("name"), Some("Alice: Smith"));
        assert_eq!(record.get_bool("active"), Some(true));
        assert_eq!(
            record
                .get("tags")
                .and_then(|v| v.as_sequence())
                .unwrap()
                .len(),
            2
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_record_merge() {
        let mut r1 = Record::new("user1");
        r1.set("name", "Alice").set("age", 30);

        let mut r2 = Record::new("user1");
        r2.set("age", 31).set("city", "Beijing");

        r1.merge(&r2);
        assert_eq!(r1.get_str("name"), Some("Alice"));
        assert_eq!(r1.get_i64("age"), Some(31));
        assert_eq!(r1.get_str("city"), Some("Beijing"));
    }

    #[test]
    fn test_record_to_json() {
        let mut record = Record::new("user1");
        record.set("name", "Alice").set("age", 30);

        let json = record.to_json().unwrap();
        assert!(json.contains("\"id\": \"user1\""));
        assert!(json.contains("\"name\": \"Alice\""));
    }

    #[test]
    fn test_clear() {
        let mut db = YamlDb::memory();
        let r1 = Record::new("user1");
        db.create(r1).unwrap();

        db.clear().unwrap();
        assert_eq!(db.count(), 0);
    }

    #[test]
    fn test_duplicate_key_error() {
        let mut db = YamlDb::memory();
        let r1 = Record::new("user1");
        db.create(r1).unwrap();

        let r2 = Record::new("user1");
        assert!(db.create(r2).is_err());
    }

    #[test]
    fn test_failed_save_does_not_change_memory_state() {
        let parent = temp_path("not-a-directory");
        std::fs::write(&parent, "file").unwrap();
        let mut db = YamlDb::new(parent.join("data.yaml"));

        assert!(db.create(Record::new("user1")).is_err());
        assert!(!db.exists("user1"));
        assert_eq!(db.count(), 0);

        let _ = std::fs::remove_file(parent);
    }

    #[test]
    fn test_not_found_error() {
        let db = YamlDb::memory();
        assert!(db.read("nonexistent").is_err());
    }
}
