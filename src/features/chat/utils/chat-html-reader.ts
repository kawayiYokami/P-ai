import type { InjectionKey } from "vue";

/**
 * 聊天里 html 卡片的「在右栏打开」能力。
 *
 * 这条链路要穿过 ChatView → ChatMessageItem → AppMarkdownRenderer → CodeBlock 四层，
 * 逐层传 props 会让后续每加一个入口都要再穿一遍，所以只在 ChatView provide、在 CodeBlock inject。
 * 右栏自己渲染 html 时不提供该能力（FileReaderPanel 里 provide null 把它挡在面板外），
 * 卡片上也就不会出现「在右栏打开」这个点自己的按钮。
 */

export type ChatHtmlReaderRequest = {
  /** 与聊天里那份完全一致的 html 源码，不落盘、只进内存标签 */
  code: string;
  /** 面板标签标题；缺省由右栏兜底 */
  title?: string;
};

export type ChatHtmlReader = {
  /** 按钮文案，由顶层统一取一次，避免四层各自取 i18n */
  label: string;
  open: (request: ChatHtmlReaderRequest) => void;
};

export const CHAT_HTML_READER_KEY: InjectionKey<ChatHtmlReader | null> = Symbol("pai:chat-html-reader");
