## 改了什么



## 怎么验证（至少勾前三项）

- [ ] `pnpm lint:tokens` 通过（新增样式无散写色值）
- [ ] `pnpm build` 通过（含 `vue-tsc --noEmit`）
- [ ] `pnpm test` 23/23 通过
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` 通过
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 通过

## 额外说明

- [ ] 移动了 `src/` 下的文件 → 同步改了 `tests/*.test.mjs` 夹具里的源码路径
- [ ] 改了 UI → 附了截图（浅色只截深色主题？本项目只有深色主题，直接截）
- [ ] 改了 Profile 契约或录制格式 → 更新了 `docs/` 对应说明
