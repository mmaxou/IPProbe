# IPProbe

基于 Rust + egui(eframe) 的 Windows 桌面应用,自动探测本机全部网络接口的 IPv4 与 IPv6 地址。

## 功能

- 启动即扫描:枚举所有网络接口,展示接口名、索引、启停状态及其 IPv4/IPv6 地址
- 地址分类:回环 / 链路本地 / 唯一本地 / 私网 / 公网
- 默认出口地址:UDP 探测本机对外通信首选的 IPv4 / IPv6 地址
- 公网出口 IPv4:外部 HTTP 查询(60s 缓存)
- 自动刷新(默认 5s,可切换 3/5/10/30s)与手动刷新
- 状态栏:接口数、IPv4/IPv6 地址数、上次扫描时间

### 设置(设置窗口)

- 开机自启动:写入当前用户注册表 Run 键(HKCU)
- 后台静默探测:窗口隐藏/最小化时仍按间隔刷新数据
- 关闭时缩小到托盘:关闭窗口不退出,保留在系统托盘
- 最小化时隐藏到托盘:最小化后隐藏窗口,仅保留托盘图标
- 设置持久化到 `%USERPROFILE%\.ipprobe\settings.json`

### 系统托盘

- 托盘图标 + 右键菜单(显示主窗口 / 退出)
- 左键单击托盘图标显示主窗口

## 构建与运行

```bash
# 开发(WSL / Linux)
cargo run

# Windows 原生
cargo run --release
# 产物: target/release/IPProbe.exe
```

技术栈: eframe 0.37(egui)、if-addrs 0.15、tray-icon 0.26、winreg 0.56、Rust 2021。
