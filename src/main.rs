use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use yamldb::{QueryOp, Record, YamlDb};

#[derive(Parser)]
#[command(name = "yamldb", about = "YAML file based database CLI")]
struct Cli {
    #[arg(short, long, default_value = "data.yaml")]
    file: PathBuf,
    #[arg(short, long, default_value = "data", global = true)]
    table: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Create {
        id: String,
        #[arg(short, long, value_delimiter = ',')]
        fields: Vec<String>,
    },
    Get {
        id: String,
        #[arg(long)]
        format: Option<String>,
    },
    List {
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Update {
        id: String,
        #[arg(short, long, value_delimiter = ',')]
        fields: Vec<String>,
    },
    Delete {
        id: String,
    },
    Query {
        #[arg(short, long)]
        key: String,
        #[arg(short, long)]
        value: String,
        #[arg(long)]
        op: Option<String>,
    },
    Search {
        #[arg(short, long)]
        keyword: String,
        #[arg(short, long)]
        key: Option<String>,
    },
    Import {
        #[arg(short, long)]
        input: PathBuf,
    },
    Export {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    Backup {
        #[arg(short, long)]
        output: PathBuf,
    },
    Stats,
    Count,
    Exists {
        id: String,
    },
    Clear {
        #[arg(long)]
        force: bool,
    },
    Tables,
    Webui {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

fn parse_value(value: &str) -> serde_yaml::Value {
    if let Ok(n) = value.parse::<i64>() {
        serde_yaml::Value::Number(n.into())
    } else if let Ok(f) = value.parse::<f64>() {
        serde_yaml::Value::Number(serde_yaml::Number::from(f))
    } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        serde_yaml::Value::Bool(value.eq_ignore_ascii_case("true"))
    } else {
        serde_yaml::Value::String(value.to_string())
    }
}

fn parse_fields(
    fields: &[String],
) -> Result<HashMap<String, serde_yaml::Value>, Box<dyn std::error::Error>> {
    let mut map = HashMap::new();
    for field in fields {
        let Some((key, value)) = field.split_once('=') else {
            return Err(format!("Invalid field '{}', expected key=value", field).into());
        };
        if key.trim().is_empty() {
            return Err(format!("Invalid field '{}', key cannot be empty", field).into());
        }
        map.insert(key.trim().to_string(), parse_value(value.trim()));
    }
    Ok(map)
}

fn format_record(record: &Record, format: Option<&str>) -> String {
    match format {
        Some("json") => record.to_json().unwrap_or_default(),
        _ => format!(
            "{}: {}",
            record.id,
            serde_yaml::to_string(&record.data).unwrap_or_default()
        ),
    }
}

fn import_records(db: &mut YamlDb, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "json" => {
            db.import_json(path)?;
        }
        "yaml" | "yml" => {
            db.import_yaml(path)?;
        }
        _ => return Err(format!("Unsupported format: {}", ext).into()),
    }

    Ok(())
}

fn command_allows_new_table(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Create { .. } | Commands::Import { .. } | Commands::Clear { force: true }
    )
}

fn resolve_cli_table_path(
    source: &Path,
    table: &str,
    allow_new_table: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if table.trim().is_empty()
        || table.contains('/')
        || table.contains('\\')
        || table == "."
        || table == ".."
    {
        return Err(format!("invalid table name: {}", table).into());
    }

    if source.is_file() || !source.exists() {
        let source_table = webui_table_name(source).unwrap_or_else(|| "data".to_string());
        if table.eq_ignore_ascii_case("data") || table.eq_ignore_ascii_case(&source_table) {
            return Ok(source.to_path_buf());
        }
        return Err(format!("unknown table '{}' for file {}", table, source.display()).into());
    }

    if !source.is_dir() {
        return Err(format!("not a YAML file or directory: {}", source.display()).into());
    }

    if let Ok(path) = webui_resolve_table_path(source, table) {
        return Ok(path);
    }

    if allow_new_table {
        return Ok(source.join(format!("{table}.yaml")));
    }

    Err(format!("unknown table '{}' in {}", table, source.display()).into())
}

fn record_json(record: &Record) -> serde_json::Value {
    let mut value = serde_json::to_value(&record.data).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "id".to_string(),
            serde_json::Value::String(record.id.clone()),
        );
    }
    value
}

