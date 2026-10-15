export type FileReaderFileKind = "markdown" | "code" | "image" | "audio" | "video" | "unsupported";

export type FileReaderFilePayload = {
  path: string;
  name: string;
  extension: string;
  kind: FileReaderFileKind | string;
  content: string;
  forcePlain?: boolean;
  virtualized?: boolean;
  totalLines?: number;
  blockLineCount?: number;
  /** 读取时的文件字节数，与 modifiedMs 一起构成内容来源签名 */
  len?: number;
  /** 读取时的文件修改时间（毫秒时间戳） */
  modifiedMs?: number;
};

/** 文件当前签名，仅用于判断磁盘内容相对已读内容是否变化 */
export type FileReaderFileSignature = {
  len: number;
  modifiedMs: number;
};

export type FileReaderFileStatPayload = {
  path: string;
  exists: boolean;
  len: number;
  modifiedMs: number;
};

export type FileReaderFileBlockPayload = {
  path: string;
  startLine: number;
  endLine: number;
  content: string;
};

export type FileReaderDirectoryEntry = {
  path: string;
  name: string;
  isDirectory: boolean;
};

export type FileReaderDirectoryPayload = {
  path: string;
  name: string;
  entries: FileReaderDirectoryEntry[];
};

export type GitDiffTabSource = {
  workspacePath: string;
  path: string;
  staged: boolean;
  hash?: string;
  /** 未跟踪文件（git ??）：无 git diff 内容，直接打开文件本体 */
  untracked?: boolean;
  /** 当前使用的 -U 上下文行数；0 表示 git 默认(3)，点"展开后续"后递增 */
  context?: number;
};

export type FileTab = {
  path: string;
  title: string;
  extension: string;
  kind: FileReaderFileKind | string;
  content: string;
  rawMode: boolean;
  forcePlain: boolean;
  virtualized: boolean;
  totalLines: number;
  blockLineCount: number;
  loaded: boolean;
  loading: boolean;
  error: string;
  diffSource?: GitDiffTabSource;
  /** 当前内容对应的磁盘签名；切入标签时据此判断是否需要重读 */
  signature?: FileReaderFileSignature | null;
  /** 离开该标签时的滚动位置（像素），仅运行时记忆 */
  scrollTop?: number;
};

export type VirtualCodeBlock = {
  key: string;
  path: string;
  startLine: number;
  endLine: number;
  lineCount: number;
};

export type DirectoryNode = {
  path: string;
  name: string;
  entries: FileReaderDirectoryEntry[];
  loaded: boolean;
  loading: boolean;
  error: string;
  expanded: boolean;
};

export type TreeRow =
  | { kind: "entry"; key: string; depth: number; entry: FileReaderDirectoryEntry }
  | { kind: "status"; key: string; depth: number; text: string };

export type FileReaderSessionState = {
  tabs?: string[];
  activePath?: string;
  directoryRootPath?: string;
  directoryTreeWidth?: number;
  /** 左侧栏模式：文件 / git，会话级记忆，下次自动恢复 */
  asideMode?: "files" | "git";
};

export type FileReaderWatchTarget = {
  path: string;
  kind: "file" | "directory";
};

export type FileReaderWatchEventPayload = {
  sessionId: string;
  path: string;
  kind: "file" | "directory";
};
