<template>
  <div class="space-y-4">
    <template v-if="mode === 'form'">
      <div class="space-y-2">
        <div class="grid grid-cols-10 items-center gap-3">
          <span class="col-span-3 text-sm text-base-content/70">传输方式</span>
          <select
            :value="transport"
            class="select select-sm select-bordered col-span-7 w-full"
            :disabled="disabled"
            @change="onTransportSelect"
          >
            <option value="stdio">stdio · 本地进程</option>
            <option value="http">http · 远程服务</option>
          </select>
        </div>

        <template v-if="transport === 'stdio'">
          <div class="grid grid-cols-10 items-center gap-3">
            <span class="col-span-3 text-sm text-base-content/70">命令<span class="text-error">*</span></span>
            <input
              v-model="form.command"
              type="text"
              class="input input-sm input-bordered col-span-7 w-full font-mono"
              placeholder="npx"
              :disabled="disabled"
              @input="flush"
            />
          </div>

          <div class="grid grid-cols-10 items-center gap-3">
            <span class="col-span-3 text-sm text-base-content/70">工作目录</span>
            <input
              v-model="form.cwd"
              type="text"
              class="input input-sm input-bordered col-span-7 w-full font-mono"
              :disabled="disabled"
              @input="flush"
            />
          </div>

          <div class="grid grid-cols-10 items-start gap-3">
            <span class="col-span-3 pt-2 text-sm text-base-content/70">参数</span>
            <div class="col-span-7 space-y-2">
              <div v-if="form.args.length" class="flex flex-wrap gap-1">
                <span
                  v-for="(arg, index) in form.args"
                  :key="`arg-${index}`"
                  class="badge badge-ghost max-w-full gap-1 font-mono"
                >
                  <span class="truncate" :title="arg">{{ arg }}</span>
                  <button
                    type="button"
                    class="text-base-content/40 hover:text-error disabled:opacity-40"
                    :disabled="disabled"
                    :title="t('common.delete')"
                    @click="removeArg(index)"
                  >
                    ✕
                  </button>
                </span>
              </div>
              <input
                v-model="argDraft"
                type="text"
                class="input input-sm input-bordered w-full font-mono"
                :disabled="disabled"
                @keydown.enter.prevent="addArg"
              />
            </div>
          </div>
        </template>

        <template v-else>
          <div class="grid grid-cols-10 items-center gap-3">
            <span class="col-span-3 text-sm text-base-content/70">地址<span class="text-error">*</span></span>
            <input
              v-model="form.url"
              type="text"
              class="input input-sm input-bordered col-span-7 w-full font-mono"
              placeholder="https://example.com/mcp"
              :disabled="disabled"
              @input="flush"
            />
          </div>

          <div class="grid grid-cols-10 items-center gap-3">
            <span class="col-span-3 text-sm text-base-content/70">密钥</span>
            <input
              v-model="form.bearerTokenEnvVar"
              type="text"
              class="input input-sm input-bordered col-span-7 w-full font-mono"
              placeholder="MCP_TOKEN"
              :disabled="disabled"
              @input="flush"
            />
          </div>
        </template>
      </div>

      <div class="space-y-2">
        <McpKeyValueRows
          :rows="transport === 'stdio' ? form.env : form.headers"
          key-placeholder="KEY"
          value-placeholder="VALUE"
          :disabled="disabled"
          @update="onRowsChanged(transport === 'stdio' ? 'env' : 'headers', $event)"
        />
        <div class="flex justify-end">
          <button
            type="button"
            class="btn btn-ghost btn-xs gap-1"
            :disabled="disabled"
            @click="addRow(transport === 'stdio' ? 'env' : 'headers')"
          >
            <AppIcon name="plus" class="size-3.5" />
            {{ transport === "stdio" ? "添加环境变量" : "添加请求头" }}
          </button>
        </div>
      </div>
    </template>

    <div v-else class="space-y-2">
      <textarea
        v-model="jsonDraft"
        class="textarea textarea-bordered min-h-56 w-full bg-base-200/30 font-mono text-xs leading-relaxed"
        spellcheck="false"
        :disabled="disabled"
        @blur="applyJsonDraft"
        @input="onJsonDraftInput"
      ></textarea>
      <p v-if="jsonError" class="text-xs text-error">{{ jsonError }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../../shared/components/AppIcon.vue";
import McpKeyValueRows, { type McpKeyValueRow } from "./McpKeyValueRows.vue";

/**
 * MCP 定义编辑器：表单语义化编辑 + JSON 直改双模式。
 *
 * `definitionJson` 始终是唯一存储格式——表单只是它的一个视图，任何改动都序列化回字符串发出去，
 * 不引入第二份持久化结构。定义可以是「mcpServers 一个成员」或「单 server 直接字段」等多种形态，
 * 写回时只替换当前那条 server 的字段，容器与其它成员原样保留。
 *
 * 模式切换按钮由宿主放在卡片标题行（`ConfigCard` 的 `#actions`），这里只接收 `mode`。
 */

const props = withDefaults(defineProps<{
  definitionJson: string;
  disabled?: boolean;
  /** 视图模式由宿主控制：按钮在卡片标题行，不在编辑器内部占一行 */
  mode?: "form" | "json";
}>(), {
  disabled: false,
  mode: "form",
});

const emit = defineEmits<{
  (e: "update:definitionJson", value: string): void;
  (e: "update:mode", value: "form" | "json"): void;
}>();

const { t } = useI18n();

/** 与卡片解析保持一致的「单 server 直接字段」签名 */
const DIRECT_FIELD_KEYS = ["command", "args", "url", "transport", "type", "env", "cwd"];

type Transport = "stdio" | "http";

const argDraft = ref("");
const jsonDraft = ref("");
const jsonError = ref("");

const form = reactive({
  transport: "stdio" as Transport,
  command: "",
  args: [] as string[],
  cwd: "",
  env: [] as McpKeyValueRow[],
  url: "",
  bearerTokenEnvVar: "",
  headers: [] as McpKeyValueRow[],
});

/** 当前定义在整棵树里的位置：改动只落到这个对象上，容器与其它成员不动。 */
let definitionTree: unknown = null;
let serverNode: Record<string, unknown> = {};

const transport = computed(() => form.transport);

// ==================== 解析：JSON → 表单模型 ====================

function asString(value: unknown): string {
  return typeof value === "string" ? value : value === undefined || value === null ? "" : String(value);
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.map((item) => asString(item)) : [];
}

function asRows(value: unknown): McpKeyValueRow[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.entries(value as Record<string, unknown>).map(([key, raw]) => ({
    key,
    value: asString(raw),
  }));
}

/**
 * 定位定义里的「当前 server」节点，保留整棵树用于回写。
 * 兼容五种形态：mcpServers 对象 / mcpServers 数组 / 根级数组 / 根级直接字段 / 平铺命名集合。
 */
function locateServer(tree: unknown): Record<string, unknown> {
  if (!tree || typeof tree !== "object") return {};
  if (Array.isArray(tree)) {
    const first = tree.find((item) => item && typeof item === "object" && !Array.isArray(item));
    return (first as Record<string, unknown>) || {};
  }
  const root = tree as Record<string, unknown>;
  const servers = root.mcpServers;
  if (servers && typeof servers === "object") {
    if (Array.isArray(servers)) {
      const first = servers.find((item) => item && typeof item === "object" && !Array.isArray(item));
      return (first as Record<string, unknown>) || {};
    }
    const first = Object.values(servers as Record<string, unknown>).find(
      (item) => item && typeof item === "object" && !Array.isArray(item),
    );
    return (first as Record<string, unknown>) || {};
  }
  if (DIRECT_FIELD_KEYS.some((key) => key in root)) return root;
  const firstNamed = Object.values(root).find(
    (item) => item && typeof item === "object" && !Array.isArray(item),
  );
  return (firstNamed as Record<string, unknown>) || root;
}

/**
 * transport 以后端口径判定为准（parser.rs）：显式 transport/type 优先，其次看 command，再看 url。
 * 表单里只呈现两档（stdio / http），sse 归到 http 一侧。
 */
function readTransport(node: Record<string, unknown>): Transport {
  const explicit = asString(node.transport || node.type).trim().toLowerCase();
  if (explicit === "stdio" || explicit === "local") return "stdio";
  if (explicit) return "http";
  if (asString(node.command).trim()) return "stdio";
  if (asString(node.url).trim()) return "http";
  return "stdio";
}

function parseIntoForm(json: string) {
  let tree: unknown = {};
  try {
    tree = json.trim() ? JSON.parse(json) : {};
  } catch {
    // JSON 坏了不打断表单：按空定义继续，用户在 JSON 模式里能看到具体错误
    tree = {};
  }
  definitionTree = tree;
  serverNode = locateServer(tree);

  form.transport = readTransport(serverNode);
  form.command = asString(serverNode.command);
  form.args = asStringArray(serverNode.args);
  form.cwd = asString(serverNode.cwd);
  form.env = asRows(serverNode.env);
  form.url = asString(serverNode.url);
  form.bearerTokenEnvVar = asString(serverNode.bearerTokenEnvVar);
  form.headers = asRows(serverNode.headers);
  // 定义被外部换掉了，上一份定义的暂存值不再适用
  stdioStash = null;
  httpStash = null;
}

// ==================== 序列化：表单模型 → JSON ====================

/** 空键的行不写进定义，否则会写出 {"": "x"} 这种无意义条目。 */
function rowsToRecord(rows: McpKeyValueRow[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key) continue;
    out[key] = row.value;
  }
  return out;
}

