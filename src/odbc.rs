#![allow(clippy::missing_safety_doc, clippy::manual_dangling_ptr)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Mutex;

use crate::{QueryOp, Record, YamlDb};

pub const SQL_SUCCESS: c_int = 0;
pub const SQL_ERROR: c_int = -1;
pub const SQL_NO_DATA: c_int = 100;

pub const SQL_HANDLE_ENV: c_int = 1;
pub const SQL_HANDLE_DBC: c_int = 2;
pub const SQL_HANDLE_STMT: c_int = 3;

pub const SQL_CHAR: c_int = 1;
pub const SQL_INTEGER: c_int = 4;
pub const SQL_VARCHAR: c_int = 12;

struct SqlDbcEntry {
    source: Option<PathBuf>,
}

struct SqlStmtEntry {
    dbc_id: usize,
    results: Vec<Record>,
    current_row: usize,
    columns: Vec<String>,
}

struct ParsedQuery {
    table: String,
    op: QueryOp,
}

lazy_static::lazy_static! {
    static ref DBC_STORE: Mutex<Vec<Option<SqlDbcEntry>>> = Mutex::new(Vec::new());
    static ref STMT_STORE: Mutex<Vec<Option<SqlStmtEntry>>> = Mutex::new(Vec::new());
}

fn slot_alloc<T>(store: &Mutex<Vec<Option<T>>>, item: T) -> usize {
    let mut v = store.lock().unwrap();
    for (i, slot) in v.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(item);
            return i;
        }
    }
    v.push(Some(item));
    v.len() - 1
}

