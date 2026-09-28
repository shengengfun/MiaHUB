# MiaKeyDrv

> 一个开源、无广告、好看的 AULA（狼蛛）机械键盘管理器。
> 替代官方那个丑丑的 ShinetekTools，并补上官方没有的功能。

![界面风格：深色 / G HUB 风格](https://img.shields.io/badge/UI-G%20HUB%20inspired-0a84ff)
![协议](https://img.shields.io/badge/protocol-HID%20feature%20report-00c8ff)
![许可](https://img.shields.io/badge/license-MIT-green)

## 为什么做这个

官方的 F3009 驱动（`ShinetekTools.exe`）功能少、界面丑，而且**连灯光休眠时间这类基础选项都没有**。
本项目直接和键盘的 HID 协议通信，不依赖官方驱动，提供更现代的管理体验。

协议是怎么抠出来的、踩过哪些坑，见姊妹项目 **[OpenALUA](https://github.com/shengengfun/OpenALUA)**（逆向与工具链）。

## 支持的设备

| 型号 | 连接方式 | 状态 |
| --- | --- | --- |
| AULA **F3009** | USB 有线（`1A2C:7F05` / `1A2C:7F07`） | ✅ 已支持 |
| AULA F3009 | 2.4G 接收器（`1A2C:7FFF`） | ⛔ 待逆向（无 feature report） |
| AULA **F2087Pro** | USB（`0C45:800A`）/ 2.4G（`05AC:024F`） | ⛔ 待逆向（不同 MCU） |

> 型号识别基于 VID/PID 白名单，见 `src-tauri/src/protocol.rs`。

## 功能

**灯效**

- **20 个主灯效果**，序号与官方固件一一对应：
  - `0` 常亮、`2` 呼吸、`3` 随按随灭 … `19` 正弦光波
  - `1` = **指点江山**（Gaming Special Key）—— 官方 UI 隐藏了这一项，MiaKeyDrv 把它放了出来
- **亮度** 0–5（0 即关灯，界面上一键切换）
- **速度** 0–2，每条灯效都能调 —— 官方是按灯效分页决定要不要显示这个滑块，
  这里不做限制；预览动画周期会跟着速度实时变化
- 键盘预览图按真实 87 键坐标绘制，效果动画同步预览

**电源与性能**（官方完全没有的部分）

- **空闲熄灯**：停止操作 N 分钟后自动关灯，按下任意键立即恢复
- **保持唤醒**：定期给键盘发一帧，避免休眠后吞掉第一个按键
- **回报率实测**：直接读键盘输入管道，实测真实的报告率（Hz）与中位间隔

**其他**

- 设备自动识别与自动连接，插拔后自动重扫
- 灯效列表支持中英文搜索
- 设备信息（VID:PID / 序列号 / 设备应答）只读查询

**计划中**

- 按键重映射与宏录制 —— 写入格式已逆向完成（宏是 XML，经 output report 下发），缺 UI
- 配置档位（4 组 profile）与按应用自动切换

## 协议

一句话版本（8 字节 HID feature report，report id = 7）：

```text
07 FF FF <效果序号 0-19> <亮度 0-5> <速度 0-2> <保留> <保留>
```

完整逆向说明见 [OpenALUA 的 PROTOCOL.md](https://github.com/shengengfun/OpenALUA/blob/main/docs/PROTOCOL.md)，
其中包含官方软件的初始化序列、读写路径、宏/改键 XML 格式、以及实测复现步骤。

## 构建

前置：Rust ≥ 1.77、Node 18+、Windows 10/11。

```bash
npm install
npm run dev      # 开发运行
npm run build    # 打包（NSIS 安装包）
```

Rust 侧单独跑测试（含与官方抓包逐字节对照的断言）：

```bash
cd src-tauri
cargo test
```

## 项目结构

```
src/                    前端（原生 HTML/CSS/JS，G HUB 风格深色界面）
  index.html
  styles.css
  main.js               灯效 / 预览 / 电源与性能逻辑
  keyboard-layout.js    87 键坐标表（由官方 positions.xml 生成）
src-tauri/              Rust 后端
  src/protocol.rs       协议定义：效果表、帧构造（含与抓包的对照测试）
  src/hid.rs            HID 枚举、feature report 收发（100ms 节流）、回报率实测
  src/lib.rs            Tauri 命令与状态
```

## 安全性

- 不修改系统、不安装驱动、不常驻后台服务
- 只通过 HID feature report 与键盘通信，写入间隔 ≥ 100 ms
  （官方驱动同样有此限制，过快会导致固件卡顿）
- 逆向过程全部基于自有设备的被动抓包与官方软件行为观察，不包含任何厂商二进制

## 致谢与贡献者

- 上游 / 灵感：AULA 官方 ShinetekTools（协议行为参考，仅用于互操作性研究）
- [shengengfun](https://github.com/shengengfun) — 项目发起与硬件实测
- **DeepSeek** — AI 协作（逆向分析、协议推理、代码实现）
- **GitHub Copilot** — AI 协作

## 许可

MIT。与东莞市索艾电子科技有限公司无关联，本项目为独立开发。
