/** 展示层格式化：纯函数，无状态。
 *  各领域 composable 与视图共用，避免同一个 hex / 风险文案出现两份实现。 */
import type { ParameterDefinition, SerialMode } from "../../types";

/** 地址统一写成 0x00FF（4 位大写十六进制）。 */
export function hex(value: number) {
  return `0x${value.toString(16).toUpperCase().padStart(4, "0")}`;
}

/** 参数风险等级 → 中文标签。 */
export function riskLabel(risk: ParameterDefinition["risk"]) {
  return { low: "低风险", medium: "中风险", high: "高风险", critical: "关键安全" }[risk];
}

/** 审计时间戳 → 本地时间（24 小时制，中文区域）。 */
export function formatTimestamp(timestampMs: number) {
  return new Date(timestampMs).toLocaleString("zh-CN", { hour12: false });
}

/** 串口模式 → RTU · 8E1 这类人读文案。 */
export function serialModeLabel(mode: SerialMode) {
  return `${mode.protocol.toUpperCase()} · 8${{ none: "N", even: "E", odd: "O" }[mode.parity]}${mode.stopBits}`;
}
