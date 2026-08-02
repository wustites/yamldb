# YamlDB 中文文档

[English](README.md)

[![CI](https://github.com/markchenim/yamldb/actions/workflows/ci.yml/badge.svg)](https://github.com/markchenim/yamldb/actions/workflows/ci.yml)
[![Release](https://github.com/markchenim/yamldb/actions/workflows/release.yml/badge.svg)](https://github.com/markchenim/yamldb/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

YamlDB 是一个轻量级 YAML 文件数据库，提供命令行工具、Rust API、实验性的 ODBC 驱动和 JDBC 驱动支持。它适合保存少量结构化数据、测试数据、配置型数据和需要人工可读/可编辑的数据文件。

## 功能特性

- **YAML 存储**：数据文件可读、可编辑，适合版本管理。
- **CRUD 操作**：支持创建、读取、更新、删除记录。
- **查询构造器**：支持等于、不等于、大小比较、字符串匹配、AND/OR/NOT。
- **模糊搜索**：支持大小写不敏感的关键字搜索。
- **导入导出**：支持 JSON 和 YAML。
- **备份**：可生成数据库快照。
- **CLI 工具**：可直接在命令行管理 YAML 数据。
- **ODBC 驱动**：支持通过简单 SQL 查询 YAML 数据。
- **JDBC 驱动**：支持 Java 通过 JDBC 读取 YAML 数据。

## 安装

YamlDB 使用 `yq` v4 解析和格式化 YAML 文件。发布包可以内置 `yq`；如果未内置，请安装 `yq` 命令，并确保它在 `PATH` 中。

示例：

```bash
# macOS
brew install yq

# 使用 Linux Homebrew
brew install yq

# 也可以从 https://github.com/mikefarah/yq 下载 yq v4 二进制文件
yq --version
```

`yq` 查找顺序：

1. `YAMLDB_YQ` 环境变量。
2. CLI/ODBC 驱动同目录下的 `yq`，或同目录 `bin/` 下的 `yq`。
3. JDBC JAR 内置资源 `bin/<os>-<arch>/yq`，例如 `bin/linux-amd64/yq`。
4. `PATH` 中的 `yq` 或 `yq.exe`。

### 通过 Cargo 安装

```bash
cargo install yamldb
```

### 从 GitHub Releases 下载

可以从 [GitHub Releases](https://github.com/markchenim/yamldb/releases) 下载预构建文件：

| 平台 | CLI | ODBC 驱动 | JDBC 驱动 |
| ---- | --- | --------- | --------- |
| Linux | `yamldb-linux` | `libyamldb-linux-odbc.so` | `yamldb-jdbc.jar` |
| Windows | `yamldb-windows.exe` | `yamldb-windows-odbc.dll` | `yamldb-jdbc.jar` |
| macOS | `yamldb-macos` | `libyamldb-macos-odbc.dylib` | `yamldb-jdbc.jar` |

## 数据源与表

YamlDB 在 CLI、Web UI、ODBC 和 JDBC 中使用一致的数据源映射。

| 数据源 | 表名 | 说明 |
| ------ | ---- | ---- |
| 单个 YAML 文件，例如 `data.yaml` | `data`；SQL 驱动也接受文件名表，例如 `data` | CLI 默认使用 `data` |
| 目录，例如 `./yaml-data` | 当前目录下每个 `.yaml/.yml` 文件一张表 | `users.yaml` 对应表 `users` |

目录数据源不递归扫描子目录。表文件需要直接放在所选目录下：

```text
yaml-data/
  users.yaml
  teams.yml
  projects.yaml
```

等价访问方式：

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

通过 CLI 或 Web UI 写入目录数据源时，如果表不存在，会创建为 `<table>.yaml`。

### 持久化保证

对于文件数据库，只有更新后的 YAML 成功写入后，修改才会提交到内存状态。YamlDB 会在目标目录写入并同步临时文件，然后原子替换数据库文件；如果发生 I/O 或 `yq` 错误，磁盘上原有文件和内存数据库状态都会保持不变。

该机制可避免单次 YamlDB 写入产生部分替换，但不提供多进程之间的锁或事务。存在多个写入进程时，调用方仍需自行协调。

### 能力对照

| 入口 | 单文件 | 目录多表 | 写入支持 | 表元数据 |
| ---- | ------ | -------- | -------- | -------- |
| CLI | 支持 | 支持，通过 `--table` 选择 | 支持 | `tables` 命令 |
| Web UI | 支持 | 支持，界面中选择表 | 支持 | 表选择器 |
| ODBC | 支持 | 支持，`SELECT * FROM <table>` | 只读 | `SQLTables` / `SQLColumns` |
| JDBC | 支持 | 支持，`SELECT * FROM <table>` | 只读 | `DatabaseMetaData` |

## CLI 用法

### 全局参数

```text
-f, --file <FILE>  指定 YAML 数据文件路径，默认 data.yaml
-t, --table <TABLE>  当 --file 指向 YAML 目录时选择表，默认 data
```

`--table` 是全局参数，可放在子命令前后。

### 记录操作

```bash
# 单文件数据源
yamldb create user1 --fields name=Alice,age=30,city=Beijing
yamldb get user1
yamldb list
yamldb list --limit 10
yamldb list --format json
yamldb update user1 --fields age=31,city=Guangzhou
yamldb delete user1

# 目录数据源
yamldb -f ./yaml-data tables
yamldb -f ./yaml-data -t users list
yamldb -f ./yaml-data -t users create user1 --fields name=Alice,age=30
yamldb -f ./yaml-data -t users get user1 --format json
yamldb -f ./yaml-data -t projects create p1 --fields name=Core
```

### 查询与搜索

```bash
# 等值查询
yamldb query --key city --value Beijing

# 比较操作
yamldb query --key age --value 25 --op gt
yamldb query --key age --value 30 --op gte
yamldb query --key age --value 50 --op lt
yamldb query --key age --value 25 --op lte
yamldb query --key city --value Beijing --op ne

# 字符串包含
yamldb query --key name --value Ali --op contains

# 模糊搜索
yamldb search --keyword alice
yamldb search --keyword alice --key name
```

等值和比较查询会把参数解析为 YAML 标量类型。例如，`--value 30` 匹配数字 `age: 30`，`--value true` 匹配布尔值。CLI 目前不提供类型覆盖参数；如果需要精确匹配数字或布尔形式的字符串，请使用 Rust API。

### 导入、导出与备份

```bash
# 导入
yamldb import -i users.json
yamldb import -i users.yaml
yamldb -f ./yaml-data -t users import -i users.yaml

# 导出
yamldb export -o backup.json
yamldb export -o backup.yaml --format yaml
yamldb -f ./yaml-data -t users export -o users.json

# 备份
yamldb backup -o backup.yaml
yamldb -f ./yaml-data -t users backup -o users-backup.yaml
```

### 工具命令

```bash
# 统计信息
yamldb stats
yamldb -f ./yaml-data -t users stats

# 记录数量
yamldb count
yamldb -f ./yaml-data -t users count

# 检查记录是否存在
yamldb exists user1
yamldb -f ./yaml-data -t users exists user1

# 清空数据库
yamldb clear --force
yamldb -f ./yaml-data -t users clear --force
```

### Web UI

为当前 YAML 文件启动本地浏览器管理界面：

```bash
yamldb -f data.yaml webui
yamldb -f /path/to/yaml-directory webui
yamldb -f data.yaml webui --host 127.0.0.1 --port 8080
```

Web UI 使用和 ODBC/JDBC 驱动一致的数据源映射。单个 YAML 文件显示为 `data` 表；目录会把每个 `.yaml/.yml` 文件显示为一张表，表名为文件名去掉扩展名。Web UI 支持对当前选中表进行列表、搜索、新建/更新和删除记录。

默认监听 `127.0.0.1:8080`，不提供内置认证；除非自行加访问控制，否则不要绑定到公网地址。

## Rust API

### 基本用法

```rust
use yamldb::{Record, YamlDb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = YamlDb::new("data.yaml");
    db.load()?;

    let mut record = Record::new("user1");
    record
        .set("name", "Alice")
        .set("age", 30)
        .set("city", "Beijing");
    db.create(record)?;

    let record = db.read("user1")?;
    println!("Name: {:?}", record.get_str("name"));
    println!("Age: {:?}", record.get_i64("age"));

    db.update_field("user1", "age", serde_yaml::Value::Number(31.into()))?;
    db.delete("user1")?;

    Ok(())
}
```

### 内存数据库

```rust
let mut db = YamlDb::memory();
let mut record = Record::new("test");
record.set("key", "value");
db.create(record)?;
```

### Record API

```rust
let mut record = Record::new("user1");

record
    .set("name", "Alice")
    .set("age", 30)
    .set("active", true);

record.get_str("name");
record.get_i64("age");
record.get_f64("score");
record.get_bool("active");

record.has_key("name");
record.keys();

let mut other = Record::new("user1");
other.set("email", "alice@example.com");
record.merge(&other);

let json = record.to_json()?;
```

### 查询构造器

```rust
use yamldb::{QueryOp, YamlDb};

let mut db = YamlDb::new("data.yaml");
db.load()?;

let result = db.query(&QueryOp::eq("city", "Beijing"));
let result = db.query(&QueryOp::ne("city", "Shanghai"));
let result = db.query(&QueryOp::gt("age", serde_yaml::Value::Number(25.into())));
let result = db.query(&QueryOp::gte("age", serde_yaml::Value::Number(30.into())));
let result = db.query(&QueryOp::lt("age", serde_yaml::Value::Number(50.into())));
let result = db.query(&QueryOp::lte("age", serde_yaml::Value::Number(25.into())));

let result = db.query(&QueryOp::contains("name", "Ali"));
let result = db.query(&QueryOp::starts_with("name", "Ali"));
let result = db.query(&QueryOp::ends_with("name", "Smith"));

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

### 搜索

```rust
let result = db.search("name", "alice");
let result = db.search_all("alice");
```

### QueryResult

```rust
let result = db.query(&QueryOp::eq("city", "Beijing"));

println!("Found: {}", result.count());
println!("Is empty: {}", result.is_empty());

if let Some(first) = result.first() {
    println!("First: {}", first.id);
}

let page1 = result.page(1, 10);
let page2 = result.page(2, 10);

let skipped = result.skip(5);
let limited = result.limit(10);
let ids = result.ids();

for record in result.iter() {
    println!("{}: {:?}", record.id, record.data);
}

let all = result.to_vec();
```

### 批量操作

```rust
use std::collections::HashMap;

let records = db.read_many(&["user1", "user2", "user3"]);

let updates = vec![
    (
        "user1".to_string(),
        HashMap::from([("age".to_string(), serde_yaml::Value::Number(31.into()))]),
    ),
    (
        "user2".to_string(),
        HashMap::from([("age".to_string(), serde_yaml::Value::Number(26.into()))]),
    ),
];
let updated = db.update_many(updates)?;

let deleted = db.delete_many(&["user1", "user2"])?;

let mut record = Record::new("user1");
record.set("name", "Alice");
db.upsert(record)?;
```

### 统计与备份

```rust
use std::path::Path;

let stats = db.stats();
println!("Total records: {}", stats.total_records);
println!("Unique keys: {:?}", stats.unique_keys);
println!("File size: {:?} bytes", stats.file_size);

db.backup(Path::new("backup.yaml"))?;
db.clear()?;
```

### 导入与导出

```rust
use std::path::Path;

let count = db.import_json(Path::new("users.json"))?;
let count = db.import_yaml(Path::new("users.yaml"))?;

db.export_json(Path::new("backup.json"))?;
db.export_yaml(Path::new("backup.yaml"))?;
```

## ODBC 驱动

YamlDB 提供 ODBC 驱动，可供 ODBC 客户端通过只读 SQL 访问 YAML 数据。

### 连接字符串

```text
DRIVER={YamlDB};DBQ=data.yaml;
DRIVER={YamlDB};FILE=data.yaml;
DRIVER={YamlDB};DBQ=/path/to/yaml-directory;
```

当 `DBQ`/`FILE` 指向单个 YAML 文件时，使用表名 `data` 查询。指向目录时，每个 `.yaml` 或 `.yml` 文件会作为一张表，表名为文件名去掉扩展名，例如 `users.yaml` 对应表 `users`。

### 表映射规则

| 数据源路径 | 暴露的表 | SQL 示例 |
| ---------- | -------- | -------- |
| `data.yaml` | `data` | `SELECT * FROM data` |
| `users.yaml` | `data`、`users` | `SELECT * FROM users` |
| `/path/to/yaml-directory` | 每个 `.yaml/.yml` 文件一张表 | `SELECT * FROM users` |

目录数据源只扫描当前目录下的 YAML 文件，不递归扫描子目录。

### 支持的 SQL

```sql
SELECT * FROM data
SELECT * FROM data WHERE city = 'Beijing'
SELECT * FROM data WHERE age > 25
SELECT * FROM data WHERE age >= 28
SELECT * FROM data WHERE age < 30
SELECT * FROM data WHERE age <= 25
SELECT * FROM data WHERE city != 'Shanghai'
SELECT * FROM data WHERE city = 'Beijing' AND age >= 28
SELECT * FROM data WHERE city = 'Beijing' OR city = 'Shanghai'
SELECT * FROM users WHERE age >= 28
```

SQL 支持范围是有意保持很小的只读子集：`SELECT * FROM <table>`，可选 `WHERE` 条件，条件支持比较操作并可用 `AND` 或 `OR` 连接。驱动只支持 `SELECT *`；不支持选择部分列、join、group by、order by、insert、update、delete。写入请使用 CLI 或 Web UI。

### 构建共享库

```bash
cargo build --release
```

输出文件：

- Windows：`target/release/yamldb.dll`
- Linux：`target/release/libyamldb.so`
- macOS：`target/release/libyamldb.dylib`

### 注册驱动

Windows：

1. 打开 ODBC 数据源管理器。
2. 进入 Drivers 选项卡。
3. 点击 Add。
4. 选择 `yamldb.dll`。

Linux：

在 `/etc/odbcinst.ini` 中添加：

```ini
[YamlDB]
Description=YamlDB ODBC Driver
Driver=/path/to/libyamldb.so
```

### DBeaver / ODBC 客户端

当客户端支持原生 ODBC 连接时，可以使用 ODBC 驱动：

1. 先在系统 ODBC 管理器中注册 YamlDB ODBC 共享库。
2. 创建 DSN 或连接，驱动使用 `DRIVER={YamlDB}`。
3. 将 `DBQ` 或 `FILE` 设置为单个 YAML 文件，或包含多个 YAML 文件的目录。
4. 单文件数据源查询 `data` 表；目录数据源查询对应文件名的表。

目录 DSN 示例：

```text
DRIVER={YamlDB};DBQ=/home/alice/yaml-data;
```

### Python 示例

```python
import pyodbc

conn = pyodbc.connect('DRIVER={YamlDB};DBQ=data.yaml;')
cursor = conn.cursor()

cursor.execute("SELECT * FROM data WHERE city = 'Beijing'")
for row in cursor:
    print(row)

conn.close()
```

### C# 示例

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

## JDBC 驱动

YamlDB 提供轻量级 JDBC 驱动，可供 Java 和 DBeaver 等工具通过只读 SQL 访问 YAML 数据。驱动可使用内置 `yq`、`-Dyamldb.yq=/path/to/yq`、`YAMLDB_YQ`，或 `PATH` 中的 `yq`。

### 连接 URL

```text
jdbc:yamldb:data.yaml
jdbc:yamldb:file:data.yaml
jdbc:yamldb:/path/to/yaml-directory
```

在 DBeaver 等 JDBC 工具中，选择 YamlDB JDBC jar，并使用上面的 URL。单个 YAML 文件暴露为 `data` 表；目录会把每个 `.yaml`/`.yml` 文件暴露为一张表。JDBC 驱动提供表和列元数据，便于在工具中浏览目录数据源。

### DBeaver 配置

1. 构建或下载 `yamldb-jdbc.jar`。
2. 在 DBeaver 中创建新的 Driver。
3. 将 `yamldb-jdbc.jar` 添加到驱动库。
4. Driver class 填写 `io.github.markchenim.yamldb.jdbc.YamlDbJdbcDriver`。
5. URL template 可填写 `jdbc:yamldb:{file}`，也可以手动输入完整 JDBC URL。
6. JDBC URL 填写 `jdbc:yamldb:/home/alice/data.yaml` 或 `jdbc:yamldb:/home/alice/yaml-data`。
7. 如果 DBeaver 无法连接，确认 DBeaver 进程能找到 `yq`，或设置 JVM 参数，例如 `-Dyamldb.yq=/opt/yamldb/yq`。
8. 测试连接后，即可浏览表或执行 SQL。

如果目录结构如下：

```text
yaml-data/
  users.yaml
  teams.yml
```

可见表为 `users` 和 `teams`。

### 支持的 SQL

JDBC 驱动支持和 ODBC 类似的只读 SQL 子集：

```sql
SELECT * FROM data
SELECT * FROM data WHERE city = 'Beijing'
SELECT * FROM data WHERE age >= 28
SELECT * FROM data WHERE city = 'Beijing' AND age >= 28
SELECT * FROM data WHERE city = 'Beijing' OR city = 'Shanghai'
SELECT * FROM users WHERE age >= 28
```

YAML 中的数组、对象等嵌套值会通过字符串读取接口以 JSON 文本返回。

JDBC 驱动是只读的，SQL 限制和 ODBC 一致：`SELECT * FROM <table>`，可选 `WHERE` 比较条件，并可用 `AND` 或 `OR` 连接。

### 构建 JDBC Jar

Windows：

```powershell
powershell -ExecutionPolicy Bypass -File jdbc\build.ps1
```

Linux/macOS：

```bash
bash jdbc/build.sh
```

输出文件：

```text
jdbc/target/yamldb-jdbc.jar
```

### Java 示例

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

## 数据格式

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

## 错误处理

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

## 项目结构

```text
yamldb/
├── src/
│   ├── lib.rs      # 核心库
│   ├── main.rs     # CLI 工具
│   └── odbc.rs     # ODBC 驱动
├── tests/
│   ├── cli.rs          # CLI 集成测试
│   ├── integration.rs  # Rust API 集成测试
│   └── odbc.rs         # ODBC 测试
├── examples/
│   └── odbc_usage.rs
├── jdbc/
│   ├── src/main/java   # JDBC 驱动
│   └── src/test/java   # JDBC 冒烟测试
├── Cargo.toml
├── README.md
├── README.zh-CN.md
├── CHANGELOG.md
└── LICENSE
```

## 注意事项

- YamlDB 适合轻量数据、配置型数据和测试数据，不适合作为高并发或大规模数据库。
- YAML 文件便于人工编辑，但并发写入需要由调用方控制。
- ODBC SQL 支持的是项目内实现的简单子集，不等同于完整 SQL 数据库。
- JDBC SQL 支持的是项目内实现的简单只读子集，不等同于完整 SQL 数据库。

## 许可证

MIT
