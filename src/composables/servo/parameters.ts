/* 参数领域：读取（按连续地址分组）、逐项写入并回读、快照比较与选择性批量写、
   应用 / 持久化事务。所有写路径都必须先走确认短语，且写前值变化即中止。 */
import { computed, ref } from "vue";
import { servoApi } from "../../api";
import { addressGroups } from "../../communication";
import type { ParameterDefinition, ParameterValue, SnapshotDiff } from "../../types";
import type { ServoCore } from "./core";
import type { createCommunication } from "./communication";
import type { createConfirmDialog } from "./confirm";
import type { createMonitoring } from "./monitoring";
import { hex, riskLabel } from "./format";

export function createParameters(
  core: ServoCore,
  communication: ReturnType<typeof createCommunication>,
  monitoring: ReturnType<typeof createMonitoring>,
  dialog: ReturnType<typeof createConfirmDialog>,
) {
  const values = ref<Record<string, ParameterValue>>({});
  const drafts = ref<Record<string, number>>({});
  const comparison = ref<SnapshotDiff[]>([]);
  const snapshotExportPath = ref("");
  const selectedBatch = ref<string[]>([]);
  const query = ref("");
  const selectedGroup = ref("全部");
  const readActive = ref(false);
  const readProgress = ref("");
  const failedGroups = ref<ParameterDefinition[][]>([]);
  const staleIds = ref(new Set<string>());

  const groups = computed(() => [
    "全部",
    ...new Set(core.profile.value?.parameters.map((item) => item.group) ?? []),
  ]);
  const comparisonById = computed(() => new Map(comparison.value.map((item) => [item.parameterId, item])));
  const changedDifferences = computed(() => comparison.value.filter((item) => item.changed));
  const canApply = computed(() => Boolean(core.profile.value?.operations?.apply));
  const canPersist = computed(() => Boolean(core.profile.value?.operations?.persist));

  const filteredParameters = computed(() => {
    const keyword = query.value.trim().toLowerCase();
    return (core.profile.value?.parameters ?? []).filter((parameter) => {
      const inGroup = selectedGroup.value === "全部" || parameter.group === selectedGroup.value;
      const searchable = `${parameter.parameterId} ${parameter.name} ${parameter.semanticId}`.toLowerCase();
      return inGroup && (!keyword || searchable.includes(keyword));
    });
  });

  function isDirty(parameter: ParameterDefinition) {
    const current = values.value[parameter.parameterId];
    return current && drafts.value[parameter.parameterId] !== current.value;
  }

  async function readAll(failedOnly = false) {
    if (!core.connected.value || core.busy.value) return;
    core.busy.value = true;
    try {
      await readParameterGroups(failedOnly);
    } finally {
      core.busy.value = false;
      monitoring.scheduleStatusPoll();
    }
  }

  async function readParameterGroups(failedOnly: boolean) {
    readActive.value = true;
    core.cancelRequested.value = false;
    core.errorMessage.value = "";
    core.notice.value = "正在读取参数";
    try {
      await monitoring.pauseForOperation();
      await communication.applyCommunicationSettings();
      const pending = failedOnly
        ? [...failedGroups.value]
        : addressGroups(core.profile.value?.parameters ?? [], communication.maxRegisters.value);
      failedGroups.value = [...pending];
      for (const group of pending) for (const item of group) staleIds.value.add(item.parameterId);
      for (let index = 0; index < pending.length && !core.cancelRequested.value && !core.lifecycle.disposed; index++) {
        const group = pending[index];
        readProgress.value = `${index + 1}/${pending.length} 地址组 · ${hex(group[0].address)}/${group.length}`;
        try {
          const result = await servoApi.readParameters(group.map((item) => item.parameterId));
          for (const item of result) {
            // A refresh must not discard the user's uncommitted edits.
            const edited =
              values.value[item.parameterId] &&
              drafts.value[item.parameterId] !== values.value[item.parameterId].value;
            values.value[item.parameterId] = item;
            if (!edited) drafts.value[item.parameterId] = item.value;
            staleIds.value.delete(item.parameterId);
          }
          failedGroups.value = failedGroups.value.filter(
            (item) => item[0].parameterId !== group[0].parameterId,
          );
        } catch (error) {
          core.showError(error);
        }
      }
      core.notice.value = `${core.cancelRequested.value ? "读取已取消" : "读取结束"}：${
        pending.length - failedGroups.value.length
      }/${pending.length} 地址组成功，${failedGroups.value.length} 组失败或未完成`;
    } catch (error) {
      core.notice.value = "参数读取未完成";
      core.showError(error);
    } finally {
      readActive.value = false;
      await communication.updateDiagnostics();
      await core.refreshAudit();
    }
  }

  async function exportSnapshot() {
    if (!core.connected.value) return;
    core.busy.value = true;
    core.errorMessage.value = "";
    core.notice.value = "请选择快照保存位置，确认后读取并导出";
    try {
      const result = await servoApi.exportSnapshot();
      if (!result) {
        core.notice.value = "已取消导出，未生成快照文件";
        return;
      }
      snapshotExportPath.value = result.path;
      core.notice.value = `已导出 ${result.parameterCount} 个参数，文件已保存至：${result.path}`;
    } catch (error) {
      core.notice.value = "快照文件导出未完成";
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function handleSnapshotFile(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    core.busy.value = true;
    core.errorMessage.value = "";
    try {
      comparison.value = await servoApi.compareSnapshot(await file.text());
      const selected = comparison.value.filter((item) => item.changed && item.writable);
      selectedBatch.value = selected.map((item) => item.parameterId);
      drafts.value = {
        ...drafts.value,
        ...Object.fromEntries(selected.map((item) => [item.parameterId, item.targetValue])),
      };
      core.notice.value = `快照比较完成：${changedDifferences.value.length} 项差异，${selected.length} 项可选择写入`;
    } catch (error) {
      core.showError(error);
    } finally {
      input.value = "";
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function writeSelectedBatch() {
    const selected = comparison.value.filter(
      (item) => item.changed && item.writable && selectedBatch.value.includes(item.parameterId),
    );
    if (!selected.length) return;
    const expectedPhrase = `批量写入 ${selected.length} 项`;
    const phrase = await dialog.requestConfirm({
      title: "选择性批量写入",
      body: `将按当前快照选择性写入并逐项回读，任何写前值变化都会让整批操作停止，共 ${selected.length} 项。`,
      danger: true,
      phrase: expectedPhrase,
    });
    if (phrase === null) return;
    core.busy.value = true;
    core.errorMessage.value = "";
    try {
      const result = await servoApi.batchWrite({
        items: selected.map((item) => ({
          parameterId: item.parameterId,
          value: item.targetValue,
          expectedRaw: item.currentRaw,
        })),
        confirmationPhrase: phrase,
      });
      for (const item of result.completed) {
        values.value[item.parameterId] = {
          parameterId: item.parameterId,
          raw: item.readBackRaw,
          value: item.value,
        };
        drafts.value[item.parameterId] = item.value;
      }
      selectedBatch.value = selectedBatch.value.filter(
        (id) => !result.completed.some((item) => item.parameterId === id),
      );
      if (result.error) {
        core.showError(
          `批量写入在 ${result.failedParameterId} 停止：${result.error}；此前 ${result.completed.length} 项已写入并回读。`,
        );
      } else {
        core.notice.value = `${result.completed.length} 项已写入并逐项回读一致`;
        comparison.value = [];
        selectedBatch.value = [];
      }
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  function clearComparison() {
    comparison.value = [];
    selectedBatch.value = [];
    for (const [parameterId, current] of Object.entries(values.value)) drafts.value[parameterId] = current.value;
  }

  async function writeOne(parameter: ParameterDefinition) {
    const current = values.value[parameter.parameterId];
    const value = Number(drafts.value[parameter.parameterId]);
    if (!current || !Number.isFinite(value)) {
      core.showError("请先读取参数并填写有效数值。");
      return;
    }
    const highRisk = parameter.risk === "high" || parameter.risk === "critical";
    const phrase = await dialog.requestConfirm({
      title: `写入 ${parameter.parameterId}`,
      body: `${parameter.parameterId}：${current.value} → ${value} ${parameter.unit}。写入后立即回读校验，不会自动应用或持久化。`,
      danger: highRisk,
      phrase: highRisk ? `写入 ${parameter.parameterId}` : undefined,
    });
    if (phrase === null) return;
    core.busy.value = true;
    core.errorMessage.value = "";
    try {
      const result = await servoApi.writeParameter({
        parameterId: parameter.parameterId,
        value,
        expectedRaw: current.raw,
        confirmed: true,
        confirmationPhrase: phrase,
      });
      values.value = {
        ...values.value,
        [parameter.parameterId]: {
          parameterId: parameter.parameterId,
          raw: result.readBackRaw,
          value: result.value,
        },
      };
      drafts.value = { ...drafts.value, [parameter.parameterId]: result.value };
      core.notice.value = `${parameter.parameterId} 写入成功，回读原始值 ${result.readBackRaw} 一致`;
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function applyChanges() {
    const phrase = await dialog.requestConfirm({
      title: "应用参数",
      body: "该操作会让已写入参数在设备上生效。",
      phrase: "应用参数",
    });
    if (phrase === null) return;
    core.busy.value = true;
    try {
      const result = await servoApi.apply(phrase);
      core.notice.value = `参数应用成功，设备状态 ${hex(result.observedStatus)}`;
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  async function persistChanges() {
    const phrase = await dialog.requestConfirm({
      title: "持久化参数",
      body: "即将执行 Profile 定义的持久化操作，会覆盖旧配置。",
      danger: true,
      phrase: "持久化参数",
    });
    if (phrase === null) return;
    core.busy.value = true;
    try {
      const result = await servoApi.persist(phrase);
      core.notice.value = `参数持久化成功，设备状态 ${hex(result.observedStatus)}`;
    } catch (error) {
      core.showError(error);
    } finally {
      core.busy.value = false;
      await core.refreshAudit();
    }
  }

  /** 新 Profile 导入：丢弃旧值 / 草稿 / 比较 / 失败分组，避免串到新设备。 */
  function resetParameterState() {
    values.value = {};
    drafts.value = {};
    staleIds.value = new Set();
    failedGroups.value = [];
    readProgress.value = "";
    comparison.value = [];
    selectedBatch.value = [];
    core.errorMessage.value = "";
  }

  return {
    values,
    drafts,
    comparison,
    snapshotExportPath,
    selectedBatch,
    query,
    selectedGroup,
    groups,
    comparisonById,
    changedDifferences,
    canApply,
    canPersist,
    filteredParameters,
    readActive,
    readProgress,
    failedGroups,
    staleIds,
    isDirty,
    riskLabel,
    readAll,
    readParameterGroups,
    exportSnapshot,
    handleSnapshotFile,
    writeSelectedBatch,
    clearComparison,
    writeOne,
    applyChanges,
    persistChanges,
    resetParameterState,
  };
}