fn parse_dsn(dsn: &str) -> Option<String> {
    for part in dsn.split(';') {
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim().to_uppercase();
            if key == "DBQ" || key == "FILE" {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn parse_query(query: &str) -> Option<ParsedQuery> {
    let q = query.trim().trim_end_matches(';').trim();
    let lower = q.to_lowercase();
    let where_idx = lower.find(" where ");
    let select_part = where_idx.map_or(q, |idx| &q[..idx]);
    let tokens: Vec<&str> = select_part.split_whitespace().collect();
    if tokens.len() != 4
        || !tokens[0].eq_ignore_ascii_case("select")
        || tokens[1] != "*"
        || !tokens[2].eq_ignore_ascii_case("from")
    {
        return None;
    }
    let table = unquote_ident(tokens[3])?;
    let op = where_idx
        .map(|idx| parse_condition(q[idx + 7..].trim()))
        .unwrap_or_else(|| Some(QueryOp::and(vec![])))?;
    Some(ParsedQuery { table, op })
}

fn unquote_ident(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('`') && trimmed.ends_with('`'))
    {
        return Some(trimmed[1..trimmed.len() - 1].to_string());
    }
    Some(trimmed.to_string())
}

fn parse_condition(cond: &str) -> Option<QueryOp> {
    let cond = cond.trim();
    let lower = cond.to_lowercase();
    if let Some(idx) = lower.find(" and ") {
        return Some(QueryOp::and(vec![
            parse_condition(&cond[..idx])?,
            parse_condition(&cond[idx + 5..])?,
        ]));
    }
    if let Some(idx) = lower.find(" or ") {
        return Some(QueryOp::or(vec![
            parse_condition(&cond[..idx])?,
            parse_condition(&cond[idx + 4..])?,
        ]));
    }
    parse_cmp(cond)
}

fn parse_cmp(s: &str) -> Option<QueryOp> {
    let s = s.trim();
    for sym in &[">=", "<=", "!=", "=", ">", "<"] {
        if let Some(idx) = s.find(*sym) {
            let key = s[..idx].trim().to_lowercase();
            let val = s[idx + sym.len()..].trim();
            let v = if val.starts_with('\'') && val.ends_with('\'') {
                serde_yaml::Value::String(val[1..val.len() - 1].to_string())
            } else if let Ok(n) = val.parse::<i64>() {
                serde_yaml::Value::Number(n.into())
            } else if let Ok(f) = val.parse::<f64>() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::String(val.to_string())
            };
            return match *sym {
                "=" => Some(QueryOp::eq(key, v)),
                "!=" => Some(QueryOp::ne(key, v)),
                ">" => Some(QueryOp::gt(key, v)),
                "<" => Some(QueryOp::lt(key, v)),
                ">=" => Some(QueryOp::gte(key, v)),
                "<=" => Some(QueryOp::lte(key, v)),
                _ => None,
            };
        }
    }
    None
}

fn val_str(rec: &Record, col: &str) -> String {
    rec.data
        .get(col)
        .map(|v| match v {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Null => "NULL".to_string(),
            _ => serde_json::to_string(v).unwrap_or_default(),
        })
        .unwrap_or_default()
}

fn result_columns(records: &[Record]) -> Vec<String> {
    let mut columns = std::collections::BTreeSet::new();
    for record in records {
        columns.extend(record.data.keys().cloned());
    }
    columns.into_iter().collect()
}

fn table_name(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}

fn resolve_table_path(source: &Path, table: &str) -> Option<PathBuf> {
    if source.is_file() {
        let source_table = table_name(source)?;
        if table.eq_ignore_ascii_case("data") || table.eq_ignore_ascii_case(&source_table) {
            return Some(source.to_path_buf());
        }
        return None;
    }

    if !source.is_dir() {
        return None;
    }

    for ext in ["yaml", "yml"] {
        let path = source.join(format!("{table}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }

    std::fs::read_dir(source).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        if is_yaml_path(&path)
            && table_name(&path)
                .map(|name| name.eq_ignore_ascii_case(table))
                .unwrap_or(false)
        {
            Some(path)
        } else {
            None
        }
    })
}

fn list_tables(source: &Path) -> Vec<String> {
    if source.is_file() {
        return vec!["data".to_string()];
    }
    if !source.is_dir() {
        return Vec::new();
    }
    let mut tables: Vec<String> = std::fs::read_dir(source)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_yaml_path(path))
        .filter_map(|path| table_name(&path))
        .collect();
    tables.sort_by_key(|name| name.to_lowercase());
    tables
}

fn matches_pattern(value: &str, pattern: Option<&str>) -> bool {
    let Some(pattern) = pattern else {
        return true;
    };
    if pattern.is_empty() || pattern == "%" {
        return true;
    }
    pattern_match(value.as_bytes(), pattern.as_bytes())
}

fn pattern_match(value: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    match pattern[0] {
        b'%' => {
            pattern_match(value, &pattern[1..])
                || (!value.is_empty() && pattern_match(&value[1..], pattern))
        }
        b'_' => !value.is_empty() && pattern_match(&value[1..], &pattern[1..]),
        ch => {
            !value.is_empty()
                && value[0].eq_ignore_ascii_case(&ch)
                && pattern_match(&value[1..], &pattern[1..])
        }
    }
}

fn source_for_statement(statement_handle: *mut c_void) -> Option<PathBuf> {
    if statement_handle.is_null() {
        return None;
    }
    let stmt_id = (statement_handle as usize).saturating_sub(1);
    let dbc_id = {
        let sv = STMT_STORE.lock().unwrap();
        sv.get(stmt_id).and_then(|e| e.as_ref())?.dbc_id
    };
    let dv = DBC_STORE.lock().unwrap();
    dv.get(dbc_id)
        .and_then(|e| e.as_ref())
        .and_then(|entry| entry.source.clone())
}

fn set_statement_results(
    statement_handle: *mut c_void,
    records: Vec<Record>,
    columns: Vec<String>,
) -> c_int {
    if statement_handle.is_null() {
        return SQL_ERROR;
    }
    let stmt_id = (statement_handle as usize).saturating_sub(1);
    let mut sv = STMT_STORE.lock().unwrap();
    if let Some(stmt) = sv.get_mut(stmt_id).and_then(|e| e.as_mut()) {
        stmt.results = records;
        stmt.current_row = 0;
        stmt.columns = columns;
        SQL_SUCCESS
    } else {
        SQL_ERROR
    }
}

unsafe fn cstr_to_str<'a>(p: *const c_char) -> Result<&'a str, ()> {
    if p.is_null() {
        return Err(());
    }
    unsafe { CStr::from_ptr(p) }.to_str().map_err(|_| ())
}

unsafe fn write_bytes(dst: *mut c_char, buf_len: c_int, src: &[u8]) {
    unsafe {
        let max = (buf_len - 1) as usize;
        let n = src.len().min(max);
        ptr::copy_nonoverlapping(src.as_ptr(), dst as *mut u8, n);
        *dst.add(n) = 0;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLAllocHandle(
    handle_type: c_int,
    input_handle: *mut c_void,
    output_handle: *mut *mut c_void,
) -> c_int {
    unsafe {
        if output_handle.is_null() {
            return SQL_ERROR;
        }
        match handle_type {
            SQL_HANDLE_ENV => {
                *output_handle = std::ptr::dangling_mut::<c_void>();
                SQL_SUCCESS
            }
            SQL_HANDLE_DBC => {
                if input_handle.is_null() {
                    return SQL_ERROR;
                }
                let id = slot_alloc(&DBC_STORE, SqlDbcEntry { source: None });
                *output_handle = (id + 1) as *mut c_void;
                SQL_SUCCESS
            }
            SQL_HANDLE_STMT => {
                if input_handle.is_null() {
                    return SQL_ERROR;
                }
                let dbc_id = (input_handle as usize).saturating_sub(1);
                let valid_dbc = DBC_STORE
                    .lock()
                    .unwrap()
                    .get(dbc_id)
                    .and_then(|entry| entry.as_ref())
                    .is_some();
                if !valid_dbc {
                    return SQL_ERROR;
                }
                let entry = SqlStmtEntry {
                    dbc_id,
                    results: Vec::new(),
                    current_row: 0,
                    columns: Vec::new(),
                };
                let id = slot_alloc(&STMT_STORE, entry);
                *output_handle = (id + 1) as *mut c_void;
                SQL_SUCCESS
            }
            _ => SQL_ERROR,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLFreeHandle(handle_type: c_int, handle: *mut c_void) -> c_int {
    if handle.is_null() {
        return SQL_ERROR;
    }
    let id = (handle as usize).saturating_sub(1);
    match handle_type {
        SQL_HANDLE_ENV => SQL_SUCCESS,
        SQL_HANDLE_DBC => {
            let mut v = DBC_STORE.lock().unwrap();
            if id < v.len() {
                v[id] = None;
            }
            SQL_SUCCESS
        }
        SQL_HANDLE_STMT => {
            let mut v = STMT_STORE.lock().unwrap();
            if id < v.len() {
                v[id] = None;
            }
            SQL_SUCCESS
        }
        _ => SQL_ERROR,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLConnect(
    connection_handle: *mut c_void,
    server_name: *const c_char,
    _nl1: c_int,
    _user: *const c_char,
    _nl2: c_int,
    _auth: *const c_char,
    _nl3: c_int,
) -> c_int {
    unsafe {
        if connection_handle.is_null() || server_name.is_null() {
            return SQL_ERROR;
        }
        let dsn = match cstr_to_str(server_name) {
            Ok(s) => s,
            Err(_) => return SQL_ERROR,
        };
        let path = parse_dsn(dsn).unwrap_or_else(|| dsn.to_string());
        let id = (connection_handle as usize).saturating_sub(1);
        let mut v = DBC_STORE.lock().unwrap();
        let entry = match v.get_mut(id).and_then(|e| e.as_mut()) {
            Some(e) => e,
            None => return SQL_ERROR,
        };
        let source = PathBuf::from(path);
        if !source.exists() {
            return SQL_ERROR;
        }
        entry.source = Some(source);
        SQL_SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLDisconnect(connection_handle: *mut c_void) -> c_int {
    if connection_handle.is_null() {
        return SQL_ERROR;
    }
    let id = (connection_handle as usize).saturating_sub(1);
    let mut v = DBC_STORE.lock().unwrap();
    match v.get_mut(id).and_then(|e| e.as_mut()) {
        Some(entry) => {
            entry.source = None;
            SQL_SUCCESS
        }
        None => SQL_ERROR,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLExecDirect(
    statement_handle: *mut c_void,
    statement_text: *const c_char,
    _text_length: c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() || statement_text.is_null() {
            return SQL_ERROR;
        }
        let query = match cstr_to_str(statement_text) {
            Ok(s) => s,
            Err(_) => return SQL_ERROR,
        };

        let stmt_id = (statement_handle as usize).saturating_sub(1);

        let parsed_query = match parse_query(query) {
            Some(query) => query,
            None => return SQL_ERROR,
        };

        let dbc_id = {
            let sv = STMT_STORE.lock().unwrap();
            match sv.get(stmt_id).and_then(|e| e.as_ref()) {
                Some(s) => s.dbc_id,
                None => return SQL_ERROR,
            }
        };

        let path = {
            let dv = DBC_STORE.lock().unwrap();
            let entry = match dv.get(dbc_id).and_then(|e| e.as_ref()) {
                Some(e) => e,
                None => return SQL_ERROR,
            };
            let source = match &entry.source {
                Some(source) => source,
                None => return SQL_ERROR,
            };
            match resolve_table_path(source, &parsed_query.table) {
                Some(path) => path,
                None => return SQL_ERROR,
            }
        };

        let mut db = YamlDb::new(path);
        if db.load().is_err() {
            return SQL_ERROR;
        }
        let records = db
            .query(&parsed_query.op)
            .to_vec()
            .into_iter()
            .cloned()
            .collect::<Vec<Record>>();

        let columns = result_columns(&records);

        let mut sv = STMT_STORE.lock().unwrap();
        if let Some(stmt) = sv.get_mut(stmt_id).and_then(|e| e.as_mut()) {
            stmt.results = records;
            stmt.current_row = 0;
            stmt.columns = columns;
            SQL_SUCCESS
        } else {
            SQL_ERROR
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLFetch(statement_handle: *mut c_void) -> c_int {
    if statement_handle.is_null() {
        return SQL_ERROR;
    }
    let id = (statement_handle as usize).saturating_sub(1);
    let mut v = STMT_STORE.lock().unwrap();
    match v.get_mut(id).and_then(|e| e.as_mut()) {
        Some(stmt) => {
            if stmt.current_row < stmt.results.len() {
                stmt.current_row += 1;
                SQL_SUCCESS
            } else {
                SQL_NO_DATA
            }
        }
        None => SQL_ERROR,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLGetData(
    statement_handle: *mut c_void,
    column_number: c_int,
    _target_type: c_int,
    target_value: *mut c_char,
    buffer_length: c_int,
    strlen_or_ind: *mut c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() || target_value.is_null() || buffer_length <= 0 {
            return SQL_ERROR;
        }
        let id = (statement_handle as usize).saturating_sub(1);
        let v = STMT_STORE.lock().unwrap();
        let stmt = match v.get(id).and_then(|e| e.as_ref()) {
            Some(s) => s,
            None => return SQL_ERROR,
        };
        if stmt.current_row == 0 || stmt.current_row > stmt.results.len() {
            return SQL_ERROR;
        }
        let row_idx = stmt.current_row - 1;
        let col_idx = (column_number - 1) as usize;
        if col_idx >= stmt.columns.len() {
            return SQL_ERROR;
        }
        let col = stmt.columns[col_idx].clone();
        let value = val_str(&stmt.results[row_idx], &col);
        let bytes = value.as_bytes();
        write_bytes(target_value, buffer_length, bytes);
        if !strlen_or_ind.is_null() {
            *strlen_or_ind = bytes.len() as c_int;
        }
        SQL_SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLNumResultCols(
    statement_handle: *mut c_void,
    column_count: *mut c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() || column_count.is_null() {
            return SQL_ERROR;
        }
        let id = (statement_handle as usize).saturating_sub(1);
        let v = STMT_STORE.lock().unwrap();
        match v.get(id).and_then(|e| e.as_ref()) {
            Some(stmt) => {
                *column_count = stmt.columns.len() as c_int;
                SQL_SUCCESS
            }
            None => SQL_ERROR,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLDescribeCol(
    statement_handle: *mut c_void,
    column_number: c_int,
    column_name: *mut c_char,
    buffer_length: c_int,
    name_length: *mut c_int,
    data_type: *mut c_int,
    _column_size: *mut c_int,
    _decimal_digits: *mut c_int,
    _nullable: *mut c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() || column_name.is_null() || buffer_length <= 0 {
            return SQL_ERROR;
        }
        let id = (statement_handle as usize).saturating_sub(1);
        let v = STMT_STORE.lock().unwrap();
        let stmt = match v.get(id).and_then(|e| e.as_ref()) {
            Some(s) => s,
            None => return SQL_ERROR,
        };
        let col_idx = (column_number - 1) as usize;
        if col_idx >= stmt.columns.len() {
            return SQL_ERROR;
        }
        let name = stmt.columns[col_idx].as_bytes();
        write_bytes(column_name, buffer_length, name);
        if !name_length.is_null() {
            *name_length = name.len() as c_int;
        }
        if !data_type.is_null() {
            *data_type = SQL_VARCHAR;
        }
        SQL_SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLRowCount(
    statement_handle: *mut c_void,
    row_count: *mut c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() || row_count.is_null() {
            return SQL_ERROR;
        }
        let id = (statement_handle as usize).saturating_sub(1);
        let v = STMT_STORE.lock().unwrap();
        match v.get(id).and_then(|e| e.as_ref()) {
            Some(stmt) => {
                *row_count = stmt.results.len() as c_int;
                SQL_SUCCESS
            }
            None => SQL_ERROR,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLColAttribute(
    statement_handle: *mut c_void,
    _column_number: c_int,
    _field_identifier: c_int,
    _character_attribute: *mut c_char,
    _buffer_length: c_int,
    _string_length: *mut c_int,
    numeric_attribute: *mut c_int,
) -> c_int {
    unsafe {
        if statement_handle.is_null() {
            return SQL_ERROR;
        }
        let id = (statement_handle as usize).saturating_sub(1);
        let v = STMT_STORE.lock().unwrap();
        if v.get(id).and_then(|e| e.as_ref()).is_none() {
            return SQL_ERROR;
        }
        if !numeric_attribute.is_null() {
            *numeric_attribute = 256;
        }
        SQL_SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLTables(
    statement_handle: *mut c_void,
    _catalog_name: *const c_char,
    _name_length1: c_int,
    _schema_name: *const c_char,
    _name_length2: c_int,
    table_name: *const c_char,
    _name_length3: c_int,
    _table_type: *const c_char,
    _name_length4: c_int,
) -> c_int {
    unsafe {
        let source = match source_for_statement(statement_handle) {
            Some(source) => source,
            None => return SQL_ERROR,
        };
        let table_pattern = if table_name.is_null() {
            None
        } else {
            cstr_to_str(table_name).ok()
        };
        let records = list_tables(&source)
            .into_iter()
            .filter(|table| matches_pattern(table, table_pattern))
            .map(|table| {
                let mut record = Record::new(table.clone());
                record.set("TABLE_NAME", table);
                record.set("TABLE_TYPE", "TABLE");
                record
            })
            .collect();
        set_statement_results(
            statement_handle,
            records,
            vec!["TABLE_NAME".to_string(), "TABLE_TYPE".to_string()],
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SQLColumns(
    statement_handle: *mut c_void,
    _catalog_name: *const c_char,
    _name_length1: c_int,
    _schema_name: *const c_char,
    _name_length2: c_int,
    table_name: *const c_char,
    _name_length3: c_int,
    column_name: *const c_char,
    _name_length4: c_int,
) -> c_int {
    unsafe {
        let source = match source_for_statement(statement_handle) {
            Some(source) => source,
            None => return SQL_ERROR,
        };
        let table_pattern = if table_name.is_null() {
            None
        } else {
            cstr_to_str(table_name).ok()
        };
        let column_pattern = if column_name.is_null() {
            None
        } else {
            cstr_to_str(column_name).ok()
        };

        let mut records = Vec::new();
        for table in list_tables(&source)
            .into_iter()
            .filter(|table| matches_pattern(table, table_pattern))
        {
            let Some(path) = resolve_table_path(&source, &table) else {
                continue;
            };
            let mut db = YamlDb::new(path);
            if db.load().is_err() {
                continue;
            }
            let rows = db.read_all().into_iter().cloned().collect::<Vec<Record>>();
            for (index, column) in result_columns(&rows)
                .into_iter()
                .filter(|column| matches_pattern(column, column_pattern))
                .enumerate()
            {
                let mut record = Record::new(format!("{table}.{column}"));
                record.set("TABLE_NAME", table.clone());
                record.set("COLUMN_NAME", column);
                record.set("TYPE_NAME", "VARCHAR");
                record.set("ORDINAL_POSITION", (index + 1) as i64);
                records.push(record);
            }
        }

        set_statement_results(
            statement_handle,
            records,
            vec![
                "TABLE_NAME".to_string(),
                "COLUMN_NAME".to_string(),
                "TYPE_NAME".to_string(),
                "ORDINAL_POSITION".to_string(),
            ],
        )
    }
}