/**
 * 只写当前传输方式那一组字段，另一组从这里删掉——后端按 transport/command/url 判类型，
 * 留着对侧字段会让判定产生歧义（例如 http 定义里残留 command 会被判成 stdio）。
 * 显式 transport/type 一并清掉，让「有没有 command / url」自己说话。
 */
function buildServerPatch(): Record<string, unknown> {
  const patch: Record<string, unknown> = {};
  if (form.transport === "stdio") {
    const env = rowsToRecord(form.env);
    if (form.command.trim()) patch.command = form.command.trim();
    if (form.args.length) patch.args = [...form.args];
    if (form.cwd.trim()) patch.cwd = form.cwd.trim();
    if (Object.keys(env).length) patch.env = env;
  } else {
    const headers = rowsToRecord(form.headers);
    if (form.url.trim()) patch.url = form.url.trim();
    if (form.bearerTokenEnvVar.trim()) patch.bearerTokenEnvVar = form.bearerTokenEnvVar.trim();
    if (Object.keys(headers).length) patch.headers = headers;
  }
  return patch;
}

const STDIO_ONLY_KEYS = ["command", "args", "cwd", "env"];
const HTTP_ONLY_KEYS = ["url", "bearerTokenEnvVar", "headers", "httpHeaders", "envHttpHeaders"];

