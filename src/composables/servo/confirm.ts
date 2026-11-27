/* 确认弹窗状态机：写入 / 应用 / 持久化 / 批量写入都要“点确认才放行”。
   弹层由 App.vue 的 KsuSheet 渲染，这里只持有请求、结果与焦点还原。
   调用方 await requestConfirm(...)：确认返回 true，取消或关闭返回 false。 */
import { nextTick, ref } from "vue";

export type ConfirmRequest = { title: string; body: string; danger?: boolean };

export function createConfirmDialog() {
  const confirmReq = ref<ConfirmRequest | null>(null);
  let confirmResolve: ((value: boolean) => void) | null = null;
  let confirmOpener: HTMLElement | null = null;

  function requestConfirm(req: ConfirmRequest): Promise<boolean> {
    confirmReq.value = req;
    confirmOpener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return new Promise((resolve) => {
      confirmResolve = resolve;
      void nextTick(() => {
        // 焦点落在“确认”按钮上，回车即可放行。
        const target = document.querySelector(".ksu-sheet-actions button:last-child");
        (target as HTMLElement | null)?.focus();
      });
    });
  }

  function closeConfirm(action: "submit" | null) {
    if (!confirmReq.value || !confirmResolve) return;
    confirmReq.value = null;
    const done = confirmResolve;
    confirmResolve = null;
    done(action === "submit");
    confirmOpener?.focus();
  }

  return {
    confirmReq,
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