fn json_to_record(value: serde_json::Value) -> Result<Record, Box<dyn std::error::Error>> {
    let mut obj = value
        .as_object()
        .ok_or("request body must be a JSON object")?
        .clone();
    let id = obj
        .remove("id")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("missing string field: id")?;
    if id.trim().is_empty() {
        return Err("id cannot be empty".into());
    }
    let data = obj
        .into_iter()
        .map(|(key, value)| {
            let yaml = serde_yaml::to_value(value).unwrap_or(serde_yaml::Value::Null);
            (key, yaml)
        })
        .collect();
    Ok(Record { id, data })
}

#[derive(Debug)]
struct WebuiSource {
    path: PathBuf,
}

fn webui_table_name(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn webui_is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        .unwrap_or(false)
}

fn webui_list_tables(source: &Path) -> Vec<String> {
    if source.is_file() || !source.exists() {
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
        .filter(|path| path.is_file() && webui_is_yaml_path(path))
        .filter_map(|path| webui_table_name(&path))
        .collect();
    tables.sort_by_key(|name| name.to_lowercase());
    tables
}

fn webui_resolve_table_path(
    source: &Path,
    table: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if source.is_file() || !source.exists() {
        let source_table = webui_table_name(source).unwrap_or_else(|| "data".to_string());
        if table.eq_ignore_ascii_case("data") || table.eq_ignore_ascii_case(&source_table) {
            return Ok(source.to_path_buf());
        }
        return Err(format!("unknown table: {}", table).into());
    }

    if !source.is_dir() {
        return Err(format!("not a YAML file or directory: {}", source.display()).into());
    }

    for ext in ["yaml", "yml"] {
        let path = source.join(format!("{table}.{ext}"));
        if path.is_file() {
            return Ok(path);
        }
    }

    let path = std::fs::read_dir(source)?.flatten().find_map(|entry| {
        let path = entry.path();
        if path.is_file()
            && webui_is_yaml_path(&path)
            && webui_table_name(&path)
                .map(|name| name.eq_ignore_ascii_case(table))
                .unwrap_or(false)
        {
            Some(path)
        } else {
            None
        }
    });
    path.ok_or_else(|| format!("unknown table: {}", table).into())
}

fn run_webui(source: PathBuf, host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((host, port))?;
    let source = Arc::new(Mutex::new(WebuiSource { path: source }));
    let url = format!("http://{}:{}", host, listener.local_addr()?.port());
    println!("YamlDB Web UI: {}", url);
    println!("Press Ctrl+C to stop");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let source = Arc::clone(&source);
                if let Err(err) = handle_webui_stream(stream, source) {
                    eprintln!("webui request error: {}", err);
                }
            }
            Err(err) => eprintln!("webui connection error: {}", err),
        }
    }
    Ok(())
}

fn handle_webui_stream(
    mut stream: TcpStream,
    source: Arc<Mutex<WebuiSource>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            let total = header_end + 4 + content_length;
            while buffer.len() < total {
                let n = stream.read(&mut chunk)?;
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..n]);
            }
            break;
        }
        if buffer.len() > 1024 * 1024 {
            return Err("request too large".into());
        }
    }

    let Some(header_end) = find_header_end(&buffer) else {
        write_response(&mut stream, 400, "text/plain", b"bad request")?;
        return Ok(());
    };
    let request_line = String::from_utf8_lossy(&buffer[..header_end])
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        write_response(&mut stream, 400, "text/plain", b"bad request")?;
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];
    let body = &buffer[header_end + 4..];

    match route_webui(method, path, body, source) {
        Ok((status, content_type, body)) => {
            write_response(&mut stream, status, content_type, body.as_bytes())?
        }
        Err(err) => {
            let body = serde_json::json!({ "error": err.to_string() }).to_string();
            write_response(&mut stream, 400, "application/json", body.as_bytes())?;
        }
    }
    Ok(())
}