/** 记住自己刚发出去的那一份，父组件回传时不再解析一遍，免得重置输入态 */
let pendingJson = "";

function emitJson(value: string) {
  pendingJson = value;
  emit("update:definitionJson", value);
}

function flush() {
  if (!serverNode || typeof serverNode !== "object") return;
  const patch = buildServerPatch();
  const dropKeys = form.transport === "stdio" ? HTTP_ONLY_KEYS : STDIO_ONLY_KEYS;
  for (const key of [...dropKeys, "transport", "type"]) {
    delete serverNode[key];
  }
  for (const [key, value] of Object.entries(patch)) {
    serverNode[key] = value;
  }
  const next = JSON.stringify(definitionTree ?? {}, null, 2);
  emitJson(next);
  jsonDraft.value = next;
  jsonError.value = "";
}

// ==================== 表单交互 ====================

function addArg() {
  const value = argDraft.value.trim();
  if (!value) return;
  form.args = [...form.args, value];
  argDraft.value = "";
  flush();
}

function removeArg(index: number) {
  form.args = form.args.filter((_, i) => i !== index);
  flush();
}

function addRow(target: "env" | "headers") {
  form[target] = [...form[target], { key: "", value: "" }];
}

function onRowsChanged(target: "env" | "headers", rows: McpKeyValueRow[]) {
  form[target] = rows;
  flush();
}

