import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { Button, ConfigProvider, Dropdown, Empty, Modal, Popover } from "antd";
import type { MenuProps } from "antd";
import {
  DownOutlined,
  EditOutlined,
  FileSearchOutlined,
  SafetyOutlined,
  StopOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { Bubble, Conversations, Sender, Think, XProvider } from "@ant-design/x";
import type { BubbleItemType, ConversationItemType } from "@ant-design/x";
import { XMarkdown } from "@ant-design/x-markdown";
// import { Streamdown } from "streamdown";
import "./App.css";

type AgentReply = {
  content: string;
  reasoning_content?: string | null;
  session_context?: SessionContext | null;
};

type SessionContext = {
  token: number;
  total_token: number;
  mode: PermissionMode;
};

type PermissionMode =
  | "Default"
  | "AcceptEdits"
  | "Plan"
  | "DontAsk"
  | "BypassPermissions";

type AgentStreamEvent = {
  sessionId: string;
  data: string;
};

type AgentToolCallEvent = {
  sessionId: string;
  id: string;
  name: string;
  arguments: string;
  content: string;
  success: boolean;
};

type PermissionApplyEvent = {
  name: string;
  content: string;
  session_id: string;
};

type PermissionRequest = PermissionApplyEvent & {
  sessionId: string;
  toolId: string;
};

type ToolCallMessage = {
  id: string;
  name: string;
  arguments: string;
  content: string;
  success: boolean;
};

type AssistantMessagePart =
  | {
      type: "text";
      id: string;
      content: string;
    }
  | {
      type: "tool";
      id: string;
      toolCall: ToolCallMessage;
    };

type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoningContent?: string;
  toolCalls?: ToolCallMessage[];
  parts?: AssistantMessagePart[];
  loading?: boolean;
  streaming?: boolean;
  error?: boolean;
};

type ChatSession = {
  key: string;
  label: string;
  messages: ChatMessage[];
  permissionMode: PermissionMode;
  sessionContext?: SessionContext | null;
};

const makeId = () => `${Date.now()}-${Math.random().toString(36).slice(2)}`;

const createSession = (): ChatSession => ({
  key: makeId(),
  label: "新会话",
  messages: [],
  permissionMode: "Default",
});

const permissionModeOptions = [
  {
    key: "Default",
    label: "默认",
    icon: <SafetyOutlined />,
    color: "#2563eb",
    border: "#93c5fd",
    background: "#eff6ff",
  },
  {
    key: "AcceptEdits",
    label: "自动编辑",
    icon: <EditOutlined />,
    color: "#079455",
    border: "#75e0a7",
    background: "#ecfdf3",
  },
  {
    key: "Plan",
    label: "计划模式",
    icon: <FileSearchOutlined />,
    color: "#7a5af8",
    border: "#c3b5fd",
    background: "#f4f3ff",
  },
  {
    key: "DontAsk",
    label: "禁止询问",
    icon: <StopOutlined />,
    color: "#d92d20",
    border: "#fda29b",
    background: "#fffbfa",
  },
  {
    key: "BypassPermissions",
    label: "完全访问",
    icon: <ThunderboltOutlined />,
    color: "#e35300",
    border: "#fd853a",
    background: "#fff6ed",
  },
];

const getPermissionModeOption = (mode: PermissionMode) =>
  permissionModeOptions.find((item) => item.key === mode) ??
  permissionModeOptions[0];

const getPermissionModeLabel = (mode: PermissionMode) =>
  getPermissionModeOption(mode).label;

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

type PermissionLogEntry = {
  time: string;
  event: string;
  detail?: unknown;
};

type PermissionLogWindow = typeof window & {
  __permissionLogs?: () => PermissionLogEntry[];
  __clearPermissionLogs?: () => void;
};

const PERMISSION_LOG_KEY = "simple-agent.permissionLogs";

