import { inject, provide, ref, type InjectionKey, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";

/** 一处被判定为已修改的字段。仅 dev 模式用于在弹窗里定位误报来源。 */
export interface DirtyFieldDiff {
  /** 字段路径，如 `apiProviders[0].apiKeys` */
  path: string;
  /** 旧值（已截断、敏感字段已脱敏）；无法取值时省略 */
  before?: string;
  /** 新值（已截断、敏感字段已脱敏）；无法取值时省略 */
  after?: string;
}

export interface DirtyChecker {
  /** 当前是否有未保存的修改 */
  isDirty: () => boolean;
  /**
   * 仅 dev 模式使用：列出被判定为已修改的字段，用于在「未保存的更改」弹窗里
   * 定位误报。生产构建不会调用。不实现则弹窗里不出现清单。
   */
  describeDirty?: () => DirtyFieldDiff[];
  /**
   * 显式优先级：数值越大越先被选中。未指定时按注册顺序逆序兜底（后注册优先，
   * 兼容旧行为）；提供后优先按 priority 排序，再按注册顺序逆序。
   *
   * 约定：api-provider 等子详情 checker 应传大于全局的值，确保「保存并离开」
   * 先落到具体草稿的 onSave，而不是全局 config.onSave。
   */
  priority?: number;
  /** 可选：弹窗自定义标题 */
  title?: string;
  /** 可选：弹窗自定义提示内容 */
  message?: string;
  /** 用户确认放弃修改时的清理动作 */
  onDiscard?: () => void | Promise<void>;
  /** 用户点击“保存并离开”时的保存动作，返回 false 表示保存失败，中断离开 */
  onSave?: () => Promise<boolean | void>;
}

export interface UnsavedConfirmDialogOptions {
  title?: string;
  message?: string;
  confirmDiscardText?: string;
  cancelText?: string;
  saveAndLeaveText?: string;
  canSaveAndLeave?: boolean;
  /** 仅 dev 模式：被判定为已修改的字段清单，用于定位误报 */
  dirtyDetails?: DirtyFieldDiff[];
  onDiscard?: () => void | Promise<void>;
  onSave?: () => Promise<boolean | void>;
}

export interface UnsavedChangesGuardContext {
  /** 注册一个脏状态检查器，返回注销函数 */
  registerDirtyChecker: (id: string, checker: DirtyChecker) => () => void;
  /** 检查是否有任何未保存的修改，若有则弹出确认对话框并等待用户操作 */
  confirmLeaveIfDirty: (optionsOverride?: Partial<UnsavedConfirmDialogOptions>) => Promise<boolean>;
  /** 是否存在未保存的修改 */
  hasDirtyState: () => boolean;
  /** 弹窗是否打开 */
  dialogOpen: Ref<boolean>;
  /** 弹窗配置 */
  dialogOptions: Ref<UnsavedConfirmDialogOptions>;
  /** 弹窗内部操作：正在保存 */
  saving: Ref<boolean>;
  /** 弹窗内部操作：用户点击“保存并离开” */
  handleSaveAndLeave: () => Promise<void>;
  /** 弹窗内部操作：用户点击“放弃修改” */
  handleDiscard: () => Promise<void>;
  /** 弹窗内部操作：用户点击“继续编辑/取消” */
  handleCancel: () => void;
}

const UNSAVED_GUARD_KEY: InjectionKey<UnsavedChangesGuardContext> = Symbol("UNSAVED_CHANGES_GUARD");

export function createUnsavedChangesGuard(): UnsavedChangesGuardContext {
  const { t } = useI18n();
  const checkers = new Map<string, DirtyChecker>();

  const dialogOpen = ref(false);
  const dialogOptions = ref<UnsavedConfirmDialogOptions>({});
  const saving = ref(false);

  let activeChecker: DirtyChecker | null = null;
  let activeCheckerId: string | null = null;
  let pendingResolve: ((confirmed: boolean) => void) | null = null;
  // 保存当前正在等待用户确认的 Promise；dialog 已打开时再次调用 confirmLeaveIfDirty
  // 复用同一个 Promise，避免覆盖 pendingResolve 导致前一个调用方永久 pending。
  let pendingPromise: Promise<boolean> | null = null;

  function registerDirtyChecker(id: string, checker: DirtyChecker): () => void {
    checkers.set(id, checker);
    return () => {
      checkers.delete(id);
    };
  }

  function getFirstDirtyChecker(): { id: string; checker: DirtyChecker } | null {
    // 先按显式 priority 降序，未指定的按 0；同优先级内按注册顺序逆序（后注册优先）。
    const list = Array.from(checkers.entries())
      .map(([id, checker], index) => ({ id, checker, index, priority: checker.priority ?? 0 }))
      .sort((a, b) => (b.priority - a.priority) || (b.index - a.index));
    const dirtyIds: string[] = [];
    let first: { id: string; checker: DirtyChecker } | null = null;
    for (const { id, checker } of list) {
      if (checker.isDirty()) {
        dirtyIds.push(id);
        if (!first) first = { id, checker };
      }
    }
    if (dirtyIds.length > 0) {
      void invokeTauri("append_runtime_log_probe", {
        message: `[unsaved-guard] dirty checkers: ${JSON.stringify(dirtyIds)} → picking ${dirtyIds[0]}`,
        level: "debug",
      }).catch(() => {});
    }
    return first;
  }

  function hasDirtyState(): boolean {
    return getFirstDirtyChecker() !== null;
  }

  /**
   * 仅 dev 模式：取当前 checker 的字段级明细。明细纯属排障，
   * 算不出来不能影响弹窗本身，所以一律吞掉异常。
   */
  function collectDirtyDetails(checker: DirtyChecker): DirtyFieldDiff[] | undefined {
    if (!import.meta.env.DEV) return undefined;
    if (typeof checker.describeDirty !== "function") return undefined;
    try {
      const fields = checker.describeDirty();
      return fields.length > 0 ? fields : undefined;
    } catch {
      return undefined;
    }
  }

  function confirmLeaveIfDirty(optionsOverride?: Partial<UnsavedConfirmDialogOptions>): Promise<boolean> {
    // 弹窗已打开时复用同一个 pending Promise；新的 override 不再生效，
    // 避免覆盖 pendingResolve 导致前一个调用方永久 pending。
    if (dialogOpen.value && pendingPromise) {
      return pendingPromise;
    }

    const dirty = getFirstDirtyChecker();
    if (!dirty) {
      return Promise.resolve(true);
    }
    const dirtyChecker = dirty.checker;

    activeCheckerId = dirty.id;
    activeChecker = {
      ...dirtyChecker,
      ...(optionsOverride?.onDiscard ? { onDiscard: optionsOverride.onDiscard } : {}),
      ...(optionsOverride?.onSave ? { onSave: optionsOverride.onSave } : {}),
    };

    const dirtyDetails = collectDirtyDetails(activeChecker);

    dialogOptions.value = {
      title: optionsOverride?.title || dirtyChecker.title || t("config.unsavedConfirm.title"),
      message: optionsOverride?.message || dirtyChecker.message || t("config.unsavedConfirm.message"),
      confirmDiscardText: optionsOverride?.confirmDiscardText || t("config.unsavedConfirm.discard"),
      cancelText: optionsOverride?.cancelText || t("config.unsavedConfirm.stay"),
      saveAndLeaveText: optionsOverride?.saveAndLeaveText || t("config.unsavedConfirm.saveAndLeave"),
      canSaveAndLeave: optionsOverride?.canSaveAndLeave ?? typeof activeChecker.onSave === "function",
      ...(dirtyDetails ? { dirtyDetails } : {}),
    };

    dialogOpen.value = true;

    pendingPromise = new Promise<boolean>((resolve) => {
      pendingResolve = resolve;
    });
    return pendingPromise;
  }

  function handleCancel() {
    dialogOpen.value = false;
    saving.value = false;
    activeChecker = null;
    activeCheckerId = null;
    pendingResolve?.(false);
    pendingResolve = null;
    pendingPromise = null;
  }

  async function handleDiscard() {
    try {
      // 「放弃修改」要清干净所有 dirty checker 的草稿，不能只跑选中的那个——
      // 否则下一个 dirty checker 还会再弹一次窗，表现为「第一次放弃没生效」。
      // 先跑当前 activeChecker（用户刚刚确认的那个），再扫一遍其它仍 dirty 的 checker。
      if (activeChecker?.onDiscard) {
        await Promise.resolve(activeChecker.onDiscard());
      }
      for (const [id, checker] of checkers.entries()) {
        // activeChecker 是合并 override 后的拷贝，引用不同——用注册 id 去重。
        if (id === activeCheckerId) continue;
        if (!checker.isDirty()) continue;
        if (checker.onDiscard) {
          await Promise.resolve(checker.onDiscard());
        }
      }
    } finally {
      dialogOpen.value = false;
      saving.value = false;
      activeChecker = null;
      activeCheckerId = null;
      pendingResolve?.(true);
      pendingResolve = null;
      pendingPromise = null;
    }
  }

  async function handleSaveAndLeave() {
    if (!activeChecker?.onSave) {
      await handleDiscard();
      return;
    }
    saving.value = true;
    try {
      const res = await activeChecker.onSave();
      if (res === false) {
        // 保存明确返回 false 说明校验或操作失败，中止离开
        return;
      }
      dialogOpen.value = false;
      activeChecker = null;
      activeCheckerId = null;
      pendingResolve?.(true);
      pendingResolve = null;
      pendingPromise = null;
    } finally {
      saving.value = false;
    }
  }

  return {
    registerDirtyChecker,
    confirmLeaveIfDirty,
    hasDirtyState,
    dialogOpen,
    dialogOptions,
    saving,
    handleSaveAndLeave,
    handleDiscard,
    handleCancel,
  };
}

export function provideUnsavedChangesGuard(): UnsavedChangesGuardContext {
  const guard = createUnsavedChangesGuard();
  provide(UNSAVED_GUARD_KEY, guard);
  return guard;
}

export function useUnsavedChangesGuard(): UnsavedChangesGuardContext {
  const injected = inject(UNSAVED_GUARD_KEY, null);
  if (injected) return injected;
  return createUnsavedChangesGuard();
}
