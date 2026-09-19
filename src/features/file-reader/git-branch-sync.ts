/**
 * 分支相对上游的同步状态（参照 GitLens 的判定口径）。
 *
 * 上游被删（missingUpstream）单独优先判：此时 ahead/behind 已无意义。
 * 没有上游时按 GitLens 的区分：仓库有远程且分支没推过 → 未发布；
 * 仓库压根没有远程 → 纯本地。
 */

export type BranchSyncStatus =
  | "ahead"
  | "behind"
  | "diverged"
  | "upToDate"
  | "missingUpstream"
  | "unpublished"
  | "local";

export type BranchSyncLike = {
  isRemote: boolean;
  upstream: string;
  upstreamMissing: boolean;
  ahead: number;
  behind: number;
};

/**
 * 判定一条本地分支的同步状态。
 * `hasRemotes` 由调用方给出（远程分支列表是否非空），用于区分未发布与纯本地。
 * 远程分支自身不参与这套状态，返回 null。
 */
export function branchSyncStatus(branch: BranchSyncLike, hasRemotes: boolean): BranchSyncStatus | null {
  if (branch.isRemote) return null;
  if (branch.upstreamMissing) return "missingUpstream";
  if (branch.upstream) {
    if (branch.ahead > 0 && branch.behind > 0) return "diverged";
    if (branch.ahead > 0) return "ahead";
    if (branch.behind > 0) return "behind";
    return "upToDate";
  }
  return hasRemotes ? "unpublished" : "local";
}

/** 状态对应的行内箭头（↑ 领先 / ↓ 落后）；分叉时两个都带 */
export function branchSyncArrows(status: BranchSyncStatus | null): { up: boolean; down: boolean } {
  switch (status) {
    case "ahead":
      return { up: true, down: false };
    case "behind":
      return { up: false, down: true };
    case "diverged":
      return { up: true, down: true };
    default:
      return { up: false, down: false };
  }
}

/** 同步状态的 DaisyUI 语义色类；纯本地与已同步不着色 */
export function branchSyncColorClass(status: BranchSyncStatus | null): string {
  switch (status) {
    case "ahead":
    case "unpublished":
      return "text-success";
    case "behind":
      return "text-error";
    case "diverged":
      return "text-warning";
    case "missingUpstream":
      return "text-error";
    default:
      return "";
  }
}
