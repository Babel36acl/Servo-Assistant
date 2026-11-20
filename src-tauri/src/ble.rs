//! BLE 无线串口桥（ServoBridge）传输层。
//!
//! 桥固件在独立仓库 ServoBridge（ESP32-C6 + 自动流向 TTL→RS485 隔离模块）：
//! NUS 透传 + CTRL 配置特性。BleTransport 实现 modbus::Transport，
//! SerialClient 的重试 / CRC / 录制逻辑零改动复用。
//!
//! 并发结构：blec 的读写是 async，而 SerialClient 是同步阻塞模型；两者之间用
//! 专用工作线程桥接——每个 BLE 链路一个 std 线程 + 私有小 runtime，同步侧经
//! job 队列发请求并等回执，通知流经 std mpsc 落进 read。不占 tauri runtime
//! 线程，也不会在异步上下文里反向阻塞。
//!
//! 连接生命周期：`drop` 时 `disconnect_on_drop=true` 的 transport 显式断开
//! BLE；false（自动查找复用链路）时只让工作线程安静退出，连接留给下一个
//! 探测配置复用，由调用方在收尾时 disconnect_all 收口。

use crate::modbus::{Protocol, SerialClient, Transport};
use crate::profile::ParitySetting;
use crate::recording::Recorder;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Nordic UART Service；RX 由本端写入，TX 以 notify 回流。
pub const NUS_SERVICE: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e);
pub const NUS_RX: Uuid = Uuid::from_u128(0x6e400002_b5a3_f393_e0a9_e50e24dcca9e);
pub const NUS_TX: Uuid = Uuid::from_u128(0x6e400003_b5a3_f393_e0a9_e50e24dcca9e);

/// 控制服务：带外配置桥的串口参数，不进 Modbus 字节流。
/// 6 字节：[0] 校验（0 无 / 1 偶 / 2 奇）[1] 停止位 [2..5] 波特率小端。
pub const CTRL_SERVICE: Uuid = Uuid::from_u128(0x9f1d0001_6e4e_4b52_8a53_2a7f4f3e1001);
pub const CTRL_CHAR: Uuid = Uuid::from_u128(0x9f1d0002_6e4e_4b52_8a53_2a7f4f3e1002);

fn ctrl_payload(baud: u32, parity: ParitySetting, stop_bits: u8) -> [u8; 6] {
    let parity_code = match parity {
        ParitySetting::None => 0u8,
        ParitySetting::Even => 1,
        ParitySetting::Odd => 2,
    };
    [
        parity_code,
        stop_bits,
        (baud & 0xff) as u8,
        ((baud >> 8) & 0xff) as u8,
        ((baud >> 16) & 0xff) as u8,
        ((baud >> 24) & 0xff) as u8,
    ]
}

pub(crate) fn bits_per_char(parity: ParitySetting, stop_bits: u8) -> u32 {
    1 + 8 + u32::from(!matches!(parity, ParitySetting::None)) + stop_bits as u32
}

enum Job {
    /// 整帧 Modbus 请求写入 NUS RX 特性。
    Write {
        data: Vec<u8>,
        done: SyncSender<io::Result<()>>,
    },
    /// 在线重配桥的串口参数（自动查找切换波特率用）。
    Configure {
        payload: [u8; 6],
        done: SyncSender<Result<(), String>>,
    },
    /// 显式断开并退出工作线程。
    Stop,
}

pub struct BleTransport {
    jobs: SyncSender<Job>,
    notifications: Receiver<Vec<u8>>,
    /// 已到达未读出的字节（对齐串口内核缓冲的语义）。
    pending: Vec<u8>,
    timeout: Duration,
    #[allow(dead_code)]
    address: String,
    alive: Arc<AtomicBool>,
    disconnect_on_drop: bool,
}

