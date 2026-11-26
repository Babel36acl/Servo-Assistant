/* 通讯设置与链路统计：重试次数 / 每组寄存器上限 / 串口统计。
   这些设置只在当前应用会话生效，不进 Profile。 */
import { ref } from "vue";
import { servoApi } from "../../api";
import type { CommunicationStats } from "../../types";
import type { ServoCore } from "./core";

export function createCommunication(core: ServoCore) {
  const retries = ref(2);
  const maxRegisters = ref(16);
  const communicationStats = ref<CommunicationStats | null>(null);

  /** 拉取串口统计；失败不能盖掉主操作的结果，所以静默。 */
  async function updateDiagnostics() {
    try {
      communicationStats.value = await servoApi.communicationStats();
    } catch {
      // Keep the primary operation's result.
    }
  }

  async function applyCommunicationSettings() {
    await servoApi.configureCommunication({ retries: retries.value, maxRegisters: maxRegisters.value });
  }

  async function saveCommunicationSettings() {
    if (core.busy.value) return;
    core.busy.value = true;
    try {
      await applyCommunicationSettings();
      core.notice.value = "通讯设置已应用（当前应用会话）";
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
    }
  }

  return {
    retries,
    maxRegisters,
    communicationStats,
    updateDiagnostics,
    applyCommunicationSettings,
    saveCommunicationSettings,
  };
}
