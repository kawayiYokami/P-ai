import type { ApiRequestFormat } from "../../../types/app";

const API_REQUEST_FORMATS = new Set<ApiRequestFormat>([
  "auto",
  "openai",
  "deepseek",
  "deepseek/kimi",
  "openai_responses",
  "codex",
  "gemini",
  "anthropic",
  "fireworks",
  "together",
  "groq",
  "mimo",
  "nebius",
  "xai",
  "zai",
  "bigmodel",
  "aliyun",
  "cohere",
  "ollama",
  "ollama_cloud",
  "vertex",
  "github_copilot",
  "openai_tts",
  "openai_stt",
  "openai_embedding",
  "openai_rerank",
  "gemini_embedding",
]);

const API_REQUEST_FORMAT_ALIASES: Record<string, ApiRequestFormat> = {
  default: "auto",
  automatic: "auto",
  "openai-compatible": "openai",
  openai_compatible: "openai",
  "openai compatible": "openai",
  openaicompatible: "openai",
  compatible_openai: "openai",
  openai_compat: "openai",
  "openai-compat": "openai",
  deepseek_kimi: "deepseek",
  "deepseek-kimi": "deepseek",
  "deepseek/kimi": "deepseek",
  openai_responses: "openai_responses",
  "openai-responses": "openai_responses",
  responses: "openai_responses",
  claude: "anthropic",
  google: "gemini",
  google_gemini: "gemini",
  "google-gemini": "gemini",
  "ollama-cloud": "ollama_cloud",
  "github-copilot": "github_copilot",
  stt: "openai_stt",
  tts: "openai_tts",
  embedding: "openai_embedding",
  embeddings: "openai_embedding",
  rerank: "openai_rerank",
  "openai-tts": "openai_tts",
  "openai-stt": "openai_stt",
  "openai-embedding": "openai_embedding",
  "openai-rerank": "openai_rerank",
  "gemini-embedding": "gemini_embedding",
};

export function normalizeApiRequestFormat(value: unknown, fallback: ApiRequestFormat = "auto"): ApiRequestFormat {
  const raw = String(value ?? "").trim().toLowerCase();
  if (!raw) return fallback;
  const normalized = raw.replace(/\s+/g, "_").replace(/-/g, "_");
  return (
    API_REQUEST_FORMAT_ALIASES[raw]
    ?? API_REQUEST_FORMAT_ALIASES[normalized]
    ?? (API_REQUEST_FORMATS.has(raw as ApiRequestFormat) ? (raw as ApiRequestFormat) : undefined)
    ?? (API_REQUEST_FORMATS.has(normalized as ApiRequestFormat) ? (normalized as ApiRequestFormat) : undefined)
    ?? fallback
  );
}
