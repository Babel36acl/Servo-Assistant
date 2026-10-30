## 改了什么



## 怎么验证（至少勾前三项）

- [ ] `pnpm lint:tokens` 通过（新增样式无散写色值）
- [ ] `pnpm build` 通过（含 `vue-tsc --noEmit`）
- [ ] `pnpm test` 25/25 通过
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 通过
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过

## 额外说明

- [ ] 移动了 `src/` 下的文件 → 同步改了 `tests/*.test.mjs` 夹具里的源码路径
- [ ] 改了 UI → 附了截图
- [ ] 改了 Profile/录制/EtherCAT 契约 → 更新了 `docs/` 对应说明
- [ ] 改了 EtherCAT 相关功能 → 说明了是否经过真实从站验证（未验证需明确标注）
- [ ] 普通功能 PR 未修改版本号；发版由维护者推送 `vX.Y.Z` 标签触发