impl Transport for BleTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            if !self.alive.load(Ordering::SeqCst) {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "蓝牙连接已断开",
                ));
            }
            let chunk = self.notifications.recv_timeout(self.timeout).map_err(
                |error| match error {
                    RecvTimeoutError::Timeout => io::ErrorKind::TimedOut.into(),
                    RecvTimeoutError::Disconnected => io::Error::new(
                        io::ErrorKind::ConnectionAborted,
                        "蓝牙通知流已关闭",
                    ),
                },
            )?;
            self.pending.extend(chunk);
        }
        let count = buf.len().min(self.pending.len());
        buf[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        Ok(count)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "蓝牙连接已断开",
            ));
        }
        let (done_tx, done_rx) = sync_channel(1);
        self.jobs
            .send(Job::Write {
                data: buf.to_vec(),
                done: done_tx,
            })
            .map_err(|_| {
                io::Error::new(io::ErrorKind::ConnectionAborted, "蓝牙工作线程已退出")
            })?;
        done_rx
            .recv_timeout(self.timeout)
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "蓝牙写入超时"))?
    }

    fn flush(&mut self) -> io::Result<()> {
        // write 用的就是 WithResponse：写入完成即落到了对端 GATT 层。
        Ok(())
    }

    fn clear_input(&mut self) -> io::Result<()> {
        self.pending.clear();
        while let Ok(chunk) = self.notifications.try_recv() {
            self.pending.extend(chunk);
        }
        self.pending.clear();
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn name(&self) -> String {
        format!("ble:{}", self.address)
    }

    fn reconfigure(
        &mut self,
        baud: u32,
        parity: ParitySetting,
        stop_bits: u8,
    ) -> Result<(), String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err("蓝牙连接已断开".into());
        }
        let (done_tx, done_rx) = sync_channel(1);
        self.jobs
            .send(Job::Configure {
                payload: ctrl_payload(baud, parity, stop_bits),
                done: done_tx,
            })
            .map_err(|_| "蓝牙工作线程已退出".to_string())?;
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "蓝牙配置写入超时".to_string())?
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        if self.disconnect_on_drop {
            let _ = self.jobs.send(Job::Stop);
        }
        // 不断开时：jobs 随字段一起释放，工作线程在 recv Err 后安静退出，
        // BLE 链路留给下一次探测复用，由调用方收尾统一断开。
    }
}

/// 建立到 ServoBridge 的 BLE 会话所需的全部参数。集中成结构体，
/// 避免 open_client 触发 clippy::too_many_arguments（上限 7 个）。
pub struct BleConnectOptions {
    pub address: String,
    pub slave_id: u8,
    pub protocol: Protocol,
    pub baud: u32,
    pub parity: ParitySetting,
    pub stop_bits: u8,
    pub timeout: Duration,
    /// false 时供自动查找复用链路，连接留给下一次探测，
    /// 结束时由调用方调 `disconnect_all` 收口。
    pub disconnect_on_drop: bool,
}

