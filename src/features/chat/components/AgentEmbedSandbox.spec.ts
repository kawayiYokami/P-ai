import { describe, expect, it } from "vitest";
import AGENT_EMBED_SOURCE from "./AgentEmbedSandbox.vue?raw";

/**
 * 嵌入本地 html 卡片（`<agent-embed src="E:/.../x.html">`）的「在右栏打开」入口。
 *
 * 这里只做源码级钉住：组件要先异步读文件才有内容可渲染，而仓库没有 jsdom / test-utils，
 * 只能 SSR 一帧，拿不到读完之后的状态。按钮真正长什么样、点了右栏开不开，在 demo 与真机里看。
 */
describe("嵌入本地 html 的「在右栏打开」", () => {
  it("把能力透传给沙箱：标题取顶层 label，点击走 openFile", () => {
    const source = AGENT_EMBED_SOURCE;
    expect(source).toContain("inject(CHAT_HTML_READER_KEY, null)");
    expect(source).toContain(":open-in-reader-title=\"openInReaderTitle\"");
    expect(source).toContain("@open-in-reader=\"handleOpenInReader\"");
    expect(source).toContain("chatHtmlReader?.openFile(path)");
  });

  it("右栏自己渲染时 inject 到 null，标题为空、按钮不出现", () => {
    const source = AGENT_EMBED_SOURCE;
    // label 为空 → InteractiveHtmlSandbox 里 `v-if="openInReaderTitle"` 不成立
    expect(source).toContain("chatHtmlReader?.label || \"\"");
  });

  it("打开用的是校验过的绝对路径，不从原始 src 重新拼", () => {
    const source = AGENT_EMBED_SOURCE;
    // 载入成功那一步把校验过的 path 记下来，handler 只读这个 ref
    expect(source).toContain("loadedPath.value = path;");
    expect(source).toContain("const path = loadedPath.value;");
    expect(source).not.toContain("openFile(rawSrc.value)");
    expect(source).not.toContain("openFile(props.src)");
  });
});