const readPermissionLogs = (): PermissionLogEntry[] => {
  try {
    const logs = JSON.parse(localStorage.getItem(PERMISSION_LOG_KEY) ?? "[]");
    return Array.isArray(logs) ? (logs as PermissionLogEntry[]) : [];
  } catch {
    return [];
  }
};

const logPermission = (event: string, detail?: unknown) => {
  const entry = {
    time: new Date().toISOString(),
    event,
    detail,
  };

  console.info(`[permission] ${event}`, detail ?? "");

  try {
    localStorage.setItem(
      PERMISSION_LOG_KEY,
      JSON.stringify([...readPermissionLogs(), entry].slice(-200)),
    );
  } catch {
    return;
  }
};

const getToolSummary = (content: string) => {
  const text = content.replace(/\s+/g, " ").trim();
  if (!text) {
    return "无输出";
  }
  return text.length > 140 ? `${text.slice(0, 140)}...` : text;
};

const getMessageParts = (message: ChatMessage): AssistantMessagePart[] =>
  message.parts ??
  (message.content
    ? [{ type: "text", id: `${message.id}-text`, content: message.content }]
    : []);

const appendTextPart = (
  message: ChatMessage,
  content: string,
): AssistantMessagePart[] => {
  const parts = getMessageParts(message);
  const lastPart = parts[parts.length - 1];

  if (lastPart?.type === "text") {
    return [
      ...parts.slice(0, -1),
      { ...lastPart, content: lastPart.content + content },
    ];
  }

  return [...parts, { type: "text", id: makeId(), content }];
};

const upsertToolPart = (
  message: ChatMessage,
  toolCall: ToolCallMessage,
): AssistantMessagePart[] => {
  const parts = getMessageParts(message);
  const index = parts.findIndex(
    (part) => part.type === "tool" && part.toolCall.id === toolCall.id,
  );
  const toolPart: AssistantMessagePart = {
    type: "tool",
    id: `tool-${toolCall.id || makeId()}`,
    toolCall,
  };

  if (index >= 0) {
    return [
      ...parts.slice(0, index),
      toolPart,
      ...parts.slice(index + 1),
    ];
  }

  return [...parts, toolPart];
};

const renderToolCall = (toolCall: ToolCallMessage) => (
  <details className="tool-call" key={toolCall.id}>
    <summary>
      <span className="tool-call-name">{toolCall.name}</span>
      <span
        className={
          toolCall.success ? "tool-call-status success" : "tool-call-status error"
        }
      >
        {toolCall.success ? "成功" : "失败"}
      </span>
      <span className="tool-call-summary">
        {getToolSummary(toolCall.content)}
      </span>
    </summary>

    <div className="tool-call-detail">
      <div className="tool-call-section">
        <div className="tool-call-label">参数</div>
        <pre>{toolCall.arguments}</pre>
      </div>
      <div className="tool-call-section">
        <div className="tool-call-label">结果</div>
        <pre>{toolCall.content}</pre>
      </div>
    </div>
  </details>
);

const isScrollAtBottom = (element: HTMLDivElement) =>
  element.scrollHeight - element.scrollTop - element.clientHeight < 8;

const getTokenPercent = (context?: SessionContext | null) => {
  if (!context?.total_token) {
    return 0;
  }

  return Math.min(100, Math.round((context.token / context.total_token) * 100));
};

