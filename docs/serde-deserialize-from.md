# Deserialize 和 From 的关系

`Deserialize` 负责把外部数据解析成 Rust 类型。

例如：

```rust
let delta = serde_json::from_str::<Delta>(json)?;
```

这里会调用 `Delta` 的 `Deserialize`，完成：

```text
JSON -> Delta
```

`From<DeltaRaw> for Delta` 负责 Rust 类型之间的转换。

例如：

```rust
let delta: Delta = raw.into();
```

这里不会解析 JSON，只完成：

```text
DeltaRaw -> Delta
```

当 JSON 结构比较宽松，但业务类型需要明确分类时，可以拆成两步：

```text
JSON -> DeltaRaw -> Delta
```

第一步让 serde 解析成宽松结构：

```rust
#[derive(Deserialize)]
struct DeltaRaw {
    role: Option<Role>,
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolDefine>>,
}
```

第二步用 `From` 写业务判断：

```rust
impl From<DeltaRaw> for Delta {
    fn from(raw: DeltaRaw) -> Self {
        if let Some(tool_calls) = raw.tool_calls {
            if !tool_calls.is_empty() {
                return Delta::ToolCalls(tool_calls);
            }
        }

        if let Some(reasoning_content) = raw.reasoning_content {
            if !reasoning_content.is_empty() {
                return Delta::Thinking(ThinkingDelta { reasoning_content });
            }
        }

        if let Some(content) = raw.content {
            if !content.is_empty() {
                return Delta::Content(ContentDelta { content });
            }
        }

        if let Some(role) = raw.role {
            return Delta::SessionStart(SessionStart { role });
        }

        Delta::Empty
    }
}
```

如果给 `Delta` 加：

```rust
#[serde(from = "DeltaRaw")]
enum Delta {
    Empty,
    SessionStart(SessionStart),
    Thinking(ThinkingDelta),
    Content(ContentDelta),
    ToolCalls(Vec<ToolDefine>),
}
```

serde 会自动执行：

```text
JSON -> DeltaRaw -> Delta
```

前提是：

```text
DeltaRaw: Deserialize
Delta: From<DeltaRaw>
```

结论：`Deserialize` 解决“外部数据怎么进来”，`From` 解决“内部类型怎么转换”。`From` 不能替代 `Deserialize`，但可以被 `Deserialize` 借用来完成最终转换。