/// 建立到 ServoBridge 的 BLE 会话并包装成 SerialClient。
///
/// 已连接同地址时复用现有链路（自动查找切波特率不走断开重连）。
pub fn open_client(
    options: BleConnectOptions,
    recorder: &Recorder,
) -> Result<SerialClient, String> {
    let handler = blec::get_handler().map_err(|error| format!("蓝牙未初始化：{error}"))?;
    let (job_tx, job_rx) = sync_channel::<Job>(8);
    let (notif_tx, notif_rx) = sync_channel::<Vec<u8>>(256);
    let alive = Arc::new(AtomicBool::new(true));
    let (setup_tx, setup_rx) = sync_channel::<Result<(), String>>(1);
    let addr = options.address;
    let addr_label = addr.clone();
    let payload = ctrl_payload(options.baud, options.parity, options.stop_bits);
    let alive_cb = alive.clone();

    std::thread::Builder::new()
        .name("ble-bridge".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = setup_tx.send(Err(error.to_string()));
                    return;
                }
            };
            let setup = runtime.block_on(async {
                if !handler.is_device_connected(&addr) {
                    handler
                        .connect(
                            &addr,
                            blec::OnDisconnectHandler::from_sync(move || {
                                alive_cb.store(false, Ordering::SeqCst);
                            }),
                            false,
                        )
                        .await
                        .map_err(|error| format!("连接 {addr} 失败：{error}"))?;
                }
                // 重新订阅前清旧回调：链路可能带着上一轮工作线程的订阅残留。
                let _ = handler
                    .unsubscribe_from(&addr, NUS_TX, Some(NUS_SERVICE))
                    .await;
                let forward = notif_tx.clone();
                handler
                    .subscribe_to(&addr, NUS_TX, Some(NUS_SERVICE), move |data| {
                        let _ = forward.send(data);
                    })
                    .await
                    .map_err(|error| format!("订阅桥通知失败：{error}"))?;
                handler
                    .send_data_to(
                        &addr,
                        CTRL_CHAR,
                        Some(CTRL_SERVICE),
                        &payload,
                        blec::models::WriteType::WithResponse,
                    )
                    .await
                    .map_err(|error| format!("写入桥配置失败：{error}"))?;
                Ok(())
            });
            if setup_tx.send(setup).is_err() {
                return; // 主线程已放弃（setup 超时），静默退出。
            }
            loop {
                let Ok(job) = job_rx.recv() else {
                    // 通道关闭：安静退出，连接留给复用方。
                    break;
                };
                match job {
                    Job::Write { data, done } => {
                        let result = runtime
                            .block_on(handler.send_data_to(
                                &addr,
                                NUS_RX,
                                Some(NUS_SERVICE),
                                &data,
                                blec::models::WriteType::WithResponse,
                            ))
                            .map_err(|error| {
                                io::Error::new(
                                    io::ErrorKind::ConnectionAborted,
                                    format!("蓝牙写入失败：{error}"),
                                )
                            });
                        let _ = done.send(result);
                    }
                    Job::Configure { payload, done } => {
                        let result = runtime
                            .block_on(handler.send_data_to(
                                &addr,
                                CTRL_CHAR,
                                Some(CTRL_SERVICE),
                                &payload,
                                blec::models::WriteType::WithResponse,
                            ))
                            .map(|_| ())
                            .map_err(|error| format!("蓝牙配置失败：{error}"));
                        let _ = done.send(result);
                    }
                    Job::Stop => {
                        let _ = runtime.block_on(handler.disconnect_device(&addr));
                        break;
                    }
                }
            }
        })
        .map_err(|error| error.to_string())?;

    setup_rx
        .recv_timeout(Duration::from_secs(30))
        .map_err(|_| "蓝牙连接超时（请确认桥已上电并靠近）".to_string())??;

    let transport = BleTransport {
        jobs: job_tx,
        notifications: notif_rx,
        pending: Vec::new(),
        timeout: options.timeout,
        address: addr_label,
        alive,
        disconnect_on_drop: options.disconnect_on_drop,
    };
    let bits = bits_per_char(options.parity, options.stop_bits);
    Ok(SerialClient::new(
        Box::new(transport),
        options.slave_id,
        options.baud,
        bits,
    )
    .with_protocol(options.protocol)
    .with_recorder(recorder.clone()))
}

/// 断开全部 BLE 链路（自动查找收尾调用）。
pub fn disconnect_all() {
    let Ok(handler) = blec::get_handler() else { return };
    std::thread::spawn(move || {
        if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            let _ = runtime.block_on(handler.disconnect_all());
        }
    });
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleDeviceInfo {
    pub address: String,
    pub name: Option<String>,
}

/// 扫描 BLE 设备，`name_contains` 按名称子串过滤（大小写不敏感）。
pub async fn scan_devices(
    name_contains: Option<String>,
    timeout_ms: u64,
) -> Result<Vec<BleDeviceInfo>, String> {
    let handler = blec::get_handler()
        .map_err(|error| format!("蓝牙不可用：适配器未就绪或权限被拒（{error}）"))?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    handler
        .discover(
            Some(tx),
            timeout_ms,
            blec::models::ScanFilter::None,
            false,
        )
        .await
        .map_err(|error| format!("启动蓝牙扫描失败：{error}"))?;
    let mut seen: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    while let Some(devices) = rx.recv().await {
        for device in devices {
            seen.entry(device.address.clone())
                .or_insert_with(|| Some(device.name.clone()));
        }
    }
    let needle = name_contains.map(|text| text.to_lowercase());
    let mut devices: Vec<BleDeviceInfo> = seen
        .into_iter()
        .map(|(address, name)| BleDeviceInfo { address, name })
        .filter(|device| match &needle {
            Some(needle) => device
                .name
                .as_deref()
                .is_some_and(|name| name.to_lowercase().contains(needle)),
            None => true,
        })
        .collect();
    devices.sort_by(|a, b| a.address.cmp(&b.address));
    Ok(devices)
}
