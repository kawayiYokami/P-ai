export type ChannelConnectionStatus = {
  channelId: string;
  connected: boolean;
  peerAddr?: string;
  connectedAt?: string;
  listenAddr: string;
  statusText?: string;
  lastError?: string;
  accountId?: string;
  baseUrl?: string;
  loginSessionKey?: string;
  qrcodeUrl?: string;
};

export type ChannelLogEntry = {
  timestamp: string;
  level: string;
  message: string;
};

/** QQ 官方机器人扫码绑定：start 返回二维码与有效期。二维码由后端直出、不落文件。
 *  `qrImage` 是 `data:image/svg+xml;base64,...` 形式的 data URL，前端直接塞 `<img src>`
 *  （与微信扫码登录同一套写法）；`qrSvg` 是原始 svg 文本，保留字段但前端不再渲染。 */
export type QqOfficialBindStartResult = {
  taskId: string;
  qrSvg: string;
  qrImage: string;
  expireSeconds: number;
};

/** poll 的结果；confirmed 时后端把 appId/appSecret 交回前端回填表单。
 *  状态取值与后端 `qq_official/bind.rs` 一致：`waiting`（平台 0，还没人扫）/
 *  `scanned`（平台 1，已扫待手机确认）/ `confirmed`（平台 2）/ `expired`（平台 3）。 */
export type QqOfficialBindPollResult = {
  status: "waiting" | "confirmed" | "expired";
  appId?: string;
  appSecret?: string;
};

export type WeixinLoginStatus = {
  channelId: string;
  connected: boolean;
  status: string;
  message: string;
  sessionKey?: string;
  qrcode?: string;
  qrcodeImgContent?: string;
  accountId?: string;
  userId?: string;
  baseUrl?: string;
  lastError?: string;
};
