/* 录制页展示格式：原始字节串与微秒时间戳。
   父视图与子组件共用，避免同一份格式化逻辑出现两份。 */

/** 字节数组 → "01 03 00 10"（大写两位十六进制）。 */
export function hexBytes(bytes: number[]): string {
  return bytes.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(" ");
}

/** 微秒时间戳 → 本地时间 + 6 位微秒后缀。 */
export function timeLabel(us: number): string {
  return us ? `${new Date(us / 1000).toLocaleString()}.${String(us % 1000000).padStart(6, "0")}` : "时间未知";
}
