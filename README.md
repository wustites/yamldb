# YamlDB

[中文文档](README.zh-CN.md)

[![CI](https://github.com/markchenim/yamldb/actions/workflows/ci.yml/badge.svg)](https://github.com/markchenim/yamldb/actions/workflows/ci.yml)
[![Release](https://github.com/markchenim/yamldb/actions/workflows/release.yml/badge.svg)](https://github.com/markchenim/yamldb/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A lightweight YAML file-based database with CLI tool, Rust API, ODBC driver, and JDBC driver support.

## Features

- **YAML Storage** - Human-readable data format
- **CRUD Operations** - Create, Read, Update, Delete
- **Query Builder** - Flexible conditions with AND/OR/NOT
- **Fuzzy Search** - Case-insensitive keyword search
- **Import/Export** - JSON and YAML formats
- **Backup** - YAML database snapshots
- **CLI Tool** - Command-line interface
- **ODBC Driver** - SQL query support
- **JDBC Driver** - Java SQL access support

## Installation

YamlDB uses `yq` v4 for YAML file parsing and formatting. Release packages may bundle `yq`; otherwise install the `yq` command and make sure it is available on `PATH`.

Examples:

```bash
# macOS
brew install yq

# Linux with Homebrew
brew install yq

# Or download a yq v4 binary from https://github.com/mikefarah/yq
yq --version
```

`yq` lookup order:

1. `YAMLDB_YQ` environment variable.
2. Bundled `yq` next to the CLI/ODBC driver, or in `bin/` next to it.
3. For JDBC, bundled JAR resources under `bin/<os>-<arch>/yq`, for example `bin/linux-amd64/yq`.
4. `yq` or `yq.exe` from `PATH`.

### From Cargo

```bash
cargo install yamldb
```

### From Releases

Download pre-built binaries from [GitHub Releases](https://github.com/markchenim/yamldb/releases):

| Platform | CLI | ODBC Driver | JDBC Driver |
|----------|-----|-------------|-------------|
| Linux | `yamldb-linux` | `libyamldb-linux-odbc.so` | `yamldb-jdbc.jar` |
| Windows | `yamldb-windows.exe` | `yamldb-windows-odbc.dll` | `yamldb-jdbc.jar` |
| macOS | `yamldb-macos` | `libyamldb-macos-odbc.dylib` | `yamldb-jdbc.jar` |

## Data Sources And Tables

YamlDB uses the same source mapping across CLI, Web UI, ODBC, and JDBC.

| Source | Table names | Notes |
|--------|-------------|-------|
| Single YAML file, for example `data.yaml` | `data`; file stem such as `data` is also accepted by SQL drivers | CLI defaults to `data` |
| Directory, for example `./yaml-data` | one table per direct child `.yaml/.yml` file | `users.yaml` becomes table `users` |

Directory sources are not recursive. Put table files directly inside the selected directory:

```text
yaml-data/
  users.yaml
  teams.yml
  projects.yaml
```

The equivalent access patterns are:

```bash
# CLI
yamldb -f ./yaml-data -t users list

# Web UI
yamldb -f ./yaml-data webui
```

```sql
-- ODBC/JDBC
SELECT * FROM users
```

When creating data through CLI or Web UI, a missing table in a directory source is created as `<table>.yaml`.

### Persistence Guarantees

File-backed mutations are committed to memory only after the updated YAML has been written successfully. YamlDB writes a synchronized temporary file in the destination directory and atomically replaces the database file, so an I/O or `yq` failure leaves both the previously saved file and the in-memory database state unchanged.

This protects an individual YamlDB write from partial replacement. It does not provide locking or transactions between multiple processes; applications must coordinate concurrent writers.

### Capability Matrix

| Surface | Single file | Directory tables | Write support | Table metadata |
|---------|-------------|------------------|---------------|----------------|
| CLI | yes | yes, via `--table` | yes | `tables` command |
| Web UI | yes | yes, table selector | yes | table selector |
| ODBC | yes | yes, `SELECT * FROM <table>` | read-only | `SQLTables` / `SQLColumns` |
| JDBC | yes | yes, `SELECT * FROM <table>` | read-only | `DatabaseMetaData` |

## CLI Usage

### Global Options

```
-f, --file <FILE>  Specify YAML file path [default: data.yaml]
-t, --table <TABLE>  Select a table when --file points to a YAML directory [default: data]
```

The `--table` option is global, so it can be placed before or after the subcommand.

### Record Operations

```bash
# Single-file source
yamldb create user1 --fields name=Alice,age=30,city=Beijing
yamldb get user1
yamldb list
yamldb list --limit 10
yamldb list --format json
yamldb update user1 --fields age=31,city=Guangzhou
yamldb delete user1

# Directory source
yamldb -f ./yaml-data tables
yamldb -f ./yaml-data -t users list
yamldb -f ./yaml-data -t users create user1 --fields name=Alice,age=30
yamldb -f ./yaml-data -t users get user1 --format json
yamldb -f ./yaml-data -t projects create p1 --fields name=Core
```

### Query & Search

```bash
# Equality
yamldb query --key city --value Beijing

# Comparison operators
yamldb query --key age --value 25 --op gt      # > 25
yamldb query --key age --value 30 --op gte     # >= 30
yamldb query --key age --value 50 --op lt      # < 50
yamldb query --key age --value 25 --op lte     # <= 25
yamldb query --key city --value Beijing --op ne  # != Beijing

# String operations
yamldb query --key name --value Ali --op contains

# Fuzzy search
yamldb search --keyword alice
yamldb search --keyword alice --key name
```

Equality and comparison values are parsed as YAML scalar types. For example, `--value 30` matches a numeric `age: 30`, while `--value true` matches a boolean value. The CLI does not currently provide a type override for numeric-looking or boolean-looking strings; use the Rust API when an exact string comparison is required for those values.

### Import & Export

```bash
# Import
yamldb import -i users.json
yamldb import -i users.yaml
yamldb -f ./yaml-data -t users import -i users.yaml

# Export
yamldb export -o backup.json
yamldb export -o backup.yaml --format yaml
yamldb -f ./yaml-data -t users export -o users.json

# Backup
yamldb backup -o backup.yaml
yamldb -f ./yaml-data -t users backup -o users-backup.yaml
```

### Utility Commands

```bash
# Statistics
yamldb stats
yamldb -f ./yaml-data -t users stats

# Count records
yamldb count
yamldb -f ./yaml-data -t users count

# Check existence
yamldb exists user1
yamldb -f ./yaml-data -t users exists user1

# Clear database
yamldb clear --force
yamldb -f ./yaml-data -t users clear --force
```

### Web UI

Start a local browser UI for the selected YAML file:

```bash
yamldb -f data.yaml webui
yamldb -f /path/to/yaml-directory webui
yamldb -f data.yaml webui --host 127.0.0.1 --port 8080
```

The Web UI uses the same source mapping as the ODBC/JDBC drivers. A single YAML file is shown as table `data`; a directory shows each `.yaml/.yml` file as a table named after the file stem. It exposes list, search, create/update, and delete actions for the selected table.

It binds to `127.0.0.1:8080` by default and does not provide authentication, so keep it on localhost unless you put it behind your own access control.

## Rust API

### Basic Usage

```rust
use yamldb::{Record, YamlDb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open database
    let mut db = YamlDb::new("data.yaml");
    db.load()?;

    // Create record
    let mut record = Record::new("user1");
    record.set("name", "Alice")
          .set("age", 30)
          .set("city", "Beijing");
    db.create(record)?;

    // Read record
    let record = db.read("user1")?;
    println!("Name: {:?}", record.get_str("name"));
    println!("Age: {:?}", record.get_i64("age"));

    // Update single field
    db.update_field("user1", "age", serde_yaml::Value::Number(31.into()))?;

    // Delete record
    db.delete("user1")?;

    Ok(())
}
```

### Memory Database

```rust
// In-memory database (no file I/O)
let mut db = YamlDb::memory();
let mut record = Record::new("test");
record.set("key", "value");
db.create(record)?;
```

### Record API

```rust
let mut record = Record::new("user1");

// Set values (chainable)
record.set("name", "Alice")
      .set("age", 30)
      .set("active", true);

// Get typed values
record.get_str("name");    // Some("Alice")
record.get_i64("age");     // Some(30)
record.get_f64("score");   // None
record.get_bool("active"); // Some(true)

// Check keys
record.has_key("name");  // true
record.keys();           // ["name", "age", "active"]

// Merge records
let mut other = Record::new("user1");
other.set("email", "alice@example.com");
record.merge(&other);

// Convert to JSON
let json = record.to_json()?;
```

### Query Builder

```rust
use yamldb::{QueryOp, YamlDb};

let db = YamlDb::new("data.yaml");
db.load()?;

// Comparison operators
let result = db.query(&QueryOp::eq("city", "Beijing"));
let result = db.query(&QueryOp::ne("city", "Shanghai"));
let result = db.query(&QueryOp::gt("age", serde_yaml::Value::Number(25.into())));
let result = db.query(&QueryOp::gte("age", serde_yaml::Value::Number(30.into())));
let result = db.query(&QueryOp::lt("age", serde_yaml::Value::Number(50.into())));
let result = db.query(&QueryOp::lte("age", serde_yaml::Value::Number(25.into())));

// String operations
let result = db.query(&QueryOp::contains("name", "Ali"));
let result = db.query(&QueryOp::starts_with("name", "Ali"));
let result = db.query(&QueryOp::ends_with("name", "Smith"));

// Logical operators
let result = db.query(&QueryOp::and(vec![
    QueryOp::eq("city", "Beijing"),
    QueryOp::gte("age", serde_yaml::Value::Number(28.into())),
]));

let result = db.query(&QueryOp::or(vec![
    QueryOp::eq("city", "Beijing"),
    QueryOp::eq("city", "Shanghai"),
]));

let result = db.query(&QueryOp::negate(QueryOp::eq("city", "Beijing")));
```

### Search

```rust
// Search in specific field (case-insensitive)
let result = db.search("name", "alice");

// Search in all fields
let result = db.search_all("alice");
```

### QueryResult

```rust
let result = db.query(&QueryOp::eq("city", "Beijing"));

// Count
println!("Found: {}", result.count());
println!("Is empty: {}", result.is_empty());

// Access records
if let Some(first) = result.first() {
    println!("First: {}", first.id);
}
if let Some(last) = result.last() {
    println!("Last: {}", last.id);
}

// Pagination
let page1 = result.page(1, 10);  // Page 1, 10 items per page
let page2 = result.page(2, 10);  // Page 2

// Skip and limit
let skipped = result.skip(5);
let limited = result.limit(10);

// Get all IDs
let ids = result.ids();

// Iterate
for record in result.iter() {
    println!("{}: {:?}", record.id, record.data);
}

// Convert to Vec
let all = result.to_vec();
```

### Batch Operations

```rust
// Read multiple records
let records = db.read_many(&["user1", "user2", "user3"]);

// Update multiple records
let updates = vec![
    ("user1".to_string(), HashMap::from([("age".to_string(), serde_yaml::Value::Number(31.into()))])),
    ("user2".to_string(), HashMap::from([("age".to_string(), serde_yaml::Value::Number(26.into()))])),
];
let updated = db.update_many(updates)?;

// Delete multiple records
let deleted = db.delete_many(&["user1", "user2"])?;

// Upsert (insert or update)
let mut record = Record::new("user1");
record.set("name", "Alice");
db.upsert(record)?;
```

### Statistics & Backup

```rust
use std::path::Path;

// Get statistics
let stats = db.stats();
println!("Total records: {}", stats.total_records);
println!("Unique keys: {:?}", stats.unique_keys);
println!("File size: {:?} bytes", stats.file_size);

// Backup
db.backup(Path::new("backup.yaml"))?;

// Clear all records
db.clear()?;
```

### Import/Export

```rust
use std::path::Path;

// Import from JSON
let count = db.import_json(Path::new("users.json"))?;
println!("Imported {} records", count);

// Import from YAML
let count = db.import_yaml(Path::new("users.yaml"))?;

// Export to JSON
db.export_json(Path::new("backup.json"))?;

// Export to YAML
db.export_yaml(Path::new("backup.yaml"))?;
```

## ODBC Driver

YamlDB includes an ODBC driver for read-only SQL access from ODBC clients.

### Connection String

```
DRIVER={YamlDB};DBQ=data.yaml;
DRIVER={YamlDB};FILE=data.yaml;
DRIVER={YamlDB};DBQ=/path/to/yaml-directory;
```

When `DBQ`/`FILE` points to a single YAML file, query it as table `data`.
When it points to a directory, each `.yaml` or `.yml` file is exposed as a table named after the file stem, for example `users.yaml` becomes table `users`.

### Table Mapping

| Source path | Tables | Example SQL |
|-------------|--------|-------------|
| `data.yaml` | `data` | `SELECT * FROM data` |
| `users.yaml` | `data`, `users` | `SELECT * FROM users` |
| `/path/to/yaml-directory` | one table per `.yaml/.yml` file | `SELECT * FROM users` |

For directory sources, only files directly inside the selected directory are exposed. Nested directories are ignored.

### Supported SQL

```sql
-- Select all
SELECT * FROM data

-- Where clause
SELECT * FROM data WHERE city = 'Beijing'

-- Comparison operators
SELECT * FROM data WHERE age > 25
SELECT * FROM data WHERE age >= 28
SELECT * FROM data WHERE age < 30
SELECT * FROM data WHERE age <= 25
SELECT * FROM data WHERE city != 'Shanghai'

-- AND/OR conditions
SELECT * FROM data WHERE city = 'Beijing' AND age >= 28
SELECT * FROM data WHERE city = 'Beijing' OR city = 'Shanghai'

-- Directory source
SELECT * FROM users WHERE age >= 28
```

Supported SQL is intentionally small: `SELECT * FROM <table>` with optional `WHERE` comparisons joined by `AND` or `OR`. Only `SELECT *` is supported; projected column lists, joins, grouping, ordering, inserts, updates, and deletes are not SQL features of the drivers. Use CLI or Web UI for writes.

### Build Shared Library

```bash
cargo build --release
```

Output:
- Windows: `target/release/yamldb.dll`
- Linux: `target/release/libyamldb.so`
- macOS: `target/release/libyamldb.dylib`

### Register Driver

**Windows:**
1. Open ODBC Data Source Administrator
2. Go to "Drivers" tab
3. Click "Add"
4. Browse to `yamldb.dll`

**Linux:**
Add to `/etc/odbcinst.ini`:
```ini
[YamlDB]
Description=YamlDB ODBC Driver
Driver=/path/to/libyamldb.so
```

### DBeaver / ODBC Clients

Use the ODBC driver when your client supports native ODBC connections:

1. Register the YamlDB ODBC shared library in the operating system ODBC manager.
2. Create a DSN or connection with `DRIVER={YamlDB}`.
3. Set `DBQ` or `FILE` to either a YAML file or a directory containing YAML files.
4. Query `data` for a single file, or query a table named after a YAML file in a directory.

Example directory DSN:

```text
DRIVER={YamlDB};DBQ=/home/alice/yaml-data;
```

### Usage Example (Python)

```python
import pyodbc

conn = pyodbc.connect('DRIVER={YamlDB};DBQ=data.yaml;')
cursor = conn.cursor()

cursor.execute("SELECT * FROM data WHERE city = 'Beijing'")
for row in cursor:
    print(row)

conn.close()
```

### Usage Example (C#)

```csharp
using System.Data.Odbc;

var conn = new OdbcConnection("DRIVER={YamlDB};DBQ=data.yaml;");
conn.Open();

var cmd = new OdbcCommand("SELECT * FROM data WHERE age > 25", conn);
var reader = cmd.ExecuteReader();

while (reader.Read())
{
    Console.WriteLine($"{reader["id"]}: {reader["name"]}");
}

conn.Close();
```

## JDBC Driver

YamlDB includes a lightweight JDBC driver for read-only SQL access from Java and tools such as DBeaver. The driver can use bundled `yq`, `-Dyamldb.yq=/path/to/yq`, `YAMLDB_YQ`, or `yq` from `PATH`.

### Connection URL

```text
jdbc:yamldb:data.yaml
jdbc:yamldb:file:data.yaml
jdbc:yamldb:/path/to/yaml-directory
```

For DBeaver and similar JDBC tools, choose the YamlDB JDBC jar and use one of the URLs above. A single YAML file is exposed as table `data`; a directory exposes every `.yaml`/`.yml` file as a table named after the file stem. The JDBC driver provides table and column metadata for browsing directory sources.

### DBeaver Setup

1. Build or download `yamldb-jdbc.jar`.
2. In DBeaver, create a new Driver.
3. Add `yamldb-jdbc.jar` to the driver libraries.
4. Set the driver class to `io.github.markchenim.yamldb.jdbc.YamlDbJdbcDriver`.
5. Set the URL template to `jdbc:yamldb:{file}` or enter a full JDBC URL manually.
6. Use a JDBC URL such as `jdbc:yamldb:/home/alice/data.yaml` or `jdbc:yamldb:/home/alice/yaml-data`.
7. If DBeaver cannot connect, make sure the DBeaver process can find `yq`, or set a JVM property such as `-Dyamldb.yq=/opt/yamldb/yq`.
8. Test the connection, then browse tables or run SQL.

For a directory source like:

```text
yaml-data/
  users.yaml
  teams.yml
```

the visible tables are `users` and `teams`.

### Supported SQL

The JDBC driver supports the same small read-only SQL subset as the ODBC driver:

```sql
SELECT * FROM data
SELECT * FROM data WHERE city = 'Beijing'
SELECT * FROM data WHERE age >= 28
SELECT * FROM data WHERE city = 'Beijing' AND age >= 28
SELECT * FROM data WHERE city = 'Beijing' OR city = 'Shanghai'
SELECT * FROM users WHERE age >= 28
```

Nested YAML values such as arrays or objects are returned as JSON text through string getters.

The JDBC driver is read-only and supports the same SQL limits as ODBC: `SELECT * FROM <table>` plus optional `WHERE` comparisons joined by `AND` or `OR`.

### Build JDBC Jar

Windows:

```powershell
powershell -ExecutionPolicy Bypass -File jdbc\build.ps1
```

Linux/macOS:

```bash
bash jdbc/build.sh
```

Output:

```text
jdbc/target/yamldb-jdbc.jar
```

### Java Example

```java
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.Statement;

Class.forName("io.github.markchenim.yamldb.jdbc.YamlDbJdbcDriver");

try (Connection conn = DriverManager.getConnection("jdbc:yamldb:data.yaml");
     Statement stmt = conn.createStatement();
     ResultSet rs = stmt.executeQuery("SELECT * FROM data WHERE city = 'Beijing'")) {
    while (rs.next()) {
        System.out.println(rs.getString("id") + ": " + rs.getString("name"));
    }
}
```

## Data Format

### YAML

```yaml
- id: user1
  name: Alice
  age: 30
  city: Beijing
- id: user2
  name: Bob
  age: 25
  city: Shanghai
```

### JSON

```json
[
  {"id": "user1", "name": "Alice", "age": 30, "city": "Beijing"},
  {"id": "user2", "name": "Bob", "age": 25, "city": "Shanghai"}
]
```

## Error Handling

```rust
use yamldb::{YamlDb, YamlDbError};

match db.create(record) {
    Ok(_) => println!("Success"),
    Err(YamlDbError::DuplicateKey(id)) => eprintln!("Duplicate: {}", id),
    Err(YamlDbError::NotFound(id)) => eprintln!("Not found: {}", id),
    Err(YamlDbError::Io(e)) => eprintln!("IO error: {}", e),
    Err(YamlDbError::Yaml(e)) => eprintln!("YAML error: {}", e),
    Err(YamlDbError::Json(e)) => eprintln!("JSON error: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Project Structure

```
yamldb/
├── src/
│   ├── lib.rs      # Core library
│   ├── main.rs     # CLI tool
│   └── odbc.rs     # ODBC driver
├── tests/
│   ├── cli.rs          # CLI integration tests
│   ├── integration.rs  # Rust API integration tests
│   └── odbc.rs         # ODBC tests
├── jdbc/
│   ├── src/main/java   # JDBC driver
│   └── src/test/java   # JDBC smoke test
├── Cargo.toml
├── README.md
├── README.zh-CN.md
├── CHANGELOG.md
└── LICENSE
```

## License

MIT
