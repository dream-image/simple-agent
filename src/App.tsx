import {useEffect, useLayoutEffect, useMemo, useRef, useState} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button, ConfigProvider, Empty } from "antd";
import type { MenuProps } from "antd";
import { Bubble, Conversations, Sender, Think, XProvider } from "@ant-design/x";
import type { BubbleItemType, ConversationItemType } from "@ant-design/x";
import { Streamdown } from "streamdown";
import "./App.css";

type AgentReply = {
  content: string;
  reasoning_content?: string | null;
};

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

type ToolCallMessage = {
  id: string;
  name: string;
  arguments: string;
  content: string;
  success: boolean;
};

type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoningContent?: string;
  toolCalls?: ToolCallMessage[];
  loading?: boolean;
  error?: boolean;
};

type ChatSession = {
  key: string;
  label: string;
  messages: ChatMessage[];
};

const makeId = () => `${Date.now()}-${Math.random().toString(36).slice(2)}`;

const createSession = (): ChatSession => ({
  key: makeId(),
  label: "新会话",
  messages: [],
});

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const getToolSummary = (content: string) => {
  const text = content.replace(/\s+/g, " ").trim();
  if (!text) {
    return "无输出";
  }
  return text.length > 140 ? `${text.slice(0, 140)}...` : text;
};

const isScrollAtBottom = (element: HTMLDivElement) =>
  element.scrollHeight - element.scrollTop - element.clientHeight < 8;

function App() {
  const [firstSession] = useState(createSession);
  const [sessions, setSessions] = useState<ChatSession[]>([firstSession]);
  const [activeKey, setActiveKey] = useState(firstSession.key);
  const [input, setInput] = useState("");
  const [loadingSession, setLoadingSession] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [bootError, setBootError] = useState("");
  const containRef = useRef<HTMLDivElement | null>(null);
  const streamTargetRef = useRef<Record<string, string>>({});
  const shouldAutoScrollRef = useRef(true);

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
          message.content || message.reasoningContent || message.toolCalls?.length,
        );
        const loading = Boolean(message.loading && !hasVisibleContent);

        return {
          key: message.id,
          role: message.role,
          content: message.content,
          loading,
          status: message.error ? "error" : loading ? "loading" : "success",
          extraInfo: {
            reasoningContent: message.reasoningContent,
            toolCalls: message.toolCalls,
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

  useEffect(() => {
    let unlistenStream: (() => void) | undefined;
    let unlistenToolCall: (() => void) | undefined;

    async function bindEvents() {
      unlistenStream = await listen<AgentStreamEvent>("agent_stream", (event) => {
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

          if (!content && !reasoningContent) {
            return;
          }

          updateMessage(sessionId, messageId, (message) => ({
            ...message,
            content: message.content + content,
            reasoningContent: `${message.reasoningContent ?? ""}${reasoningContent}`,
            loading: false,
          }));
        } catch {
          return;
        }
      });

      unlistenToolCall = await listen<AgentToolCallEvent>(
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
              loading: false,
            };
          });
        },
      );
    }

    bindEvents();

    return () => {
      unlistenStream?.();
      unlistenToolCall?.();
    };
  }, []);

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

      updateMessage(sessionId, assistantMessage.id, (message) => ({
        ...message,
        content: reply.content || message.content,
        reasoningContent: reply.reasoning_content || message.reasoningContent,
        loading: false,
      }));
    } catch (error) {
      updateMessage(sessionId, assistantMessage.id, (message) => ({
        ...message,
        content: getErrorMessage(error),
        loading: false,
        error: true,
      }));
    } finally {
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

                        return (
                          <div className="assistant-message">
                            {reasoningContent ? (
                              <Think title="思考" defaultExpanded={false}>
                                <Streamdown>{reasoningContent}</Streamdown>
                              </Think>
                            ) : null}
                            {toolCalls.length ? (
                              <div className="tool-call-list">
                                {toolCalls.map((toolCall) => (
                                  <details className="tool-call" key={toolCall.id}>
                                    <summary>
                                      <span className="tool-call-name">
                                        {toolCall.name}
                                      </span>
                                      <span
                                        className={
                                          toolCall.success
                                            ? "tool-call-status success"
                                            : "tool-call-status error"
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
                                ))}
                              </div>
                            ) : null}
                            <Streamdown>{String(content)}</Streamdown>
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
              />
            </div>
          </section>
        </main>
      </XProvider>
    </ConfigProvider>
  );
}

export default App;
