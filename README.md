# IPProbe

基于 Rust + egui(eframe) 的 Windows 桌面应用,自动探测本机全部网络接口的 IPv4 与 IPv6 地址。

## 功能(MVP)

- 启动即扫描:枚举所有网络接口,展示接口名、索引、启停状态及其 IPv4/IPv6 地址
- 地址分类:回环 / 链路本地 / 唯一本地 / 私网 / 公网
- 默认出口地址:UDP 探测本机对外通信首选的 IPv4 / IPv6 地址
- 自动刷新(默认 5s,可切换 3/5/10/30s)与手动刷新
- 状态栏:接口数、IPv4/IPv6 地址数、上次扫描时间

## 构建与运行

```bash
# 开发(WSL / Linux)
cargo run

# Windows 原生
cargo run --release
# 产物: target/release/IPProbe.exe
```

技术栈: eframe 0.37(egui)、if-addrs 0.15、Rust 2021。