/** 切换传输方式时暂存另一组字段，切回来还能接着填；JSON 里始终只存当前那一组。 */
type StdioStash = { command: string; args: string[]; cwd: string; env: McpKeyValueRow[] };
type HttpStash = { url: string; bearerTokenEnvVar: string; headers: McpKeyValueRow[] };

let stdioStash: StdioStash | null = null;
let httpStash: HttpStash | null = null;

function snapshotCurrentGroup(): void {
  if (form.transport === "stdio") {
    stdioStash = {
      command: form.command,
      args: [...form.args],
      cwd: form.cwd,
      env: form.env.map((row) => ({ ...row })),
    };
  } else {
    httpStash = {
      url: form.url,
      bearerTokenEnvVar: form.bearerTokenEnvVar,
      headers: form.headers.map((row) => ({ ...row })),
    };
  }
}

/** 先把目标那一组清空，避免切换后还留着上一条定义的值；有暂存再覆盖回去。 */
function resetFormGroup(target: Transport): void {
  if (target === "stdio") {
    const stash = stdioStash;
    form.command = stash?.command ?? "";
    form.args = stash ? [...stash.args] : [];
    form.cwd = stash?.cwd ?? "";
    form.env = stash ? stash.env.map((row) => ({ ...row })) : [];
  } else {
    const stash = httpStash;
    form.url = stash?.url ?? "";
    form.bearerTokenEnvVar = stash?.bearerTokenEnvVar ?? "";
    form.headers = stash ? stash.headers.map((row) => ({ ...row })) : [];
  }
}

function switchTransport(next: Transport) {
  if (props.disabled || next === form.transport) return;
  snapshotCurrentGroup();
  form.transport = next;
  resetFormGroup(next);
  flush();
}

function onTransportSelect(event: Event) {
  const next = (event.target as HTMLSelectElement).value;
  switchTransport(next === "http" ? "http" : "stdio");
}

// ==================== JSON 模式 ====================

/** 由宿主切换视图；切到 JSON 时把当前定义灌进编辑框。 */
watch(
  () => props.mode,
  (next) => {
    if (next !== "json") return;
    jsonDraft.value = props.definitionJson;
    jsonError.value = "";
  },
  { immediate: true },
);

function onJsonDraftInput() {
  jsonError.value = "";
}

/** 失焦时才应用：边打字边重新解析会让光标乱跳，也会把写坏一半的 JSON 当成错。 */
function applyJsonDraft() {
  const text = jsonDraft.value.trim();
  if (!text) {
    jsonError.value = "";
    definitionTree = {};
    serverNode = {};
    parseIntoForm("{}");
    emitJson("{}");
    return;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    jsonError.value = `JSON 语法错误：${String((error as Error)?.message || error)}`;
    return;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    jsonError.value = "定义必须是一个 JSON 对象";
    return;
  }
  const root = parsed as Record<string, unknown>;
  const hasServers = !!root.mcpServers && typeof root.mcpServers === "object";
  const hasDirect = DIRECT_FIELD_KEYS.some((key) => key in root);
  if (!hasServers && !hasDirect) {
    jsonError.value = "缺少 mcpServers（或 command / url 等单 server 字段）";
    return;
  }
  jsonError.value = "";
  definitionTree = parsed;
  serverNode = locateServer(parsed);
  parseIntoForm(JSON.stringify(parsed));
  emitJson(JSON.stringify(parsed, null, 2));
}

// ==================== 与父组件同步 ====================

watch(
  () => props.definitionJson,
  (next) => {
    if (next === pendingJson) {
      pendingJson = "";
      return;
    }
    parseIntoForm(next);
    jsonDraft.value = next;
    jsonError.value = "";
  },
  { immediate: true },
);
</script>

