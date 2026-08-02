use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

pub mod odbc;

#[derive(Error, Debug)]
pub enum YamlDbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("yq error: {0}")]
    Yq(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Record not found: {0}")]
    NotFound(String),
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(flatten)]
    pub data: HashMap<String, serde_yaml::Value>,
}

impl Record {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            data: HashMap::new(),
        }
    }

    pub fn set(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_yaml::Value>,
    ) -> &mut Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.data.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.data.get(key).and_then(|v| v.as_i64())
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_f64())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.data.get(key).and_then(|v| v.as_bool())
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    pub fn keys(&self) -> Vec<&String> {
        self.data.keys().collect()
    }

    pub fn merge(&mut self, other: &Record) {
        for (key, value) in &other.data {
            self.data.insert(key.clone(), value.clone());
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut map = self.data.clone();
        map.insert("id".to_string(), serde_yaml::Value::String(self.id.clone()));
        serde_json::to_string_pretty(&map)
    }
}

#[derive(Debug, Clone)]
pub enum QueryOp {
    Eq(String, serde_yaml::Value),
    Ne(String, serde_yaml::Value),
    Gt(String, serde_yaml::Value),
    Lt(String, serde_yaml::Value),
    Gte(String, serde_yaml::Value),
    Lte(String, serde_yaml::Value),
    Contains(String, String),
    StartsWith(String, String),
    EndsWith(String, String),
    And(Vec<QueryOp>),
    Or(Vec<QueryOp>),
    Not(Box<QueryOp>),
}

impl QueryOp {
    pub fn eq(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Eq(key.into(), value.into())
    }

    pub fn ne(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Ne(key.into(), value.into())
    }

    pub fn gt(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Gt(key.into(), value.into())
    }

    pub fn lt(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Lt(key.into(), value.into())
    }

    pub fn gte(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Gte(key.into(), value.into())
    }

    pub fn lte(key: impl Into<String>, value: impl Into<serde_yaml::Value>) -> Self {
        Self::Lte(key.into(), value.into())
    }

    pub fn contains(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Contains(key.into(), value.into())
    }

    pub fn starts_with(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::StartsWith(key.into(), value.into())
    }

    pub fn ends_with(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::EndsWith(key.into(), value.into())
    }

    pub fn and(ops: Vec<QueryOp>) -> Self {
        Self::And(ops)
    }

    pub fn or(ops: Vec<QueryOp>) -> Self {
        Self::Or(ops)
    }

    pub fn negate(op: QueryOp) -> Self {
        Self::Not(Box::new(op))
    }

    pub fn matches(&self, record: &Record) -> bool {
        match self {
            QueryOp::Eq(key, value) => record.data.get(key).map(|v| v == value).unwrap_or(false),
            QueryOp::Ne(key, value) => record.data.get(key).map(|v| v != value).unwrap_or(true),
            QueryOp::Gt(key, value) => {
                compare_values(record.data.get(key), value, std::cmp::Ordering::Greater)
            }
            QueryOp::Lt(key, value) => {
                compare_values(record.data.get(key), value, std::cmp::Ordering::Less)
            }
            QueryOp::Gte(key, value) => {
                compare_values(record.data.get(key), value, std::cmp::Ordering::Greater)
                    || record.data.get(key).map(|v| v == value).unwrap_or(false)
            }
            QueryOp::Lte(key, value) => {
                compare_values(record.data.get(key), value, std::cmp::Ordering::Less)
                    || record.data.get(key).map(|v| v == value).unwrap_or(false)
            }
            QueryOp::Contains(key, substr) => record
                .data
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.contains(substr.as_str()))
                .unwrap_or(false),
            QueryOp::StartsWith(key, prefix) => record
                .data
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.starts_with(prefix.as_str()))
                .unwrap_or(false),
            QueryOp::EndsWith(key, suffix) => record
                .data
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.ends_with(suffix.as_str()))
                .unwrap_or(false),
            QueryOp::And(ops) => ops.iter().all(|op| op.matches(record)),
            QueryOp::Or(ops) => ops.iter().any(|op| op.matches(record)),
            QueryOp::Not(op) => !op.matches(record),
        }
    }
}

