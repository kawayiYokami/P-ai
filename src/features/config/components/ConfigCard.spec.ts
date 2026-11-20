// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import ConfigCard from "./ConfigCard.vue";
import {
  CONTACT_GROUP_KEY_ATTR,
  contactGroupKeyFromEvent,
  shouldOpenContactGroupMenu,
} from "../views/config-tabs/remote-im/helpers";
// 模板接线守卫：读源码文本，漏挂 data 键/具名 handler 时当场红
import remoteImTabSource from "../views/config-tabs/RemoteImTab.vue?raw";

afterEach(() => {
  // 任一断言抛错也不要把 spy 泄漏给后面的用例
  vi.restoreAllMocks();
});

const text = (value: string) => [h("span", value)];

async function renderStatic(
  component: typeof ConfigCard,
  props: Record<string, unknown>,
  slots: Record<string, () => unknown[]> = {},
): Promise<string> {
  const app = createApp({ render: () => h(component, props, slots) });
  return renderToString(app);
}

describe("ConfigCard 的容器与扩展", () => {
  it("默认形态：标题外置 + 卡片边框容器，带内容内边距", async () => {
    const html = await renderStatic(ConfigCard, { title: "渠道配置" }, { default: () => text("内容") });
    expect(html).toContain("rounded-box border border-base-300 bg-base-100");
    expect(html).toContain("渠道配置");
    expect(html).toContain("内容");
    // 标题在卡片外部 header 中渲染
    expect(html).toMatch(/<header[^>]*>[\s\S]*渠道配置[\s\S]*<\/header>[\s\S]*rounded-box/);
    // 内容区默认带内边距
    expect(html).toContain("px-3");
  });

  it("interactive 时整卡可点，过渡覆盖 transform（按下不瞬跳）", async () => {
    const html = await renderStatic(ConfigCard, { interactive: true }, { default: () => text("卡片内容") });
    expect(html).toContain("hover:border-primary/50");
    expect(html).toContain("active:scale-[0.99]");
    expect(html).toContain("transition-all duration-150");
  });

  it("默认不带整卡可点态，既有使用方不受影响", async () => {
    const html = await renderStatic(ConfigCard, {}, { default: () => text("卡片内容") });
    expect(html).not.toContain("active:scale-[0.99]");
  });

  it("flush 时内容区不加内边距，且保留内容盒（不改成 contents）", async () => {
    const html = await renderStatic(ConfigCard, { flush: true }, { default: () => text("内容") });
    expect(html).not.toContain('class="px-3"');
    // 有内容时就该直接贴着一个无内边距的容器
    expect(html).toContain('<div class="">');
  });

  it("overflowVisible 控制是否裁边：默认裁，卡内有内联下拉时打开它", async () => {
    const clipped = await renderStatic(ConfigCard, {}, { default: () => text("内容") });
    expect(clipped).toContain("overflow-hidden");

    const visible = await renderStatic(ConfigCard, { overflowVisible: true }, { default: () => text("内容") });
    expect(visible).toContain("overflow-visible");
    expect(visible).not.toContain("overflow-hidden");
  });
});

describe("分组右键链路（data 键 → 分组）", () => {
  function mountHeaderWithKey(key?: string) {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const resolved: string[] = [];
    const attrs = key === undefined ? {} : { [CONTACT_GROUP_KEY_ATTR]: key };
    const app = createApp({
      render: () =>
        h(
          ConfigCard,
          attrs,
          {
            title: () => [
              h(
                "div",
                {
                  class: "group-header",
                  onContextmenu: (event: MouseEvent) => resolved.push(contactGroupKeyFromEvent(event)),
                },
                "核心群",
              ),
            ],
            default: () => [h("div", { class: "contact-row" }, "联系人行")],
          },
        ),
    });
    app.mount(host);
    return {
      host,
      resolved,
      fire: (selector: string) => {
        const el = host.querySelector(selector) as HTMLElement;
        el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
      },
      unmount: () => {
        app.unmount();
        host.remove();
      },
    };
  }

  it("卡片根上的 data 键能被分组头取回（透传没断、closest 链没断）", () => {
    const m = mountHeaderWithKey("g1");
    m.fire(".group-header");
    expect(m.resolved).toEqual(["g1"]);
    m.unmount();
  });

  it("没有该属性时取回空串，不会悄悄拿到别的分组", () => {
    const m = mountHeaderWithKey();
    m.fire(".group-header");
    expect(m.resolved).toEqual([""]);
    m.unmount();
  });

  it("组内联系人行右键不冒到分组头（菜单只挂分组头）", () => {
    const m = mountHeaderWithKey("g1");
    m.fire(".contact-row");
    expect(m.resolved).toEqual([]);
    m.unmount();
  });

  it("「未分组」段不弹菜单，只有真分组才弹", () => {
    expect(shouldOpenContactGroupMenu(null)).toBe(false);
    expect(shouldOpenContactGroupMenu({ id: "" })).toBe(false);
    expect(shouldOpenContactGroupMenu({ id: "g1" })).toBe(true);
  });

  it("RemoteImTab 的分组卡同时挂 data 键与具名右键 handler（模板接线别断）", () => {
    expect(remoteImTabSource).toContain(':data-contact-group-key="section.key"');
    expect(remoteImTabSource).toContain('@contextmenu="handleContactGroupHeaderContextmenu"');
    // 换渠道与返回列表都要把 tab 复位，接线断了会停在上一次的面上
    expect(remoteImTabSource).toContain("watch(selectedChannelId, () => {");
    expect(remoteImTabSource).toContain("resetDetailTab();");
    // 分组卡里有未 teleport 的内联下拉，卡片不能裁边
    expect(remoteImTabSource).toContain("overflow-visible");
  });
});
