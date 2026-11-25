import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import CronPicker from "./CronPicker.vue";

async function render(modelValue: string, disabled = false): Promise<string> {
  const app = createSSRApp({
    render: () => h(CronPicker, { modelValue, disabled }),
  });
  return renderToString(app);
}

describe("CronPicker 外部传值回显", () => {
  it("外部表达式会同步成可读描述，且对应字段被选中", async () => {
    const html = await render("30 8 * * 1");
    expect(html).toContain("每周一 08:30");
    // 周一落在周字段里；周字段未展开，靠标签上的圆点提示已配置
    expect(html).not.toContain("30 8 * * 1");
  });

  it("外部传范围写法会被展开成列表", async () => {
    const html = await render("0 9 * * 1-5");
    expect(html).toContain("每周一、二、三、四、五 09:00");
  });

  it("外部传无法识别的值不覆盖界面，只给提示", async () => {
    const html = await render("这不是cron");
    expect(html).toContain("无法识别");
    expect(html).toContain("每天 09:00");
  });

  it("外部清空时保留当前点选状态", async () => {
    const html = await render("");
    expect(html).toContain("每天 09:00");
  });

  it("不把 cron 语法露给用户", async () => {
    const html = await render("*/5 * * * *");
    expect(html).toContain("每 5 分钟");
    expect(html).not.toContain("*/5 * * * *");
  });
});
