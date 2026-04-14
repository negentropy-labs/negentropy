# Negentropy — main vs claude 分支架构对比

## main 分支架构图 (V2 MVP)

```
                          main.rs
                            │
               ┌────────────┼────────────────┐
               ▼            ▼                ▼
            cli.rs      discovery.rs     report/mod.rs
          (clap CLI)   (文件发现扫描)    (Report + Table 渲染)
               │
               ▼
          ┌─── analyze() ───┐
          │                  │
          ▼                  ▼
    facts/mod.rs        graph/mod.rs
    (事实提取)          (图算法)
          │                  │
          ▼                  ▼
    parser/mod.rs       petgraph
    (tree-sitter)      (SCC + TCR)
          │
          ▼
  tree-sitter-javascript
          │
          ▼
    metrics/mod.rs ←── facts + graph
    (所有指标集中计算)
          │
          ▼
      model.rs
  (Dimension, Hotspot, RiskLevel)
```

### 关键设计特征
- **单体 `FileFacts` struct**: 所有事实一次提取到一个大 struct 里
- **集中式指标计算**: 7 个指标全部写在 `metrics/mod.rs` 一个 300+ 行文件中
- **无 trait 抽象**: 没有 `LanguageSupport` 或 `Metric` trait
- **硬编码 JavaScript**: parser 固定使用 `tree-sitter-javascript`
- **手动 AST 遍历**: 用 `walk_named_nodes` 回调遍历，不使用 Tree-sitter Query DSL
- **子命令模式**: `negentropy analyze <path>` (clap Subcommand)

---

## claude 分支架构图 (Two-Phase Extraction)

```
                          main.rs
                            │
                            ▼
                        clap CLI
                            │
               ┌────────────┼────────────┐
               ▼            ▼            ▼
         lang/mod.rs   metric/mod.rs  report/mod.rs
     (LanguageSupport   (Metric trait  (Report +
      trait + 文件扫描)  + build 工厂)  Diagnostic)
           │                │              │
           ▼                ▼              ├──► json.rs
    lang/typescript.rs    ┌─┴─┐           └──► terminal.rs
    (Query strings ×8)    │   │
                          ▼   ▼
                   ┌──────────────────────┐
                   │  7 独立 Metric 实现   │
                   │  每个持有自己的 Query  │
                   ├──────────────────────┤
                   │ plme.rs  sse.rs      │
                   │ tce.rs   tcr.rs      │
                   │ edr.rs   iie.rs      │
                   │ ead.rs               │
                   └──────────────────────┘
                          │
            Phase 1       │       Phase 2
     LanguageSupport ─►  ParsedFile ─► Metric.analyze()
      .parse()         (AST+源码)     (自有 Query 提取+计算)
```

### 关键设计特征
- **Two-Phase Extraction**: 解析与提取解耦，Metric 自包含
- **Trait 抽象**: `LanguageSupport` trait (语言扩展点) + `Metric` trait (指标扩展点)
- **每个 Metric 独占 Query**: 构造时注入预编译 Query，所有权清晰
- **Tree-sitter Query DSL**: 用 `.scm` 模式声明式提取，非手动遍历
- **TypeScript 专用**: 使用 `tree-sitter-typescript`
- **直接命令**: `negentropy <path>` (无子命令)

---

## 逐维度对比

| 维度 | main 分支 | claude 分支 | 对比分析 |
|------|----------|------------|---------|
| **IIE (模块抽象度)** | `metrics/mod.rs` 300+ 行集中计算，是典型浅模块 | 每个指标独立文件，接口小 (trait 2 methods)，实现深 | claude 更符合 Deep Module 原则 |
| **EAD (逻辑内聚度)** | `compute_metrics()` 函数同时访问 `FileFacts` 全部字段 — Feature Envy | 每个 Metric 只访问自己持有的 Query + ParsedFile 公共接口 | claude 消除了 Feature Envy |
| **TCR (波及效应)** | 改动 `FileFacts` struct → 影响 `facts/`, `metrics/`, `graph/` 三个模块 | 改动某个 Metric 不影响其他 Metric，ParsedFile 极简 | claude 波及半径更小 |
| **TCE (架构解耦)** | `metrics/mod.rs` ↔ `facts/mod.rs` ↔ `graph/mod.rs` 三方紧耦合 | Metric 之间无依赖，唯一共享是 `tce::build_import_graph` 被 TCR 复用 | claude 耦合度更低 |
| **EDR (显式依赖)** | 指标函数直接硬编码访问 facts struct 字段 | Metric 通过构造函数注入 Query (依赖注入) | claude EDR 更高 |
| **PLME (意图冗余)** | 无深层路径，结构扁平 | 同样扁平 | 持平 |
| **SSE+OA (状态封装)** | `FileFacts` 是 God struct，多处读写 | 每个 Metric 独占 Query 实例，无共享可变状态 | claude 所有权更清晰 |

## 语言扩展性对比

| 方面 | main | claude |
|------|------|--------|
| 新增语言需改动 | 改 `parser/mod.rs` + 改 `facts/mod.rs` 全部提取逻辑 | 实现 `LanguageSupport` trait (1 个文件) |
| 解析器 | 硬编码 `tree-sitter-javascript` | 通过 trait 动态选择 |
| 查询方式 | 手动 `walk_named_nodes` 回调 | Tree-sitter Query DSL (声明式) |
| 支持语言 | JS/TS (共用 JS parser) | TypeScript (专用 TS parser) |

## 各自优势

### main 分支的优势
- 更成熟的 **CLI 设计**: 子命令、`--fail-on`、`--extensions`、`--output` 文件输出
- 更完善的 **风险等级体系**: `RiskLevel::Low/Medium/High` + `risk_ascending/descending`
- 更好的 **统计聚合**: `median()`, `percentile()` 用于项目级评分
- 完整的 **测试体系**: `tests/dimension_fixtures.rs` + `tests/integration_cli.rs`
- **导入路径解析更健壮**: 使用文件系统 `exists()` 验证

### claude 分支的优势
- 更符合 dialog.md 推崇的 **架构品味**: trait 抽象、深模块、所有权清晰
- **语言扩展性**: 新增语言只需实现一个 trait
- **指标独立性**: 每个 Metric 完全自包含，可独立开发/测试
- **声明式查询**: Tree-sitter Query DSL 比手动遍历更易维护
- 更丰富的 **终端输出**: 彩色 severity 标签、actionable suggestions

## 融合建议

最优路径是 **以 claude 分支的架构为骨架，吸收 main 分支的产品成熟度**：

1. **保留** claude 的 trait 体系 (`LanguageSupport` + `Metric`) 和 two-phase extraction
2. **移植** main 的 CLI 设计 (`--fail-on`, `--extensions`, `--output`)
3. **移植** main 的 `RiskLevel` 体系和 `risk_ascending/descending` 阈值函数
4. **移植** main 的 `median()` / `percentile()` 统计聚合（用于项目级评分）
5. **移植** main 的测试体系 (`dimension_fixtures` + `integration_cli`)
6. **保留** claude 的 Tree-sitter Query DSL (比 `walk_named_nodes` 更声明式)
7. **融合** 导入解析: 用 main 的 `exists()` 验证 + claude 的 `normalize_path()` 兜底
