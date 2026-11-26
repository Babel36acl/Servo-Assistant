/* 确认弹窗状态机：写入 / 应用 / 持久化 / 批量写入都要“输入短语才放行”。
   弹层由 App.vue 的 KsuSheet 渲染，这里只持有请求、输入、错误与焦点还原。
   调用方 await requestConfirm(...)：确认返回短语字符串，取消返回 null。 */
import { nextTick, ref } from "vue";

export type ConfirmRequest = { title: string; body: string; danger?: boolean; phrase?: string };

export function createConfirmDialog() {
  const confirmReq = ref<ConfirmRequest | null>(null);
  const confirmInput = ref("");
  const confirmError = ref("");
  const confirmField = ref<HTMLInputElement | null>(null);
  let confirmResolve: ((value: string | null) => void) | null = null;
  let confirmOpener: HTMLElement | null = null;

  function requestConfirm(req: ConfirmRequest): Promise<string | null> {
    confirmReq.value = req;
    confirmInput.value = "";
    confirmError.value = "";
    confirmOpener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return new Promise((resolve) => {
      confirmResolve = resolve;
      void nextTick(() => {
        // 无短语输入框时把焦点放到确认按钮上（KsuSheet 的动作区）。
        const fallback = document.querySelector(".ksu-sheet-actions button:last-child");
        const target = (confirmField.value ?? fallback) as HTMLElement | null;
        target?.focus();
      });
    });
  }

  function closeConfirm(action: "submit" | null) {
    const req = confirmReq.value;
    if (!req || !confirmResolve) return;
    if (action === null) {
      confirmReq.value = null;
      const done = confirmResolve;
      confirmResolve = null;
      done(null);
      confirmOpener?.focus();
      return;
    }
    if (req.phrase && confirmInput.value !== req.phrase) {
      confirmError.value = `请输入“${req.phrase}”再继续`;
      confirmField.value?.focus();
      return;
    }
    confirmReq.value = null;
    const done = confirmResolve;
    confirmResolve = null;
    done(req.phrase ?? "ok");
    confirmOpener?.focus();
  }

  return {
    confirmReq,
    confirmInput,
    confirmError,
    confirmField,
    requestConfirm,
    closeConfirm,
    get confirmResolve() {
      return confirmResolve;
    },
    get confirmOpener() {
      return confirmOpener;
    },
  };
}
