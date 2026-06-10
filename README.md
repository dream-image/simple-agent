# `simple-gent`

简单的ReAct agent，以验证、实践所学，同时进行rust实践。

# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

# 功能实现

> 前端代码由ai生成。还在学习rust中，因此手搓的rust代码有些bug没解决。


## 前端

- React + Ant Design X 聊天界面
- 多会话创建、删除、切换
- 普通会话和项目会话分组
- 项目会话支持选择本地目录
- 流式展示 assistant 回复
- 展示 DeepSeek `reasoning_content` 思考内容
- Markdown 渲染
- 代码高亮
- Mermaid 渲染
- Latex 公式渲染
- Think、Sources、自定义 icon 渲染
- 工具调用结果展示
- 工具参数、执行结果、成功/失败状态展示
- 工具权限弹窗
- 支持拒绝、允许、总是允许
- 权限模式切换
- token 用量显示
- 当前请求取消
- 自动滚动到底部
- 会话标题自动取第一条用户消息

## Rust

- Tauri 命令注册
- agent 初始化
- 会话创建和删除
- 会话上下文管理
- 工作区上下文管理
- system prompt 生成
- DeepSeek Chat Completions 调用
- DeepSeek SSE 流式响应解析
- assistant 内容聚合
- `reasoning_content` 聚合
- `tool_calls` 流式参数聚合
- 工具定义生成
- 工具注册表管理
- 工具执行调度
- `get_weather` 工具
- `read_file` 工具
- `edit_file` 工具
- `shell` 工具
- 工具权限模型
- 默认、自动编辑、计划模式、禁止询问、完全访问模式
- 前后端权限申请事件通信
- 工具执行结果回传前端
- 会话 token 统计
- 会话历史管理
- 会话历史写入 Tauri app data 目录
- 当前请求取消
- 基于 `watch::Sender<SessionStatus>` 的会话状态通知