function App() {
  const [firstSession] = useState(createSession);
  const [sessions, setSessions] = useState<ChatSession[]>([firstSession]);
  const [activeKey, setActiveKey] = useState(firstSession.key);
  const [input, setInput] = useState("");
  const [loadingSession, setLoadingSession] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [bootError, setBootError] = useState("");
  const [permissionQueue, setPermissionQueue] = useState<PermissionRequest[]>([]);
  const containRef = useRef<HTMLDivElement | null>(null);
  const streamTargetRef = useRef<Record<string, string>>({});
  const permissionListenersRef = useRef<Record<string, () => void>>({});
  const shouldAutoScrollRef = useRef(true);

  useEffect(() => {
    const navigation = performance.getEntriesByType(
      "navigation",
    )[0] as PerformanceNavigationTiming | undefined;
    const target = window as PermissionLogWindow;
    const handlePageHide = (event: PageTransitionEvent) => {
      logPermission("pagehide", { persisted: event.persisted });
    };
    const handleBeforeUnload = () => {
      logPermission("beforeunload");
    };
    const handleError = (event: ErrorEvent) => {
      logPermission("window error", {
        message: event.message,
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
      });
    };
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      logPermission("unhandled rejection", {
        reason: getErrorMessage(event.reason),
      });
    };

    target.__permissionLogs = readPermissionLogs;
    target.__clearPermissionLogs = () => {
      localStorage.removeItem(PERMISSION_LOG_KEY);
    };
    logPermission("app mounted", { navigationType: navigation?.type });
    window.addEventListener("pagehide", handlePageHide);
    window.addEventListener("beforeunload", handleBeforeUnload);
    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    import.meta.hot?.on("vite:beforeFullReload", (payload) => {
      logPermission("vite beforeFullReload", payload);
    });
    import.meta.hot?.on("vite:beforeUpdate", (payload) => {
      logPermission("vite beforeUpdate", payload);
    });

    return () => {
      window.removeEventListener("pagehide", handlePageHide);
      window.removeEventListener("beforeunload", handleBeforeUnload);
      window.removeEventListener("error", handleError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
    };
  }, []);

  useEffect(() => {
    let alive = true;

    async function init() {
      try {
        await invoke("agent_init");
        await invoke("create_session", { sessionId: firstSession.key });
        if (alive) {
          setReady(true);
        }
      } catch (error) {
        if (alive) {
          setBootError(getErrorMessage(error));
        }
      }
    }

    init();

    return () => {
      alive = false;
    };
  }, [firstSession.key]);

  const activeSession = sessions.find((item) => item.key === activeKey);
  const tokenPercent = getTokenPercent(activeSession?.sessionContext);
  const tokenRingStyle = {
    "--token-percent": `${tokenPercent}%`,
  } as CSSProperties;
  useLayoutEffect(() => {
    shouldAutoScrollRef.current = true;
    containRef.current?.scrollTo(0, containRef.current.scrollHeight);
  }, [activeKey]);
  useLayoutEffect(() => {
    if (shouldAutoScrollRef.current) {
      containRef.current?.scrollTo(0, containRef.current.scrollHeight);
    }
  }, [activeSession?.messages]);
  const conversationItems = useMemo<ConversationItemType[]>(
    () => sessions.map(({ key, label }) => ({ key, label })),
    [sessions],
  );

  const bubbleItems = useMemo<BubbleItemType[]>(
    () =>
      (activeSession?.messages ?? []).map((message) => {
        const hasVisibleContent = Boolean(
          message.content ||
            message.reasoningContent ||
            message.parts?.length ||
            message.toolCalls?.length,
        );
        const loading = Boolean(message.loading && !hasVisibleContent);
        const streaming = Boolean(message.streaming);

        return {
          key: message.id,
          role: message.role,
          content: message.content,
          streaming,
          loading,
          status: message.error ? "error" : loading ? "loading" : "success",
          extraInfo: {
            reasoningContent: message.reasoningContent,
            toolCalls: message.toolCalls,
            parts: message.parts,
            streaming,
          },
        };
      }),
    [activeSession?.messages],
  );

  const updateMessage = (
    sessionId: string,
    messageId: string,
    updater: (message: ChatMessage) => ChatMessage,
  ) => {
    setSessions((current) =>
      current.map((session) =>
        session.key === sessionId
          ? {
              ...session,
              messages: session.messages.map((message) =>
                message.id === messageId ? updater(message) : message,
              ),
            }
          : session,
      ),
    );
  };

  const handleMessageAreaScroll = () => {
    const element = containRef.current;
    if (element) {
      shouldAutoScrollRef.current = isScrollAtBottom(element);
    }
  };

  const cleanupPermissionListeners = (sessionId?: string) => {
    Object.entries(permissionListenersRef.current).forEach(([key, unlisten]) => {
      if (!sessionId || key.startsWith(`${sessionId}_`)) {
        unlisten();
        delete permissionListenersRef.current[key];
      }
    });
  };

  const cleanupPermissionListener = (key: string) => {
    permissionListenersRef.current[key]?.();
    delete permissionListenersRef.current[key];
  };

  const listenPermissionRequest = async (sessionId: string, toolId: string) => {
    const key = `${sessionId}_${toolId}`;

    if (permissionListenersRef.current[key]) {
      logPermission("listener exists", { sessionId, toolId });
      return;
    }

    logPermission("listen", { sessionId, toolId });
    const unlisten = await listen<PermissionApplyEvent>(
      `apply_permission_${sessionId}_${toolId}`,
      (event) => {
        const payload = event.payload;
        logPermission("apply received", {
          sessionId: payload.session_id || sessionId,
          toolId,
          name: payload.name,
        });
        setPermissionQueue((current) => [
          ...current,
          {
            ...payload,
            sessionId: payload.session_id || sessionId,
            toolId,
          },
        ]);
        cleanupPermissionListener(key);
      },
    );

    permissionListenersRef.current[key] = unlisten;
  };

  useEffect(() => {
    let disposed = false;
    const unlisteners: (() => void)[] = [];

    const addUnlistener = (unlisten: () => void) => {
      if (disposed) {
        unlisten();
        return;
      }

      unlisteners.push(unlisten);
    };

    async function bindEvents() {
      addUnlistener(await listen<AgentStreamEvent>("agent_stream", (event) => {
        const { sessionId, data } = event.payload;
        const messageId = streamTargetRef.current[sessionId];

        if (!messageId || data === "[DONE]") {
          return;
        }

        try {
          const chunk = JSON.parse(data);
          const delta = chunk?.choices?.[0]?.delta;
          const content =
            typeof delta?.content === "string" ? delta.content : "";
          const reasoningContent =
            typeof delta?.reasoning_content === "string"
              ? delta.reasoning_content
              : "";
          const toolCalls = Array.isArray(delta?.tool_calls)
            ? delta.tool_calls
            : [];

          toolCalls.forEach((item) => {
            const toolId = typeof item?.id === "string" ? item.id : "";

            if (toolId) {
              void listenPermissionRequest(sessionId, toolId);
            }
          });

          if (!content && !reasoningContent) {
            return;
          }

          updateMessage(sessionId, messageId, (message) => ({
            ...message,
            content: message.content + content,
            reasoningContent: `${message.reasoningContent ?? ""}${reasoningContent}`,
            parts: content ? appendTextPart(message, content) : message.parts,
            loading: false,
            streaming: true,
          }));
        } catch {
          return;
        }
      }));

      addUnlistener(await listen<AgentToolCallEvent>(
        "agent_tool_call",
        (event) => {
          const payload = event.payload;
          const messageId = streamTargetRef.current[payload.sessionId];

          if (!messageId) {
            return;
          }

          updateMessage(payload.sessionId, messageId, (message) => {
            const toolCalls = [...(message.toolCalls ?? [])];
            const index = toolCalls.findIndex((item) => item.id === payload.id);
            const toolCall = {
              id: payload.id,
              name: payload.name,
              arguments: payload.arguments,
              content: payload.content,
              success: payload.success,
            };

            if (index >= 0) {
              toolCalls[index] = toolCall;
            } else {
              toolCalls.push(toolCall);
            }

            return {
              ...message,
              toolCalls,
              parts: upsertToolPart(message, toolCall),
              loading: false,
              streaming: true,
            };
          });
        },
      ));
    }

    bindEvents();

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
      cleanupPermissionListeners();
    };
  }, []);

  const replyPermission = async (result: "AlwaysAllow" | "Allow" | "Deny") => {
    const request = permissionQueue[0];

    if (!request) {
      logPermission("reply without request", { result });
      return;
    }

    logPermission("reply", {
      sessionId: request.sessionId,
      toolId: request.toolId,
      name: request.name,
      result,
    });
    await emit(`permission_callback_${request.sessionId}_${request.toolId}`, {
      result,
      reason: result === "Deny" ? "用户拒绝授权" : "",
    });
    setPermissionQueue((current) => current.slice(1));
  };

  const permissionRequest = permissionQueue[0] ?? null;
  const activePermissionMode = activeSession?.permissionMode ?? "Default";
  const activePermissionModeOption =
    getPermissionModeOption(activePermissionMode);
  const permissionModeMenu: MenuProps = {
    selectedKeys: [activePermissionMode],
    items: permissionModeOptions.map((item) => ({
      key: item.key,
      label: (
        <span
          className="permission-mode-menu-item"
          style={{ "--permission-mode-color": item.color } as CSSProperties}
        >
          {item.icon}
          <span>{item.label}</span>
        </span>
      ),
    })),
    onClick: ({ key }) => handlePermissionModeChange(key as PermissionMode),
  };

  const handleMockMessages = () => {
    const sessionId = activeKey;
    const lorem =
      "这是一段很长的模拟文本，用来撑开消息高度，让滚动距离变得足够大。" +
      "当 useEffect 在浏览器绘制之后才执行滚动时，你会先看到顶部的内容，" +
      "然后瞬间跳到底部——这就是所谓的闪烁效果。" +
      "useLayoutEffect 则会在绘制前同步执行滚动，不会有这个闪烁。";

    const mockMessages: ChatMessage[] = Array.from({ length: 6 }, (_, i) => ({
      id: makeId(),
      role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
      content: `[Mock #${i + 1}] ${lorem}\n\n---\n\n${lorem}`,
      reasoningContent: i % 2 === 1 ? `思考过程 #${i + 1}: ${lorem}` : undefined,
    }));

    setSessions((current) =>
      current.map((session) =>
        session.key === sessionId
          ? {
              ...session,
              messages: [...session.messages, ...mockMessages],
            }
          : session,
      ),
    );
  };

  const handleCreateSession = async () => {
    const session = createSession();
    setSessions((current) => [session, ...current]);
    setActiveKey(session.key);

    try {
      await invoke("create_session", { sessionId: session.key });
    } catch (error) {
      setBootError(getErrorMessage(error));
    }
  };

  const handleDeleteSession = async (sessionId: string) => {
    const nextSessions = sessions.filter((session) => session.key !== sessionId);
    const replacement = nextSessions.length === 0 ? createSession() : null;
    const finalSessions = replacement ? [replacement] : nextSessions;

    setSessions(finalSessions);

    if (activeKey === sessionId) {
      setActiveKey(finalSessions[0].key);
    }

    try {
      await invoke("delete_session", { sessionId });
      if (replacement) {
        await invoke("create_session", { sessionId: replacement.key });
      }
    } catch (error) {
      setBootError(getErrorMessage(error));
    }
  };

  const handlePermissionModeChange = async (mode: PermissionMode) => {
    const sessionId = activeKey;
    setSessions((current) =>
      current.map((session) =>
        session.key === sessionId
          ? {
              ...session,
              permissionMode: mode,
              sessionContext: session.sessionContext
                ? { ...session.sessionContext, mode }
                : session.sessionContext,
            }
          : session,
      ),
    );

    try {
      const sessionContext = await invoke<SessionContext>("set_permission_mode", {
        sessionId,
        mode,
      });
      setSessions((current) =>
        current.map((session) =>
          session.key === sessionId
            ? {
                ...session,
                permissionMode: sessionContext.mode,
                sessionContext,
              }
            : session,
        ),
      );
    } catch (error) {
      setBootError(getErrorMessage(error));
    }
  };

  const sessionMenu = (item: ConversationItemType): MenuProps => ({
    items: [{ key: "delete", label: "删除" }],
    onClick: ({ domEvent }) => {
      domEvent.stopPropagation();
      handleDeleteSession(item.key);
    },
  });

  const handleSubmit = async (message: string) => {
    const question = message.trim();

    if (!question || !ready || loadingSession) {
      return;
    }

    const sessionId = activeKey;
    const userMessage: ChatMessage = {
      id: makeId(),
      role: "user",
      content: question,
    };
    const assistantMessage: ChatMessage = {
      id: makeId(),
      role: "assistant",
      content: "",
      loading: true,
      streaming: true,
    };

    streamTargetRef.current[sessionId] = assistantMessage.id;
    setInput("");
    setLoadingSession(sessionId);
    setSessions((current) =>
      current.map((session) => {
        if (session.key !== sessionId) {
          return session;
        }

        const hasUserMessage = session.messages.some(
          (item) => item.role === "user",
        );

        return {
          ...session,
          label: hasUserMessage ? session.label : question.slice(0, 24),
          messages: [...session.messages, userMessage, assistantMessage],
        };
      }),
    );

    try {
      const reply = await invoke<AgentReply>("call", {
        sessionId,
        question,
      });
      const sessionContext = reply.session_context ?? null;

      updateMessage(sessionId, assistantMessage.id, (message) => ({
        ...message,
        content: message.content || reply.content,
        reasoningContent: message.reasoningContent || reply.reasoning_content || undefined,
        parts: message.parts ?? (reply.content ? appendTextPart(message, reply.content) : undefined),
        loading: false,
        streaming: false,
      }));
      if (sessionContext) {
        setSessions((current) =>
          current.map((session) =>
            session.key === sessionId
              ? {
                  ...session,
                  permissionMode: sessionContext.mode,
                  sessionContext,
                }
              : session,
          ),
        );
      }
    } catch (error) {
      updateMessage(sessionId, assistantMessage.id, (message) => ({
        ...message,
        content: getErrorMessage(error),
        loading: false,
        streaming: false,
        error: true,
      }));
    } finally {
      cleanupPermissionListeners(sessionId);
      delete streamTargetRef.current[sessionId];
      setLoadingSession((current) => (current === sessionId ? null : current));
    }
  };

  return (
    <ConfigProvider
      theme={{
        token: {
          borderRadius: 8,
          colorPrimary: "#1677ff",
        },
      }}
    >
      <XProvider>
        <main className="app-shell">
          <aside className="sidebar">
            <div className="sidebar-header">
              <div className="brand">Simple Agent</div>
              <Button type="primary" onClick={handleCreateSession}>
                新建
              </Button>
              {/*<Button onClick={handleMockMessages}>Mock</Button>*/}
            </div>
            <Conversations
              className="conversation-list"
              activeKey={activeKey}
              items={conversationItems}
              menu={sessionMenu}
              onActiveChange={setActiveKey}
            />
          </aside>

          <section className="chat-panel">
            <header className="chat-header">
              <div className="chat-title">{activeSession?.label}</div>
              {bootError ? <div className="boot-error">{bootError}</div> : null}
            </header>

            <div
              className="message-area"
              ref={containRef}
              onScroll={handleMessageAreaScroll}
            >
              {bubbleItems.length ? (
                <Bubble.List
                  items={bubbleItems}
                  role={{
                    user: {
                      placement: "end",
                      variant: "filled",
                    },
                    assistant: {
                      placement: "start",
                      variant: "borderless",
                      contentRender: (content, info) => {
                        const reasoningContent =
                          typeof info.extraInfo?.reasoningContent === "string"
                            ? info.extraInfo.reasoningContent
                            : "";
                        const toolCalls = Array.isArray(
                          info.extraInfo?.toolCalls,
                        )
                          ? (info.extraInfo.toolCalls as ToolCallMessage[])
                          : [];
                        const parts = Array.isArray(info.extraInfo?.parts)
                          ? (info.extraInfo.parts as AssistantMessagePart[])
                          : [];
                        const streaming = Boolean(info.extraInfo?.streaming);
                        const markdownProps = {
                          streaming: {
                            hasNextChunk: streaming,
                            enableAnimation: true,
                            tail: streaming,
                          },
                        };

                        return (
                          <div className="assistant-message">
                            {reasoningContent ? (
                              <Think title="思考" defaultExpanded={false}>
                                {/*<Streamdown mode="streaming" parseIncompleteMarkdown isAnimating={streaming}>
                                  {reasoningContent}
                                </Streamdown>*/}
                                <XMarkdown content={reasoningContent} {...markdownProps} />
                              </Think>
                            ) : null}
                            {parts.length ? (
                              parts.map((part) =>
                                part.type === "text" ? (
                                  <XMarkdown
                                    key={part.id}
                                    content={part.content}
                                    {...markdownProps}
                                  />
                                ) : (
                                  <div className="tool-call-list" key={part.id}>
                                    {renderToolCall(part.toolCall)}
                                  </div>
                                ),
                              )
                            ) : toolCalls.length ? (
                              <div className="tool-call-list">
                                {toolCalls.map(renderToolCall)}
                              </div>
                            ) : null}
                            {/*<Streamdown mode="streaming" parseIncompleteMarkdown isAnimating={streaming}>
                              {String(content)}
                            </Streamdown>*/}
                            {parts.length ? null : (
                              <XMarkdown content={String(content)} {...markdownProps} />
                            )}
                          </div>
                        );
                      },
                    },
                  }}
                />
              ) : (
                <Empty description="开始对话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
              )}
            </div>

            <div className="sender-wrap">
              <Sender
                value={input}
                loading={loadingSession === activeKey}
                disabled={!ready || Boolean(loadingSession)}
                placeholder={ready ? "输入消息" : "初始化中"}
                submitType="enter"
                onChange={setInput}
                onSubmit={handleSubmit}
                footer={
                  <div className="sender-footer">
                    <Dropdown menu={permissionModeMenu} trigger={["click"]}>
                      <Button
                        className="permission-mode-button"
                        icon={activePermissionModeOption.icon}
                        style={
                          {
                            "--permission-mode-color":
                              activePermissionModeOption.color,
                            "--permission-mode-border":
                              activePermissionModeOption.border,
                            "--permission-mode-background":
                              activePermissionModeOption.background,
                          } as CSSProperties
                        }
                        onClick={(event) => event.preventDefault()}
                      >
                        {getPermissionModeLabel(activePermissionMode)}
                        <DownOutlined />
                      </Button>
                    </Dropdown>
                    {activeSession?.sessionContext ? (
                      <Popover
                        content={
                          <div className="token-popover">
                            <div>
                              已用：
                              {activeSession.sessionContext.token.toLocaleString()}
                            </div>
                            <div>
                              总量：
                              {activeSession.sessionContext.total_token.toLocaleString()}
                            </div>
                            <div>
                              剩余：
                              {Math.max(
                                0,
                                activeSession.sessionContext.total_token -
                                  activeSession.sessionContext.token,
                              ).toLocaleString()}
                            </div>
                          </div>
                        }
                        placement="topLeft"
                        trigger="hover"
                      >
                        <div className="bottom-bar">
                          <div className="token-ring" style={tokenRingStyle}>
                            <span>{tokenPercent}</span>
                          </div>
                        </div>
                      </Popover>
                    ) : null}
                  </div>
                }
              />
            </div>
          </section>
        </main>
        <Modal
          title={permissionRequest?.name ?? "权限申请"}
          open={Boolean(permissionRequest)}
          footer={[
            <Button key="deny" onClick={() => replyPermission("Deny")}>
              拒绝
            </Button>,
            <Button key="allow" onClick={() => replyPermission("Allow")}>
              允许
            </Button>,
            <Button key="always" type="primary" onClick={() => replyPermission("AlwaysAllow")}>
              总是允许
            </Button>,
          ]}
          onCancel={() => replyPermission("Deny")}
        >
          {permissionRequest?.content}
        </Modal>
      </XProvider>
    </ConfigProvider>
  );
}

export default App;