fn compare_values(
    record_val: Option<&serde_yaml::Value>,
    query_val: &serde_yaml::Value,
    ordering: std::cmp::Ordering,
) -> bool {
    match (record_val, query_val) {
        (Some(serde_yaml::Value::Number(n1)), serde_yaml::Value::Number(n2)) => {
            if let (Some(a), Some(b)) = (n1.as_i64(), n2.as_i64()) {
                a.cmp(&b) == ordering
            } else if let (Some(a), Some(b)) = (n1.as_f64(), n2.as_f64()) {
                a.partial_cmp(&b).map(|o| o == ordering).unwrap_or(false)
            } else {
                false
            }
        }
        (Some(serde_yaml::Value::String(s1)), serde_yaml::Value::String(s2)) => {
            s1.cmp(s2) == ordering
        }
        _ => false,
    }
}

#[derive(Debug)]
pub struct QueryResult<'a> {
    records: Vec<&'a Record>,
}

impl<'a> QueryResult<'a> {
    pub fn first(&self) -> Option<&'a Record> {
        self.records.first().copied()
    }

    pub fn last(&self) -> Option<&'a Record> {
        self.records.last().copied()
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn sort_by_key(&mut self, key: &str, ascending: bool) {
        self.records.sort_by(|a, b| {
            let cmp = a.data.get(key).partial_cmp(&b.data.get(key));
            if ascending {
                cmp.unwrap_or(std::cmp::Ordering::Equal)
            } else {
                cmp.unwrap_or(std::cmp::Ordering::Equal).reverse()
            }
        });
    }

    pub fn limit(&self, n: usize) -> Vec<&'a Record> {
        self.records.iter().take(n).copied().collect()
    }

    pub fn skip(&self, n: usize) -> Vec<&'a Record> {
        self.records.iter().skip(n).copied().collect()
    }

    pub fn page(&self, page: usize, page_size: usize) -> Vec<&'a Record> {
        if page == 0 || page_size == 0 {
            return Vec::new();
        }
        let start = (page - 1) * page_size;
        self.records
            .iter()
            .skip(start)
            .take(page_size)
            .copied()
            .collect()
    }

    pub fn to_vec(&self) -> Vec<&'a Record> {
        self.records.clone()
    }

    pub fn iter(&self) -> impl Iterator<Item = &&'a Record> {
        self.records.iter()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.records.iter().map(|r| r.id.as_str()).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DbStats {
    pub total_records: usize,
    pub total_keys: usize,
    pub unique_keys: Vec<String>,
    pub file_size: Option<u64>,
}

pub struct YamlDb {
    path: Option<PathBuf>,
    records: HashMap<String, Record>,
}

