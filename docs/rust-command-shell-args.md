# Rust Command shell 参数 以及 `as_ref`和`&`的区别

`Command::new()` 只接收可执行程序，不会解析整条 shell 命令。

```rust
Command::new("sh")
    .args(["-c", input.shell.as_str()])
    .output()?;
```

Windows：

```rust
Command::new("cmd")
    .args(["/C", input.shell.as_str()])
    .output()?;
```

带 `args` 时：

```rust
let output = if let Some(args) = input.args.as_ref().filter(|args| !args.is_empty()) {
    Command::new(&input.shell)
        .args(args)
        .output()
        .map_err(|err| anyhow!(err))
} else {
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", input.shell.as_str()])
            .output()
            .map_err(|err| anyhow!(err))
    }

    #[cfg(not(windows))]
    {
        Command::new("sh")
            .args(["-c", input.shell.as_str()])
            .output()
            .map_err(|err| anyhow!(err))
    }
};
```

`as_ref()`：

```rust
Option<Vec<String>> -> Option<&Vec<String>>
```

`&input.args`：

```rust
Option<Vec<String>> -> &Option<Vec<String>>
```

`as_ref()` 适合直接借用里面的 `Vec`，避免 `unwrap()` 把值移动出来。
