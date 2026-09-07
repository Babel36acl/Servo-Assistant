import { invoke } from "@tauri-apps/api/core";
import type {
  AuditEntry,
  SerialProtocol,
  ConnectionPreset,
  CommunicationStats,
  DiscoveryStatus,
  ProbeResult,
  BatchWriteResult,
  ConnectionMode,
  ConnectionStatus,
  ParameterValue,
  OperationResult,
  Parity,
  ProfileSummary,
  ParameterSnapshot,
  SnapshotDiff,
  SerialPortInfo,
  ServoProfile,
  StatusValue,
} from "./types";

export const servoApi = {
  discover(request: {
    connection: { protocol: SerialProtocol; preset: ConnectionPreset; mode: ConnectionMode; portName: string; slaveId: number; baudRate: number; parity: Parity; stopBits: number; timeoutMs: number };
    startSlave: number; endSlave: number;
  }) { return invoke<DiscoveryStatus>("discover_device", { request }); },
  discoveryStatus() { return invoke<DiscoveryStatus>("get_discovery_status"); },
  cancelDiscovery() { return invoke<void>("cancel_discovery"); },
  configureCommunication(settings: { retries: number; maxRegisters: number }) {
    return invoke<void>("configure_communication", { settings });
  },
  communicationStats() { return invoke<CommunicationStats>("get_communication_stats"); },
  probeRead(address: number, count: number) {
    return invoke<ProbeResult>("probe_read", { address, count });
  },
  importProfile(profileJson: string) {
    return invoke<ProfileSummary>("import_profile", { profileJson });
  },
  getProfile() {
    return invoke<ServoProfile | null>("get_active_profile");
  },
  listPorts() {
    return invoke<SerialPortInfo[]>("list_serial_ports");
  },
  connect(request: {
    protocol: SerialProtocol;
    preset: ConnectionPreset;
    mode: ConnectionMode;
    portName: string | null;
    slaveId: number;
    baudRate: number;
    parity: Parity;
    stopBits: number;
    timeoutMs: number;
  }) {
    return invoke<ConnectionStatus>("connect_device", { request });
  },
  disconnect() {
    return invoke<ConnectionStatus>("disconnect_device");
  },
  readParameters(parameterIds: string[] = []) {
    return invoke<ParameterValue[]>("read_parameters", { parameterIds });
  },
  captureSnapshot(label: string | null = null) {
    return invoke<ParameterSnapshot>("capture_parameter_snapshot", { label });
  },
  exportSnapshot() {
    return invoke<{ path: string; parameterCount: number } | null>("export_parameter_snapshot");
  },
  compareSnapshot(snapshotJson: string) {
    return invoke<SnapshotDiff[]>("compare_parameter_snapshot", { snapshotJson });
  },
  batchWrite(request: {
    items: Array<{ parameterId: string; value: number; expectedRaw: number }>;
    confirmationPhrase: string;
  }) {
    return invoke<BatchWriteResult>("batch_write_parameters", { request });
  },
  writeParameter(request: {
    parameterId: string;
    value: number;
    expectedRaw: number | null;
    confirmed: boolean;
    confirmationPhrase: string | null;
  }) {
    return invoke<{
      parameterId: string;
      previousRaw: number;
      writtenRaw: number;
      readBackRaw: number;
      value: number;
    }>("write_parameter", { request });
  },
  apply(confirmationPhrase: string) {
    return invoke<OperationResult>("apply_parameters", { confirmationPhrase });
  },
  persist(confirmationPhrase: string) {
    return invoke<OperationResult>("persist_parameters", { confirmationPhrase });
  },
  readStatuses() {
    return invoke<StatusValue[]>("read_statuses");
  },
  getAuditLog() {
    return invoke<AuditEntry[]>("get_audit_log");
  },
};
