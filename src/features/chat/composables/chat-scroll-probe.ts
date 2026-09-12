// import { invokeTauri } from "../../../services/tauri-api";

/**
 * 滚动链路临时探针：把滚动写入点写进运行日志（backend.log），用于定位
 * 「发送消息后被强制贴底」到底是哪个机制触发。
 *
 * 排障已完成（2026-09-13），当前停用、保留实现以便以后复用：
 * 需要时把下面的 import 与函数体一起取消注释即可，调用点无需改动。
 */
export function probeChatScroll(tag: string, data?: Record<string, unknown>): void {
  // let detail = "";
  // if (data) {
  //   try {
  //     detail = ` ${JSON.stringify(data)}`;
  //   } catch {
  //     detail = " [unserializable]";
  //   }
  // }
  // try {
  //   void invokeTauri<boolean>("append_runtime_log_probe", {
  //     message: `[滚动诊断] ${tag}${detail}`,
  //   }).catch(() => {});
  // } catch {
  //   // 忽略：诊断探针失败不得影响主链路
  // }
  void tag;
  void data;
}
