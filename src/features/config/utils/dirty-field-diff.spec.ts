import { describe, expect, it } from "vitest";
import { collectDirtyFields, DIRTY_DETAIL_MAX_ITEMS, formatDirtyValue, parseDirtySnapshot, shortenDirtyPathForDisplay } from "./dirty-field-diff";

describe("collectDirtyFields 基础比对", () => {
  it("只列出不一致的叶子字段，并给出旧值 → 新值", () => {
    const fields = collectDirtyFields(
      { uiSizeScale: 1, uiFont: "A", uiLanguage: "zh-CN" },
      { uiSizeScale: 1.1, uiFont: "A", uiLanguage: "zh-CN" },
    );

    expect(fields).toEqual([{ path: "uiSizeScale", before: "1", after: "1.1" }]);
  });

  it("嵌套对象展开成点号路径", () => {
    const fields = collectDirtyFields(
      { editor: { tabSize: 2, wrap: false } },
      { editor: { tabSize: 4, wrap: false } },
    );

    expect(fields).toEqual([{ path: "editor.tabSize", before: "2", after: "4" }]);
  });

  it("一侧缺失（新增或删除）时那一侧写「（无）」", () => {
    const added = collectDirtyFields({}, { proxy: "http://127.0.0.1:7890" });
    expect(added).toEqual([{ path: "proxy", before: "（无）", after: "http://127.0.0.1:7890" }]);

    const removed = collectDirtyFields({ proxy: "http://127.0.0.1:7890" }, {});
    expect(removed).toEqual([{ path: "proxy", before: "http://127.0.0.1:7890", after: "（无）" }]);
  });

  it("完全一致时返回空清单", () => {
    expect(collectDirtyFields({ a: 1, b: [1, 2] }, { a: 1, b: [1, 2] })).toEqual([]);
  });

  it("pathPrefix 作为清单最外层名字", () => {
    const fields = collectDirtyFields({ dirty: false }, { dirty: true }, "personas");
    expect(fields[0]?.path).toBe("personas.dirty");
  });
});

describe("collectDirtyFields 数组配对", () => {
  it("带 id 的数组按 id 配对：中间插入一条不会让后面全部误报", () => {
    const saved = [
      { id: "a", baseUrl: "https://a" },
      { id: "b", baseUrl: "https://b" },
    ];
    const current = [
      { id: "a", baseUrl: "https://a" },
      { id: "c", baseUrl: "https://c" },
      { id: "b", baseUrl: "https://b" },
    ];

    const fields = collectDirtyFields(saved, current, "apiProviders");

    // 只报新增的那条，a 与 b 都不该出现
    expect(fields).toHaveLength(1);
    expect(fields[0]?.path).toBe("apiProviders[c]");
    expect(fields[0]?.before).toBe("（无）");
  });

  it("无 id 的数组按位置配对", () => {
    const fields = collectDirtyFields({ list: ["a", "b"] }, { list: ["a", "x"] });
    expect(fields).toEqual([{ path: "list[1]", before: "b", after: "x" }]);
  });

  it("数组长度变化只报多出或少掉的位置", () => {
    const fields = collectDirtyFields({ list: ["a"] }, { list: ["a", "b"] });
    expect(fields).toEqual([{ path: "list[1]", before: "（无）", after: "b" }]);
  });
});

describe("formatDirtyValue 值处理", () => {
  it("密钥形态的字段打码", () => {
    expect(formatDirtyValue("sk-live-abcdef", "apiKeys[0]")).toBe("***");
    expect(formatDirtyValue("sk-live-abcdef", "apiKey")).toBe("***");
    expect(formatDirtyValue("hunter2", "password")).toBe("***");
    expect(formatDirtyValue("abc", "clientSecret")).toBe("***");
    expect(formatDirtyValue("tok", "accessToken")).toBe("***");
    expect(formatDirtyValue("tok", "authToken")).toBe("***");
    expect(formatDirtyValue("tok", "refreshToken")).toBe("***");
    expect(formatDirtyValue("-----BEGIN", "privateKey")).toBe("***");
    // 整组密钥以数组形式出现时也不能把内容打出来（字段名含 apiKey）
    expect(formatDirtyValue(["sk-1", "sk-2"], "apiKeys")).toBe("***");
  });

  it("含 token / key 的计数类字段不打码（否则真正想看的那行变成 *** → ***）", () => {
    expect(formatDirtyValue(128000, "contextWindowTokens")).toBe("128000");
    expect(formatDirtyValue(8192, "maxTokens")).toBe("8192");
    expect(formatDirtyValue(4096, "maxTokenCount")).toBe("4096");
    expect(formatDirtyValue("cl100k_base", "tokenizer")).toBe("cl100k_base");
    // 名字命中密钥形态、但值是数字：数字装不下密钥，不该打码
    expect(formatDirtyValue(2, "apiKeysCount")).toBe("2");
  });

  it("contextWindowTokens 改动时清单里能看到真实旧值与新值", () => {
    const fields = collectDirtyFields(
      { contextWindowTokens: 128000 },
      { contextWindowTokens: 200000 },
      "apiProviders[api-config-1771703416707].models[api-config-1771703416707-model-default]",
    );

    expect(fields).toHaveLength(1);
    expect(fields[0]?.before).toBe("128000");
    expect(fields[0]?.after).toBe("200000");
    expect(fields[0]?.before).not.toBe("***");
  });

  it("敏感字段为空时与普通空值区分", () => {
    expect(formatDirtyValue("", "apiKey")).toBe("（空）");
    expect(formatDirtyValue("", "baseUrl")).toBe("（空）");
  });

  it("长值截到 80 字符并加省略号", () => {
    const long = "x".repeat(120);
    const formatted = formatDirtyValue(long, "baseUrl");
    expect(formatted).toHaveLength(81);
    expect(formatted.endsWith("…")).toBe(true);
  });

  it("非字符串值走 JSON", () => {
    expect(formatDirtyValue({ a: 1 }, "meta")).toBe('{"a":1}');
    expect(formatDirtyValue(true, "enabled")).toBe("true");
  });

  it("undefined 显示（无）", () => {
    expect(formatDirtyValue(undefined, "note")).toBe("（无）");
  });
});

