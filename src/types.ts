export type RawType = "u16" | "i16";
export type Access = "ro" | "rw";
export type RiskLevel = "low" | "medium" | "high" | "critical";
export type Parity = "none" | "even" | "odd";
export type ConnectionMode = "simulator" | "serial";

export interface EnumChoice {
  value: number;
  label: string;
}

export interface ParameterDefinition {
  semanticId: string;
  parameterId: string;
  name: string;
  group: string;
  address: number;
  rawType: RawType;
  decimals: number;
  unit: string;
  min: number;
  max: number;
  defaultValue: number;
  access: Access;
  risk: RiskLevel;
  requiresRestart: boolean;
  applicableModes: string[];
  enumValues: EnumChoice[];
  description: string;
}

export interface ServoProfile {
  schemaVersion: string;
  device: {
    id: string;
    name: string;
    profileVersion: string;
  };
  transport: {
    kind: string;
    defaultSlaveId: number;
    defaultBaudRate: number;
    allowedBaudRates: number[];
    dataBits: number;
    parity: Parity;
    stopBits: number;
    timeoutMs: number;
  };
  parameters: ParameterDefinition[];
  statuses: Array<{
    id: string;
    name: string;
    address: number;
    rawType: RawType;
    decimals: number;
    unit: string;
  }>;
  operations?: {
    commandRegister: number;
    statusRegister: number;
    apply?: OperationDefinition;
    persist?: OperationDefinition;
  };
}

export interface OperationDefinition {
  command: number;
  successStatus: number;
  timeoutMs: number;
}

export interface ProfileSummary {
  deviceId: string;
  deviceName: string;
  profileVersion: string;
  parameterCount: number;
  statusCount: number;
}

export interface ConnectionStatus {
  connected: boolean;
  mode: ConnectionMode | null;
  deviceName: string | null;
}

export interface ParameterValue {
  parameterId: string;
  raw: number;
  value: number;
}

export interface SnapshotValue extends ParameterValue {}

export interface ParameterSnapshot {
  schemaVersion: "servo-parameter-snapshot/1.0";
  createdAtMs: number;
  label: string;
  deviceId: string;
  deviceName: string;
  profileVersion: string;
  values: SnapshotValue[];
}

export interface SnapshotDiff {
  parameterId: string;
  name: string;
  currentRaw: number;
  currentValue: number;
  targetRaw: number;
  targetValue: number;
  changed: boolean;
  writable: boolean;
  risk: RiskLevel;
}

export interface WriteResult {
  parameterId: string;
  previousRaw: number;
  writtenRaw: number;
  readBackRaw: number;
  value: number;
}

export interface BatchWriteResult {
  completed: WriteResult[];
  failedParameterId: string | null;
  error: string | null;
}

export interface StatusValue {
  id: string;
  name: string;
  address: number;
  raw: number;
  value: number;
  unit: string;
}

export interface OperationResult {
  operation: string;
  command: number;
  observedStatus: number;
}

export interface AuditEntry {
  timestampMs: number;
  action: string;
  status: string;
  detail: string;
}

export interface SerialPortInfo {
  name: string;
  description: string;
}

export interface CommunicationStats {
  transactions: number;
  firstSuccesses: number;
  recovered: number;
  failed: number;
  crcErrors: number;
  timeouts: number;
  retries: number;
  lastSuccessMs: number | null;
  lastFailure: string | null;
}

export interface ProbeResult {
  success: boolean;
  attempts: number;
  elapsedMs: number;
  error: string | null;
}

export interface DiscoveryStatus {
  completed: number;
  total: number;
  slaveId: number | null;
  baudRate: number | null;
  found: boolean;
  cancelled: boolean;
}
