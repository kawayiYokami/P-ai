import { describe, expect, it } from "vitest";
import {
  CONVERSATION_PROGRESS_DOT_CLASS,
  CONVERSATION_PROGRESS_STATE_KEYS,
  conversationProgressDotClass,
  conversationProgressVisual,
  conversationShowsProgressState,
  isKnownProgressState,
} from "./conversation-item-display";

// 七档互不撞色的排布：灰浅 / 灰深 / 紫 / 蓝 / 橙 / 红 / 绿
describe("conversationProgressVisual 颜色映射", () => {
  it("六态各自映射到预期 token", () => {
    expect(conversationProgressVisual("planning").dotClass).toBe("bg-accent");
    expect(conversationProgressVisual("todo").dotClass).toBe("bg-base-content/45");
    expect(conversationProgressVisual("in_progress").dotClass).toBe("bg-primary");
    expect(conversationProgressVisual("in_review").dotClass).toBe("bg-warning");
    expect(conversationProgressVisual("blocked").dotClass).toBe("bg-error");
    expect(conversationProgressVisual("done").dotClass).toBe("bg-success");
  });

  it("未设置是最弱一档灰", () => {
    const unset = conversationProgressVisual("");
    expect(unset.key).toBe("");
    expect(unset.dotClass).toBe("bg-base-content/25");
    expect(unset.labelKey).toBe("chat.conversationStatus.unset");
  });

  it("缺字段与空串一样落到未设置，不会变成任何六态", () => {
    for (const empty of [undefined, null, "", "   "]) {
      expect(conversationProgressVisual(empty).key).toBe("");
    }
  });

  it("未设置与待办是两档不同的灰，不可合并", () => {
    expect(conversationProgressVisual("").dotClass).not.toBe(conversationProgressVisual("todo").dotClass);
  });

  it("未知值退回未设置，不复用任一已知状态的配色", () => {
    const known = CONVERSATION_PROGRESS_STATE_KEYS.map((key) => conversationProgressVisual(key).dotClass);
    const unknown = conversationProgressVisual("archive_pending");
    expect(unknown.key).toBe("");
    expect(known).not.toContain(unknown.dotClass);
    expect(isKnownProgressState("archive_pending")).toBe(false);
  });

  it("七个 key 颜色互不相同", () => {
    const dotClasses = [
      conversationProgressVisual("").dotClass,
      ...CONVERSATION_PROGRESS_STATE_KEYS.map((key) => conversationProgressVisual(key).dotClass),
    ];
    expect(new Set(dotClasses).size).toBe(dotClasses.length);
  });

  // 这两条是计划里写死的禁令，防止后人「优化」回去
  it("不使用 info 与 secondary（分别撞 primary 蓝、success 绿）", () => {
    for (const state of ["", ...CONVERSATION_PROGRESS_STATE_KEYS]) {
      const visual = conversationProgressVisual(state);
      expect(visual.dotClass).not.toContain("info");
      expect(visual.dotClass).not.toContain("secondary");
      expect(visual.chipClass).not.toContain("info");
      expect(visual.chipClass).not.toContain("secondary");
    }
  });

  it("颜色段不含基础 badge 类", () => {
    for (const state of ["", ...CONVERSATION_PROGRESS_STATE_KEYS]) {
      const chipClass = conversationProgressVisual(state).chipClass;
      expect(chipClass.length).toBeGreaterThan(0);
      expect(chipClass).not.toMatch(/(^|\s)badge(\s|$)/);
    }
  });

  // 形态统一：六态 + 未设置都走实心，只有色差，没有第二套形态
  it("所有状态同一形态，不出现描边或透明底", () => {
    for (const state of ["", ...CONVERSATION_PROGRESS_STATE_KEYS]) {
      const chipClass = conversationProgressVisual(state).chipClass;
      expect(chipClass).not.toContain("bg-transparent");
      expect(chipClass).not.toContain("border-");
      expect(chipClass).not.toContain("color-mix");
    }
  });
});

// 圆点只靠颜色不能自解释，所以六态各带一个 2~3 字短标签；未设置不带标签（空状态不占文字位置）
describe("会话状态短标签", () => {
  it("六态各有短标签，未设置为空", () => {
    expect(conversationProgressVisual("").shortLabelKey).toBe("");
    for (const state of CONVERSATION_PROGRESS_STATE_KEYS) {
      expect(conversationProgressVisual(state).shortLabelKey).toMatch(/^chat\.conversationStatus\.short\./);
    }
  });

  it("未知值退回未设置后同样不带标签，不会冒出某个已知状态的文字", () => {
    expect(conversationProgressVisual("archive_pending").shortLabelKey).toBe("");
  });
});

// 白名单：只有追问会话有状态。主会话管议题流转用看板，不挂进度工具
describe("conversationShowsProgressState 白名单", () => {
  it("只有追问会话（含队员会话）有状态", () => {
    expect(conversationShowsProgressState("side_chat")).toBe(true);
  });

  it("主会话改用议题看板，不再有会话状态", () => {
    expect(conversationShowsProgressState("chat")).toBe(false);
  });

  it("系统通知、委托、远程联系人、空值都没有状态", () => {
    for (const kind of ["system_notification", "delegate", "remote_im_contact", "", "   ", undefined, null, "future_kind"]) {
      expect(conversationShowsProgressState(kind)).toBe(false);
    }
  });
});

describe("圆点类名", () => {
  it("描边独立于填充色，负责存在性", () => {
    expect(CONVERSATION_PROGRESS_DOT_CLASS).toContain("ring-1");
    expect(CONVERSATION_PROGRESS_DOT_CLASS).toContain("ring-base-content/40");
    expect(CONVERSATION_PROGRESS_DOT_CLASS).toContain("rounded-full");
  });

  it("整套类名同时含几何、描边与当前状态的填充色", () => {
    const classes = conversationProgressDotClass(conversationProgressVisual("blocked"));
    expect(classes).toContain(CONVERSATION_PROGRESS_DOT_CLASS);
    expect(classes).toContain("bg-error");
  });
});
