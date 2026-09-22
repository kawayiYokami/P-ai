import { inject, provide, ref, type InjectionKey, type Ref } from "vue";
import { useI18n } from "vue-i18n";

export interface DirtyChecker {
  /** 当前是否有未保存的修改 */
  isDirty: () => boolean;
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

  function registerDirtyChecker(id: string, checker: DirtyChecker): () => void {
    checkers.set(id, checker);
    return () => {
      checkers.delete(id);
    };
  }

  function getFirstDirtyChecker(): DirtyChecker | null {
    // 逆序查找，优先取最后注册的（通常为当前激活的子组件/详情页）
    const list = Array.from(checkers.values()).reverse();
    for (const checker of list) {
      if (checker.isDirty()) {
        return checker;
      }
    }
    return null;
  }

  function hasDirtyState(): boolean {
    return getFirstDirtyChecker() !== null;
  }

  function confirmLeaveIfDirty(optionsOverride?: Partial<UnsavedConfirmDialogOptions>): Promise<boolean> {
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

    return new Promise<boolean>((resolve) => {
      pendingResolve = resolve;
    });
  }

  function handleCancel() {
    dialogOpen.value = false;
    saving.value = false;
    activeChecker = null;
    pendingResolve?.(false);
    pendingResolve = null;
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
