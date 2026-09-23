import { inject, provide, ref, type InjectionKey, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";

export interface DirtyChecker {
  /** 当前是否有未保存的修改 */
  isDirty: () => boolean;
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

  function getFirstDirtyChecker(): DirtyChecker | null {
    // 先按显式 priority 降序，未指定的按 0；同优先级内按注册顺序逆序（后注册优先）。
    const list = Array.from(checkers.entries())
      .map(([id, checker], index) => ({ id, checker, index, priority: checker.priority ?? 0 }))
      .sort((a, b) => (b.priority - a.priority) || (b.index - a.index));
    const dirtyIds: string[] = [];
    let first: DirtyChecker | null = null;
    for (const { id, checker } of list) {
      if (checker.isDirty()) {
        dirtyIds.push(id);
        if (!first) first = checker;
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

  function confirmLeaveIfDirty(optionsOverride?: Partial<UnsavedConfirmDialogOptions>): Promise<boolean> {
    // 弹窗已打开时复用同一个 pending Promise；新的 override 不再生效，
    // 避免覆盖 pendingResolve 导致前一个调用方永久 pending。
    if (dialogOpen.value && pendingPromise) {
      return pendingPromise;
    }

    const dirtyChecker = getFirstDirtyChecker();
    if (!dirtyChecker) {
      return Promise.resolve(true);
    }

    activeChecker = {
      ...dirtyChecker,
      ...(optionsOverride?.onDiscard ? { onDiscard: optionsOverride.onDiscard } : {}),
      ...(optionsOverride?.onSave ? { onSave: optionsOverride.onSave } : {}),
    };

    dialogOptions.value = {
      title: optionsOverride?.title || dirtyChecker.title || t("config.unsavedConfirm.title"),
      message: optionsOverride?.message || dirtyChecker.message || t("config.unsavedConfirm.message"),
      confirmDiscardText: optionsOverride?.confirmDiscardText || t("config.unsavedConfirm.discard"),
      cancelText: optionsOverride?.cancelText || t("config.unsavedConfirm.stay"),
      saveAndLeaveText: optionsOverride?.saveAndLeaveText || t("config.unsavedConfirm.saveAndLeave"),
      canSaveAndLeave: optionsOverride?.canSaveAndLeave ?? typeof activeChecker.onSave === "function",
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
    pendingResolve?.(false);
    pendingResolve = null;
    pendingPromise = null;
  }

  async function handleDiscard() {
    try {
      if (activeChecker?.onDiscard) {
        await Promise.resolve(activeChecker.onDiscard());
      }
    } finally {
      dialogOpen.value = false;
      saving.value = false;
      activeChecker = null;
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
