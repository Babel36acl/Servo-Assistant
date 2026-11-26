/* 录制页共用类型：录制状态、捕获状态、报文分页与 Modbus 事务解析结果。
   父视图与子组件共用，避免同一份结构声明两遍。 */

export type RecordItem = {
  sequence: number;
  timestampUs: number;
  source: string;
  protocol: string;
  direction: string;
  transaction: number;
  bytes: number[];
  detail: string;
};

export type Page = {
  records: RecordItem[];
  nextOffset: number | null;
  warning: string | null;
  total?: number;
  offset?: number;
};

export type ModbusFrame = {
  hex: string;
  checksum: string;
  slave: number | null;
  function: number | null;
  address: number | null;
  count: number | null;
  values: number[];
  exception: string | null;
  error: string | null;
};

export type ModbusTransaction = {
  source: string;
  protocol: string;
  transaction: number;
  outcome: string;
  durationUs: number | null;
  request: ModbusFrame;
  response: ModbusFrame;
  events: string[];
  warning: string | null;
};

export type RecordingStatus = {
  active: boolean;
  path: string;
  accepted: number;
  written: number;
  dropped: number;
  unwritten: number;
  bytes: number;
  error: string | null;
  armed?: boolean;
  discarded?: number;
  triggeredAtUs?: number | null;
};

export type CaptureStatus = {
  active: boolean;
  path: string;
  packets: number;
  bytes: number;
  driverDropped: number | null;
  error: string | null;
};