describe("collectDirtyFields 上限与快照解析", () => {
  it("明细超过上限时截断", () => {
    const saved: Record<string, number> = {};
    const current: Record<string, number> = {};
    for (let index = 0; index < DIRTY_DETAIL_MAX_ITEMS + 50; index += 1) {
      saved[`k${index}`] = 0;
      current[`k${index}`] = 1;
    }

    expect(collectDirtyFields(saved, current)).toHaveLength(DIRTY_DETAIL_MAX_ITEMS);
  });

  it("空快照按空对象处理，坏 JSON 抛给调用方", () => {
    expect(parseDirtySnapshot("")).toEqual({});
    expect(parseDirtySnapshot("   ")).toEqual({});
    expect(parseDirtySnapshot('{"a":1}')).toEqual({ a: 1 });
    expect(() => parseDirtySnapshot("{bad")).toThrow();
  });
});

describe("shortenDirtyPathForDisplay 路径显示", () => {
  it("长 id 截到 8 位加省略号，短 id 保持原样", () => {
    expect(shortenDirtyPathForDisplay(
      "apiProviders[api-config-1771703416707].models[api-config-1771703416707-model-default].contextWindowTokens",
    )).toBe("apiProviders[api-conf…].models[api-conf…].contextWindowTokens");

    expect(shortenDirtyPathForDisplay("skills[custom/demo].name")).toBe("skills[custom/demo].name");
    expect(shortenDirtyPathForDisplay("apiProviders[p1].apiKeys[k1].apiKey"))
      .toBe("apiProviders[p1].apiKeys[k1].apiKey");
  });

  it("只影响显示，明细里的 path 仍是完整 id", () => {
    const fullId = "api-config-1771703416707";
    const fields = collectDirtyFields(
      { models: [{ id: fullId, maxTokens: 8192 }] },
      { models: [{ id: fullId, maxTokens: 16384 }] },
      "apiProviders[p]",
    );

    expect(fields[0]?.path).toBe(`apiProviders[p].models[${fullId}].maxTokens`);
    expect(shortenDirtyPathForDisplay(fields[0]?.path ?? "")).toBe("apiProviders[p].models[api-conf…].maxTokens");
  });
});

// 以下三组按三个 checker 的真实数据形状构造，验证「传给弹窗的那份清单」方向与口径正确。
describe("api-provider：供应商详情草稿", () => {
  it("改地址与密钥时给出字段路径，密钥脱敏", () => {
    const saved = {
      id: "p1",
      name: "OpenAI",
      baseUrl: "https://api.example.com/v1",
      apiKeys: [{ id: "k1", apiKey: "sk-old-value" }],
    };
    const current = {
      id: "p1",
      name: "OpenAI",
      baseUrl: "https://api2.example.com/v1",
      apiKeys: [{ id: "k1", apiKey: "sk-new-value" }],
    };

    const fields = collectDirtyFields(saved, current, "apiProviders[p1]");

    expect(fields.map((field) => field.path)).toEqual([
      "apiProviders[p1].apiKeys[k1].apiKey",
      "apiProviders[p1].baseUrl",
    ]);
    expect(fields.find((field) => field.path.endsWith("apiKey"))?.after).toBe("***");
    expect(fields.find((field) => field.path.endsWith("baseUrl"))?.before).toBe("https://api.example.com/v1");
  });
});

describe("image-provider：图片生成配置快照", () => {
  it("模型 id 与供应商字段各自独立成条", () => {
    const saved = {
      imageGenerationModelId: "m1",
      imageProviders: [{ id: "ip1", name: "Comfy", providerType: "comfyui" }],
    };
    const current = {
      imageGenerationModelId: "m2",
      imageProviders: [{ id: "ip1", name: "Comfy 本地", providerType: "comfyui" }],
    };

    const fields = collectDirtyFields(saved, current, "imageProviders");

    expect(fields).toEqual([
      { path: "imageProviders.imageGenerationModelId", before: "m1", after: "m2" },
      { path: "imageProviders.imageProviders[ip1].name", before: "Comfy", after: "Comfy 本地" },
    ]);
  });
});

describe("custom-skill：技能草稿", () => {
  it("名称、描述、正文各成一条，正文长值被截断", () => {
    const skill = { path: "custom/demo", name: "旧名", description: "旧描述", content: "a".repeat(200) };
    const draft = { path: skill.path, name: "新名", description: "旧描述", content: `${"a".repeat(200)}b` };

    const fields = collectDirtyFields(skill, draft, `skills[${skill.path}]`);

    expect(fields).toHaveLength(2);
    const nameField = fields.find((field) => field.path === "skills[custom/demo].name");
    expect(nameField).toEqual({
      path: "skills[custom/demo].name",
      before: "旧名",
      after: "新名",
    });
    const contentField = fields.find((field) => field.path === "skills[custom/demo].content");
    expect(contentField?.after?.endsWith("…")).toBe(true);
    expect(contentField?.after?.length).toBe(81);
  });
});