impl YamlDb {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            records: HashMap::new(),
        }
    }

    pub fn memory() -> Self {
        Self {
            path: None,
            records: HashMap::new(),
        }
    }

    pub fn load(&mut self) -> Result<(), YamlDbError> {
        let path = match &self.path {
            Some(p) => p,
            None => return Ok(()),
        };
        if !path.exists() {
            self.records = HashMap::new();
            return Ok(());
        }
        if fs::read_to_string(path)?.trim().is_empty() {
            self.records = HashMap::new();
            return Ok(());
        }
        let records = read_records_with_yq(path)?;
        self.records = records.into_iter().map(|r| (r.id.clone(), r)).collect();
        Ok(())
    }

    pub fn save(&self) -> Result<(), YamlDbError> {
        let path = match &self.path {
            Some(p) => p,
            None => return Ok(()),
        };
        self.write_yaml(path)?;
        Ok(())
    }

    pub fn create(&mut self, record: Record) -> Result<(), YamlDbError> {
        if self.records.contains_key(&record.id) {
            return Err(YamlDbError::DuplicateKey(record.id));
        }
        let mut records = self.records.clone();
        records.insert(record.id.clone(), record);
        self.commit_records(records)
    }

    pub fn insert(&mut self, record: Record) -> Result<(), YamlDbError> {
        let mut records = self.records.clone();
        records.insert(record.id.clone(), record);
        self.commit_records(records)
    }

    pub fn read(&self, id: &str) -> Result<&Record, YamlDbError> {
        self.records
            .get(id)
            .ok_or_else(|| YamlDbError::NotFound(id.to_string()))
    }

    pub fn read_all(&self) -> Vec<&Record> {
        self.sorted_records()
    }

    pub fn read_many(&self, ids: &[&str]) -> Vec<&Record> {
        ids.iter().filter_map(|id| self.records.get(*id)).collect()
    }

    pub fn update(
        &mut self,
        id: &str,
        data: HashMap<String, serde_yaml::Value>,
    ) -> Result<(), YamlDbError> {
        let mut records = self.records.clone();
        let record = records
            .get_mut(id)
            .ok_or_else(|| YamlDbError::NotFound(id.to_string()))?;
        record.data = data;
        self.commit_records(records)
    }

    pub fn update_field(
        &mut self,
        id: &str,
        key: &str,
        value: serde_yaml::Value,
    ) -> Result<(), YamlDbError> {
        let mut records = self.records.clone();
        let record = records
            .get_mut(id)
            .ok_or_else(|| YamlDbError::NotFound(id.to_string()))?;
        record.data.insert(key.to_string(), value);
        self.commit_records(records)
    }

    pub fn update_many(
        &mut self,
        updates: Vec<(String, HashMap<String, serde_yaml::Value>)>,
    ) -> Result<usize, YamlDbError> {
        let mut records = self.records.clone();
        let mut count = 0;
        for (id, data) in updates {
            if let Some(record) = records.get_mut(&id) {
                record.data = data;
                count += 1;
            }
        }
        if count > 0 {
            self.commit_records(records)?;
        }
        Ok(count)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), YamlDbError> {
        let mut records = self.records.clone();
        records
            .remove(id)
            .ok_or_else(|| YamlDbError::NotFound(id.to_string()))?;
        self.commit_records(records)
    }

    pub fn delete_many(&mut self, ids: &[&str]) -> Result<usize, YamlDbError> {
        let mut records = self.records.clone();
        let mut count = 0;
        for id in ids {
            if records.remove(*id).is_some() {
                count += 1;
            }
        }
        if count > 0 {
            self.commit_records(records)?;
        }
        Ok(count)
    }

    pub fn query(&self, op: &QueryOp) -> QueryResult<'_> {
        let records: Vec<&Record> = self
            .sorted_records()
            .into_iter()
            .filter(|r| op.matches(r))
            .collect();
        QueryResult { records }
    }

    pub fn find_where<F>(&self, filter: F) -> QueryResult<'_>
    where
        F: Fn(&Record) -> bool,
    {
        let records: Vec<&Record> = self
            .sorted_records()
            .into_iter()
            .filter(|r| filter(r))
            .collect();
        QueryResult { records }
    }

    pub fn search(&self, key: &str, keyword: &str) -> QueryResult<'_> {
        let keyword_lower = keyword.to_lowercase();
        let records: Vec<&Record> = self
            .sorted_records()
            .into_iter()
            .filter(|r| {
                r.data
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase().contains(&keyword_lower))
                    .unwrap_or(false)
            })
            .collect();
        QueryResult { records }
    }

    pub fn search_all(&self, keyword: &str) -> QueryResult<'_> {
        let keyword_lower = keyword.to_lowercase();
        let records: Vec<&Record> = self
            .sorted_records()
            .into_iter()
            .filter(|r| {
                r.id.to_lowercase().contains(&keyword_lower)
                    || r.data.values().any(|v| {
                        v.as_str()
                            .map(|s| s.to_lowercase().contains(&keyword_lower))
                            .unwrap_or(false)
                    })
            })
            .collect();
        QueryResult { records }
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn exists(&self, id: &str) -> bool {
        self.records.contains_key(id)
    }

    pub fn clear(&mut self) -> Result<(), YamlDbError> {
        self.commit_records(HashMap::new())
    }

    pub fn upsert(&mut self, record: Record) -> Result<(), YamlDbError> {
        let mut records = self.records.clone();
        records.insert(record.id.clone(), record);
        self.commit_records(records)
    }

    pub fn stats(&self) -> DbStats {
        let mut all_keys = std::collections::HashSet::new();
        for record in self.records.values() {
            for key in record.data.keys() {
                all_keys.insert(key.clone());
            }
        }
        let mut unique_keys: Vec<String> = all_keys.into_iter().collect();
        unique_keys.sort();

        let file_size = self
            .path
            .as_ref()
            .and_then(|p| fs::metadata(p).ok())
            .map(|m| m.len());

        DbStats {
            total_records: self.records.len(),
            total_keys: unique_keys.len(),
            unique_keys,
            file_size,
        }
    }

    pub fn backup(&self, backup_path: &Path) -> Result<(), YamlDbError> {
        self.write_yaml(backup_path)?;
        Ok(())
    }

    pub fn import_json(&mut self, path: &Path) -> Result<usize, YamlDbError> {
        let content = fs::read_to_string(path)?;
        let items: Vec<serde_json::Value> = serde_json::from_str(&content)?;
        let mut records = self.records.clone();
        let mut count = 0;
        for item in items {
            if let Some(obj) = item.as_object() {
                let id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or(YamlDbError::InvalidQuery("Missing 'id' field".to_string()))?
                    .to_string();
                let data: HashMap<String, serde_yaml::Value> = obj
                    .iter()
                    .filter(|(k, _)| *k != "id")
                    .map(|(k, v)| {
                        let yaml_val: serde_yaml::Value =
                            serde_yaml::to_value(v).unwrap_or(serde_yaml::Value::Null);
                        (k.clone(), yaml_val)
                    })
                    .collect();
                records.insert(id.clone(), Record { id, data });
                count += 1;
            }
        }
        self.commit_records(records)?;
        Ok(count)
    }

    pub fn import_yaml(&mut self, path: &Path) -> Result<usize, YamlDbError> {
        let imported = read_records_with_yq(path)?;
        let count = imported.len();
        let mut records = self.records.clone();
        for record in imported {
            records.insert(record.id.clone(), record);
        }
        self.commit_records(records)?;
        Ok(count)
    }

    pub fn export_json(&self, path: &Path) -> Result<(), YamlDbError> {
        let records = self.sorted_records();
        let content = serde_json::to_string_pretty(&records)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn export_yaml(&self, path: &Path) -> Result<(), YamlDbError> {
        self.write_yaml(path)?;
        Ok(())
    }

    fn sorted_records(&self) -> Vec<&Record> {
        let mut records: Vec<&Record> = self.records.values().collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    fn write_yaml(&self, path: &Path) -> Result<(), YamlDbError> {
        let records = self.sorted_records();
        let content = write_records_with_yq(&records)?;
        write_file_atomically(path, content.as_bytes())?;
        Ok(())
    }

    fn commit_records(&mut self, records: HashMap<String, Record>) -> Result<(), YamlDbError> {
        if let Some(path) = &self.path {
            let mut sorted: Vec<&Record> = records.values().collect();
            sorted.sort_by(|a, b| a.id.cmp(&b.id));
            let content = write_records_with_yq(&sorted)?;
            write_file_atomically(path, content.as_bytes())?;
        }
        self.records = records;
        Ok(())
    }
}

fn read_records_with_yq(path: &Path) -> Result<Vec<Record>, YamlDbError> {
    let output = Command::new(yq_command())
        .arg("-o=json")
        .arg(".")
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(YamlDbError::Yq(command_stderr(output.stderr)));
    }

    Ok(serde_json::from_slice(&output.stdout)?)
}

