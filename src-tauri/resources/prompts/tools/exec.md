exec 是一次性进程，命令结束、失败或超时后进程树就被回收，没有可依赖的持久 shell。
exec 需要命令持续运行时用 mode: background 起，它会返回 job id，完成时系统主动通知，用 background 工具的 list、status、kill 管理，不要轮询也不要 sleep 等。
exec 每次调用都带 description 说明用途与预期影响，缺了会被直接拦截。
改文件用 write、update、delete、move，不要在 exec 里用 cat、echo、重定向、heredoc 或 python 写文件。
测试与构建只跑与本次改动直接相关的最小必要检查。
