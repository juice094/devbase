# GreptimeDB 集成计划

## 目标
将 devbase 的时序数据（health、code_metrics、stars）从 SQLite 单表迁移到 GreptimeDB，实现趋势分析、Burn-rate 监控与大时间窗口聚合。

## 架构原则
- **SQLite 保留为 Registry OLTP**：repo 元数据、关系图谱、vault notes 继续存 SQLite。
- **GreptimeDB 作为 OLAP 时序库**：health、metrics、stars 等时间序列数据双写。
- **Feature-gated**：`--features greptimedb` 才编译接入层，零成本抽象。
- **异步写入**：使用 `greptimedb-ingester` gRPC 客户端，批量异步提交。

## Schema 设计

### health_metrics
```sql
CREATE TABLE health_metrics (
    repo_id STRING,
    status STRING,
    ahead INT,
    behind INT,
    checked_at TIMESTAMP,
    TIME INDEX (checked_at),
    PRIMARY KEY (repo_id, checked_at)
);
```

### code_metrics
```sql
CREATE TABLE code_metrics (
    repo_id STRING,
    total_lines INT,
    source_lines INT,
    test_lines INT,
    comment_lines INT,
    file_count INT,
    language_breakdown STRING,
    updated_at TIMESTAMP,
    TIME INDEX (updated_at),
    PRIMARY KEY (repo_id, updated_at)
);
```

### stars_history
```sql
CREATE TABLE stars_history (
    repo_id STRING,
    stars INT,
    fetched_at TIMESTAMP,
    TIME INDEX (fetched_at),
    PRIMARY KEY (repo_id, fetched_at)
);
```

## 实施阶段

### Phase A: 基础架构（当前）
- [x] Cargo.toml feature gate + `greptimedb-ingester` 依赖
- [x] `GreptimeConfig` 配置结构
- [x] `src/greptime.rs` 空模块与连接管理

### Phase B: Health 双写 PoC
- [ ] `save_health` 后调用 `greptime::write_health`
- [ ] CLI `health` 命令增加 `--write-greptime` 标志

### Phase C: Metrics & Stars
- [ ] `run_metrics` 双写
- [ ] `github-info` stars 双写

### Phase D: 查询适配
- [ ] `query` 命令支持 `trend:` 前缀（从 GreptimeDB 读取时序趋势）
- [ ] Dashboard SQL 模板

## 兼容性
- 无 `greptimedb` feature 时，100% 保持现有 SQLite-only 行为。
- 连接失败时降级为仅 SQLite，打印 warning，不阻塞主流程。