fn route_webui(
    method: &str,
    path: &str,
    body: &[u8],
    source: Arc<Mutex<WebuiSource>>,
) -> Result<(u16, &'static str, String), Box<dyn std::error::Error>> {
    let path_only = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    match (method, path_only) {
        ("GET", "/") => Ok((200, "text/html; charset=utf-8", WEBUI_HTML.to_string())),
        ("GET", "/api/records") => {
            let records = webui_read_records(&source, "data")?;
            Ok((200, "application/json", serde_json::to_string(&records)?))
        }
        ("POST", "/api/records") => {
            let value: serde_json::Value = serde_json::from_slice(body)?;
            webui_upsert_record(&source, "data", value)?;
            Ok((200, "application/json", r#"{"ok":true}"#.to_string()))
        }
        _ if method == "DELETE" && path_only.starts_with("/api/records/") => {
            let id = url_decode(&path_only["/api/records/".len()..])?;
            webui_delete_record(&source, "data", &id)?;
            Ok((200, "application/json", r#"{"ok":true}"#.to_string()))
        }
        ("GET", "/api/tables") => {
            let source = source.lock().unwrap();
            let tables = webui_list_tables(&source.path);
            Ok((200, "application/json", serde_json::to_string(&tables)?))
        }
        _ if method == "GET"
            && path_only.starts_with("/api/tables/")
            && path_only.ends_with("/records") =>
        {
            let table = &path_only["/api/tables/".len()..path_only.len() - "/records".len()];
            let table = url_decode(table)?;
            let records = webui_read_records(&source, &table)?;
            Ok((200, "application/json", serde_json::to_string(&records)?))
        }
        _ if method == "POST"
            && path_only.starts_with("/api/tables/")
            && path_only.ends_with("/records") =>
        {
            let table = &path_only["/api/tables/".len()..path_only.len() - "/records".len()];
            let table = url_decode(table)?;
            let value: serde_json::Value = serde_json::from_slice(body)?;
            webui_upsert_record(&source, &table, value)?;
            Ok((200, "application/json", r#"{"ok":true}"#.to_string()))
        }
        _ if method == "DELETE" && path_only.starts_with("/api/tables/") => {
            let rest = &path_only["/api/tables/".len()..];
            let Some((table, record_path)) = rest.split_once("/records/") else {
                return Ok((404, "text/plain", "not found".to_string()));
            };
            let table = url_decode(table)?;
            let id = url_decode(record_path)?;
            webui_delete_record(&source, &table, &id)?;
            Ok((200, "application/json", r#"{"ok":true}"#.to_string()))
        }
        _ => Ok((404, "text/plain", "not found".to_string())),
    }
}

fn webui_read_records(
    source: &Arc<Mutex<WebuiSource>>,
    table: &str,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let path = {
        let source = source.lock().unwrap();
        webui_resolve_table_path(&source.path, table)?
    };
    let mut db = YamlDb::new(path);
    db.load()?;
    Ok(db.read_all().into_iter().map(record_json).collect())
}

fn webui_upsert_record(
    source: &Arc<Mutex<WebuiSource>>,
    table: &str,
    value: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let record = json_to_record(value)?;
    let path = {
        let source = source.lock().unwrap();
        webui_resolve_table_path(&source.path, table)?
    };
    let mut db = YamlDb::new(path);
    db.load()?;
    db.upsert(record)?;
    Ok(())
}

fn webui_delete_record(
    source: &Arc<Mutex<WebuiSource>>,
    table: &str,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = {
        let source = source.lock().unwrap();
        webui_resolve_table_path(&source.path, table)?
    };
    let mut db = YamlDb::new(path);
    db.load()?;
    db.delete(id)?;
    Ok(())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
    )?;
    stream.write_all(body)
}

fn url_decode(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    let mut iter = value.as_bytes().iter().copied();
    while let Some(byte) = iter.next() {
        match byte {
            b'%' => {
                let hi = iter.next().ok_or("invalid percent encoding")?;
                let lo = iter.next().ok_or("invalid percent encoding")?;
                let hex = [hi, lo];
                let hex = std::str::from_utf8(&hex)?;
                bytes.push(u8::from_str_radix(hex, 16)?);
            }
            b'+' => bytes.push(b' '),
            other => bytes.push(other),
        }
    }
    Ok(String::from_utf8(bytes)?)
}

const WEBUI_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>YamlDB Web UI</title>
  <style>
    body { margin: 0; font-family: system-ui, sans-serif; color: #17202a; background: #f6f7f9; }
    header { padding: 14px 20px; background: #1f2937; color: white; display: flex; align-items: center; gap: 12px; }
    header h1 { font-size: 18px; margin: 0; }
    main { display: grid; grid-template-columns: minmax(220px, 320px) 1fr; min-height: calc(100vh - 52px); }
    aside { border-right: 1px solid #d8dee8; background: white; padding: 14px; overflow: auto; }
    section { padding: 18px; overflow: auto; }
    input, textarea, button { font: inherit; }
    input, textarea { width: 100%; box-sizing: border-box; border: 1px solid #ccd3dd; border-radius: 6px; padding: 9px; background: white; }
    textarea { min-height: 430px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; line-height: 1.45; }
    button { border: 0; border-radius: 6px; padding: 9px 12px; background: #2563eb; color: white; cursor: pointer; }
    button.secondary { background: #64748b; }
    button.danger { background: #dc2626; }
    .toolbar { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; flex-wrap: wrap; }
    .list { display: grid; gap: 6px; }
    .item { text-align: left; background: #eef2ff; color: #1e3a8a; overflow: hidden; text-overflow: ellipsis; }
    .item.active { background: #2563eb; color: white; }
    .meta { color: #64748b; font-size: 13px; margin-bottom: 10px; }
    .error { color: #b91c1c; white-space: pre-wrap; }
    @media (max-width: 760px) { main { grid-template-columns: 1fr; } aside { border-right: 0; border-bottom: 1px solid #d8dee8; } }
  </style>
</head>
<body>
  <header><h1>YamlDB</h1><span id="status"></span></header>
  <main>
    <aside>
      <div class="meta">Tables</div>
      <div id="tables" class="list"></div>
      <hr>
      <div class="toolbar">
        <input id="search" placeholder="Search records">
        <button class="secondary" onclick="newRecord()">New</button>
      </div>
      <div class="meta"><span id="count">0</span> records</div>
      <div id="records" class="list"></div>
    </aside>
    <section>
      <div class="toolbar">
        <button onclick="saveRecord()">Save</button>
        <button class="danger" onclick="deleteRecord()">Delete</button>
        <button class="secondary" onclick="loadRecords()">Reload</button>
      </div>
      <textarea id="editor" spellcheck="false"></textarea>
      <p class="error" id="error"></p>
    </section>
  </main>
  <script>
    let tables = [];
    let selectedTable = 'data';
    let records = [];
    let selected = null;

    async function api(path, options = {}) {
      const response = await fetch(path, options);
      if (!response.ok) {
        let text = await response.text();
        try { text = JSON.parse(text).error || text; } catch (_) {}
        throw new Error(text);
      }
      return response.headers.get('content-type')?.includes('json') ? response.json() : response.text();
    }

    async function loadTables() {
      clearError();
      tables = await api('/api/tables');
      if (!tables.length) {
        tables = ['data'];
      }
      if (!tables.includes(selectedTable)) {
        selectedTable = tables[0];
      }
      renderTables();
      await loadRecords();
    }

    function renderTables() {
      const list = document.getElementById('tables');
      list.innerHTML = '';
      for (const table of tables) {
        const button = document.createElement('button');
        button.className = 'item' + (table === selectedTable ? ' active' : '');
        button.textContent = table;
        button.onclick = () => selectTable(table);
        list.appendChild(button);
      }
    }

    async function selectTable(table) {
      selectedTable = table;
      selected = null;
      document.getElementById('editor').value = '';
      renderTables();
      await loadRecords();
    }

    async function loadRecords() {
      clearError();
      records = await api('/api/tables/' + encodeURIComponent(selectedTable) + '/records');
      records.sort((a, b) => String(a.id).localeCompare(String(b.id)));
      renderList();
      if (records.length && !selected) selectRecord(records[0].id);
      document.getElementById('status').textContent = 'table: ' + selectedTable;
    }

    function renderList() {
      const query = document.getElementById('search').value.toLowerCase();
      const list = document.getElementById('records');
      const shown = records.filter(r => JSON.stringify(r).toLowerCase().includes(query));
      document.getElementById('count').textContent = records.length;
      list.innerHTML = '';
      for (const record of shown) {
        const button = document.createElement('button');
        button.className = 'item' + (record.id === selected ? ' active' : '');
        button.textContent = record.id;
        button.onclick = () => selectRecord(record.id);
        list.appendChild(button);
      }
    }

    function selectRecord(id) {
      selected = id;
      const record = records.find(r => r.id === id);
      document.getElementById('editor').value = JSON.stringify(record || { id }, null, 2);
      renderList();
      clearError();
    }

    function newRecord() {
      selected = null;
      document.getElementById('editor').value = JSON.stringify({ id: 'new-record' }, null, 2);
      renderList();
      clearError();
    }

    async function saveRecord() {
      try {
        clearError();
        const value = JSON.parse(document.getElementById('editor').value);
        await api('/api/tables/' + encodeURIComponent(selectedTable) + '/records', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(value)
        });
        selected = value.id;
        await loadRecords();
      } catch (error) {
        showError(error);
      }
    }

    async function deleteRecord() {
      try {
        clearError();
        const value = JSON.parse(document.getElementById('editor').value || '{}');
        if (!value.id) throw new Error('No record id selected');
        await api('/api/tables/' + encodeURIComponent(selectedTable) + '/records/' + encodeURIComponent(value.id), { method: 'DELETE' });
        selected = null;
        document.getElementById('editor').value = '';
        await loadRecords();
      } catch (error) {
        showError(error);
      }
    }

    function showError(error) { document.getElementById('error').textContent = error.message || String(error); }
    function clearError() { document.getElementById('error').textContent = ''; }
    document.getElementById('search').addEventListener('input', renderList);
    loadTables().catch(showError);
  </script>
</body>
</html>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if let Commands::Webui { host, port } = &cli.command {
        return run_webui(cli.file.clone(), host, *port);
    }

    if let Commands::Tables = &cli.command {
        for table in webui_list_tables(&cli.file) {
            println!("{}", table);
        }
        return Ok(());
    }

    let table_path = resolve_cli_table_path(
        &cli.file,
        &cli.table,
        command_allows_new_table(&cli.command),
    )?;
    let mut db = YamlDb::new(&table_path);
    db.load()?;

    match cli.command {
        Commands::Create { id, fields } => {
            let data = parse_fields(&fields)?;
            db.create(Record {
                id: id.clone(),
                data,
            })?;
            println!("Created record: {}", id);
        }
        Commands::Get { id, format } => {
            let record = db.read(&id)?;
            println!("{}", format_record(record, format.as_deref()));
        }
        Commands::List { format, limit } => {
            let records = db.read_all();
            if records.is_empty() {
                println!("No records found");
            } else {
                let iter: Vec<_> = if let Some(n) = limit {
                    records.into_iter().take(n).collect()
                } else {
                    records
                };
                for record in iter {
                    println!("{}", format_record(record, format.as_deref()));
                }
            }
        }
        Commands::Update { id, fields } => {
            let data = parse_fields(&fields)?;
            db.update(&id, data)?;
            println!("Updated record: {}", id);
        }
        Commands::Delete { id } => {
            db.delete(&id)?;
            println!("Deleted record: {}", id);
        }
        Commands::Query { key, value, op } => {
            let query_op = match op.as_deref() {
                Some("ne") => QueryOp::ne(key, parse_value(&value)),
                Some("gt") => QueryOp::gt(key, parse_value(&value)),
                Some("lt") => QueryOp::lt(key, parse_value(&value)),
                Some("gte") => QueryOp::gte(key, parse_value(&value)),
                Some("lte") => QueryOp::lte(key, parse_value(&value)),
                Some("contains") => QueryOp::contains(key, value),
                Some(other) => return Err(format!("Unsupported query operator: {}", other).into()),
                _ => QueryOp::eq(key, parse_value(&value)),
            };
            let results = db.query(&query_op);
            if results.is_empty() {
                println!("No matching records");
            } else {
                for record in results.to_vec() {
                    println!("{}", format_record(record, None));
                }
            }
        }
        Commands::Search { keyword, key } => {
            let results = if let Some(k) = key {
                db.search(&k, &keyword)
            } else {
                db.search_all(&keyword)
            };
            if results.is_empty() {
                println!("No matching records");
            } else {
                for record in results.to_vec() {
                    println!("{}", format_record(record, None));
                }
            }
        }
        Commands::Import { input } => {
            import_records(&mut db, &input)?;
            println!("Imported records from: {}", input.display());
        }
        Commands::Export { output, format } => {
            match format.as_str() {
                "json" => db.export_json(&output)?,
                "yaml" | "yml" => db.export_yaml(&output)?,
                _ => return Err(format!("Unsupported format: {}", format).into()),
            }
            println!("Exported to: {}", output.display());
        }
        Commands::Backup { output } => {
            db.backup(&output)?;
            println!("Backup created: {}", output.display());
        }
        Commands::Stats => {
            let stats = db.stats();
            println!("Total records: {}", stats.total_records);
            println!("Total unique keys: {}", stats.total_keys);
            println!("Keys: {:?}", stats.unique_keys);
            if let Some(size) = stats.file_size {
                println!("File size: {} bytes", size);
            }
        }
        Commands::Count => {
            println!("{}", db.count());
        }
        Commands::Exists { id } => {
            if db.exists(&id) {
                println!("true");
            } else {
                println!("false");
            }
        }
        Commands::Clear { force } => {
            if force {
                db.clear()?;
                println!("Database cleared");
            } else {
                println!("Use --force to confirm clearing the database");
            }
        }
        Commands::Tables => unreachable!("tables is handled before loading a table"),
        Commands::Webui { .. } => unreachable!("webui is handled before loading the YAML file"),
    }

    Ok(())
}
