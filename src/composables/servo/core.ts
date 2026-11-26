/* 应用内核：跨领域共享的状态与副作用出口。
   - 页面导航 / 后台活动文案（壳层读取）
   - Profile 与摘要（各领域只读）
   - connectionMode / connected（连接方式与会话状态，连接页 / 参数 / 监控都要读）
   - busy / notice / errorMessage / audit（所有操作都要写）
   - lifecycle：销毁标记，轮询与长操作据此停手
   这里不放具体业务（连接、参数、监控各有自己的模块）。 */
import { computed, ref, watch } from "vue";
import { servoApi } from "../../api";
import { pages } from "../../shell/activities";
import type { AuditEntry, ConnectionMode, ProfileSummary, ServoProfile } from "../../types";

export type PageId = (typeof pages)[number]["id"];

export function createServoCore() {
  const backgroundActivity = ref("");
  const activePage = ref<PageId>("connection");
  const communicationTab = ref<"recording" | "ethercat">("recording");
  const currentPage = computed(() => pages.find((page) => page.id === activePage.value)!);
  watch(activePage, () => {
    window.scrollTo({ top: 0 });
    document.getElementById("main")?.focus({ preventScroll: true });
  });

  const profile = ref<ServoProfile | null>(null);
  const summary = ref<ProfileSummary | null>(null);
  const connectionMode = ref<ConnectionMode>("simulator");
  /** 会话是否已建立（连接领域写，参数 / 监控 / 诊断读）。 */
  const connected = ref(false);
  const busy = ref(false);
  /** 顶部提示 / 底部 toast 文案（成功路径与失败路径分开：失败进 errorMessage）。 */
  const notice = ref("请导入设备 JSON Profile");
  const errorMessage = ref("");
  const audit = ref<AuditEntry[]>([]);
  /** 用户点了“取消当前操作”：长循环在每个分组边界检查。 */
  const cancelRequested = ref(false);
  const lifecycle = { disposed: false };

  function showError(error: unknown) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  }

  async function refreshAudit() {
    try {
      audit.value = await servoApi.getAuditLog();
    } catch {
      // Audit refresh must never hide the result of the primary operation.
    }
  }

  function dispose() {
    lifecycle.disposed = true;
    cancelRequested.value = true;
  }

  return {
    backgroundActivity,
    activePage,
    currentPage,
    communicationTab,
    profile,
    summary,
    connectionMode,
    connected,
    busy,
    notice,
    errorMessage,
    audit,
    cancelRequested,
    lifecycle,
    showError,
    refreshAudit,
    dispose,
    get disposed() {
      return lifecycle.disposed;
    },
  };
}

export type ServoCore = ReturnType<typeof createServoCore>;
