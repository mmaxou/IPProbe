# IPProbe

基于 Rust + Iced(0.13,tiny-skia 软件渲染) 的 Windows 桌面应用,自动探测本机全部网络接口的 IPv4 与 IPv6 地址。

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

### Cloudflare 动态解析(DDNS)

- 自动把本机公网地址写入 Cloudflare DNS 记录:IPv6 → AAAA、IPv4 → A(均可单独开关)
- 域名转发效果:访问 `nas.example.com` 即访问本机(需在路由器/ISP 侧放行对应端口)
- 更新策略参考 ddns-go:仅当记录内容或代理状态变化时才调用 API,避免无效写入
- 使用 Cloudflare API Token(需 `Zone → DNS → Edit` 权限),Token 保存在本地 `settings.json`
- 支持 记录名称自动拼接主域、`@` 表示主域、TTL 选择(1=自动)、橙色云代理开关
- 同步间隔默认 5 分钟,可在设置中调整,并提供“立即同步”按钮

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

技术栈: Iced 0.13(iced::application + tiny-skia/smol)、if-addrs 0.15、tray-icon 0.26、winreg 0.56、Rust 2021。

### Cloudflare Token 创建步骤

1. 登录 [Cloudflare 控制台](https://dash.cloudflare.com) → My Profile → API Tokens → Create Token
2. 选择 “Edit zone DNS” 模板,Zone Resources 选对应域名(或 All zones)
3. 生成 Token 后填入 IPProbe 设置 → Cloudflare 动态解析 → API Token
4. 主域名填根域(如 `example.com`),记录名称填完整记录(如 `nas.example.com` 或子域 `nas`)

---

## 致谢

感谢 [Linux Do](https://linux.do) 社区的交流与支持。

## License

[Apache License 2.0](LICENSE)
