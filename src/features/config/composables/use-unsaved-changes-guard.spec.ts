import { describe, expect, it, vi } from "vitest";
import { createUnsavedChangesGuard } from "./use-unsaved-changes-guard";

vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

describe("useUnsavedChangesGuard", () => {
  it("无 dirty checker 时应直接允许离开且不打开弹窗", async () => {
    const guard = createUnsavedChangesGuard();

    const result = await guard.confirmLeaveIfDirty();

    expect(result).toBe(true);
    expect(guard.dialogOpen.value).toBe(false);
  });

  it("checker 为 dirty 时应打开弹窗，用户点击取消时返回 false", async () => {
    const guard = createUnsavedChangesGuard();
    guard.registerDirtyChecker("test-1", {
      isDirty: () => true,
    });

    const leavePromise = guard.confirmLeaveIfDirty();
    expect(guard.dialogOpen.value).toBe(true);

    guard.handleCancel();
    const result = await leavePromise;

    expect(result).toBe(false);
    expect(guard.dialogOpen.value).toBe(false);
  });

  it("用户点击放弃修改时应触发 onDiscard 并返回 true", async () => {
    const guard = createUnsavedChangesGuard();
    const onDiscard = vi.fn();
    guard.registerDirtyChecker("test-2", {
      isDirty: () => true,
      onDiscard,
    });

    const leavePromise = guard.confirmLeaveIfDirty();
    expect(guard.dialogOpen.value).toBe(true);

    await guard.handleDiscard();
    const result = await leavePromise;

    expect(result).toBe(true);
    expect(onDiscard).toHaveBeenCalledTimes(1);
    expect(guard.dialogOpen.value).toBe(false);
  });

  it("用户点击保存并离开时应触发 onSave 并返回 true", async () => {
    const guard = createUnsavedChangesGuard();
    const onSave = vi.fn().mockResolvedValue(true);
    guard.registerDirtyChecker("test-3", {
      isDirty: () => true,
      onSave,
    });

    const leavePromise = guard.confirmLeaveIfDirty();
    expect(guard.dialogOptions.value.canSaveAndLeave).toBe(true);

    await guard.handleSaveAndLeave();
    const result = await leavePromise;

    expect(result).toBe(true);
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(guard.dialogOpen.value).toBe(false);
  });

  it("保存失败（onSave 返回 false）时不应关闭弹窗且不 resolve 离开", async () => {
    const guard = createUnsavedChangesGuard();
    const onSave = vi.fn().mockResolvedValue(false);
    guard.registerDirtyChecker("test-4", {
      isDirty: () => true,
      onSave,
    });

    void guard.confirmLeaveIfDirty();
    await guard.handleSaveAndLeave();

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(guard.dialogOpen.value).toBe(true);

    // 后续用户点击取消
    guard.handleCancel();
    expect(guard.dialogOpen.value).toBe(false);
  });

  it("注销 checker 后不再触发拦截", async () => {
    const guard = createUnsavedChangesGuard();
    const unregister = guard.registerDirtyChecker("test-5", {
      isDirty: () => true,
    });

    expect(guard.hasDirtyState()).toBe(true);
    unregister();
    expect(guard.hasDirtyState()).toBe(false);

    const result = await guard.confirmLeaveIfDirty();
    expect(result).toBe(true);
    expect(guard.dialogOpen.value).toBe(false);
  });
});
