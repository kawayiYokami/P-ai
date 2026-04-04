export type UpdateRuntimeKind = "installer" | "portable";

export type GithubUpdateInfo = {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseUrl: string;
  updateSource: string;
  releaseNotes: string;
  publishedAt?: string;
  runtimeKind: UpdateRuntimeKind;
  canForceUpdate: boolean;
};

export type UpdateProgressPayload = {
  stage: string;
  message: string;
  runtimeKind: UpdateRuntimeKind;
  currentVersion?: string;
  targetVersion?: string;
  downloadedBytes?: number;
  contentLength?: number;
  percent?: number;
  error?: string;
};