fn write_records_with_yq(records: &[&Record]) -> Result<String, YamlDbError> {
    let input = serde_json::to_vec(records)?;
    let mut child = Command::new(yq_command())
        .arg("-P")
        .arg("-o=yaml")
        .arg(".")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| YamlDbError::Yq("failed to open yq stdin".to_string()))?;
        stdin.write_all(&input)?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(YamlDbError::Yq(command_stderr(output.stderr)));
    }

    String::from_utf8(output.stdout).map_err(|e| YamlDbError::Yq(e.to_string()))
}

fn yq_command() -> PathBuf {
    if let Ok(path) = std::env::var("YAMLDB_YQ")
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }

    for dir in yq_candidate_dirs() {
        let candidate = dir.join(yq_exe_name());
        if candidate.is_file() {
            return candidate;
        }
        let candidate = dir.join("bin").join(yq_exe_name());
        if candidate.is_file() {
            return candidate;
        }
    }

    PathBuf::from(yq_exe_name())
}

fn yq_candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        dirs.push(dir.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    dirs
}

fn yq_exe_name() -> &'static str {
    if cfg!(windows) { "yq.exe" } else { "yq" }
}

fn command_stderr(stderr: Vec<u8>) -> String {
    let message = String::from_utf8_lossy(&stderr).trim().to_string();
    if message.is_empty() {
        "yq command failed".to_string()
    } else {
        message
    }
}

fn write_file_atomically(path: &Path, content: &[u8]) -> Result<(), std::io::Error> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let suffix = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("yamldb");
    let tmp_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        suffix
    ));

    let result = (|| {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content)?;
        file.sync_all()?;

        replace_file(&tmp_path, path)?;

        #[cfg(unix)]
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
