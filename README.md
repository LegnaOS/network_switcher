# Network Switcher / 网络配置切换器

A macOS network configuration switcher with GUI. Automatically or manually switch network settings (IP, subnet, gateway, DNS) based on connected WiFi or wired network.

macOS 网络配置切换工具，支持 GUI 界面。根据连接的 WiFi 或有线网络，自动或手动切换网络设置（IP、子网掩码、网关、DNS）。

## Features / 功能特点

- 🔄 **Auto Switch / 自动切换**: Automatically apply saved configurations when connecting to a specific network
- 📶 **WiFi Support / WiFi 支持**: Detect WiFi SSID and router MAC for precise network identification
- 🔌 **Wired Support / 有线支持**: Support Ethernet, Thunderbolt, USB network adapters
- 🔒 **Password Protection / 密码保护**: Protect the app with a startup password
- 💾 **Multiple Configs / 多配置**: Create multiple configurations for the same network
- 🎯 **Manual Apply / 手动应用**: Manually apply any saved configuration

## Requirements / 系统要求

- macOS 10.15+
- Administrator privileges (for changing network settings)
- Rust 1.70+ (for building)

## Installation / 安装

### Build from Source / 从源码编译

```bash
# Clone the repository / 克隆仓库
git clone https://github.com/LegnaOS/network_switcher.git
cd network_switcher

# Build / 编译
cargo build --release

# Run / 运行
./target/release/network_switcher
```

## Usage / 使用方法

### 1. Start the App / 启动程序

```bash
./target/release/network_switcher
```

Enter the password `Legna` to unlock.  
输入密码 `Legna` 解锁。

### 2. Add Configuration / 添加配置

1. Connect to the target network / 连接到目标网络
2. Click **➕ Add** button / 点击 **➕ 添加** 按钮
3. Enter a configuration name (e.g., "Home-Static") / 输入配置名称（如 "家-静态IP"）
4. Choose to bind router MAC for precise matching (optional) / 选择是否绑定路由器 MAC 精确匹配（可选）
5. Click **Get from Current** to copy current settings / 点击 **从当前获取配置** 复制当前设置
6. Edit settings as needed / 根据需要编辑设置
7. Check **🔄 Auto Apply** if you want automatic switching / 勾选 **🔄 自动应用** 以启用自动切换
8. Click **💾 Save** / 点击 **💾 保存**

### 3. Configuration Options / 配置选项

| Option | Description |
|--------|-------------|
| Name / 配置名称 | Custom name for the configuration |
| Match SSID / 匹配 SSID | WiFi SSID to match (leave empty for any) |
| Router MAC | Router MAC address for precise matching |
| Auto Apply / 自动应用 | Automatically apply when network matches |
| Target Service / 目标服务 | Network service to apply settings to |
| Use DHCP | Enable/disable DHCP |
| IP Address | Static IP address |
| Subnet Mask | Subnet mask |
| Router | Default gateway |
| DNS Servers | DNS server addresses |

### 4. Auto Switch / 自动切换

1. Enable **Auto Switch Config** checkbox / 启用 **自动切换配置** 复选框
2. Make sure the configuration has **🔄 Auto Apply** checked / 确保配置勾选了 **🔄 自动应用**
3. The app will automatically apply the matching configuration when network changes / 当网络变化时，程序会自动应用匹配的配置

### 5. Manual Apply / 手动应用

Click the **Apply** button next to any saved configuration to apply it immediately.  
点击任意已保存配置旁边的 **应用** 按钮立即应用。

## Configuration File / 配置文件

Configurations are saved to:  
配置文件保存位置：

```
~/.config/network_switcher/config.json
```

## Screenshots / 截图

The app displays:
- Current network connection (WiFi SSID or wired)
- Router MAC address
- Current IP, subnet, gateway, DNS settings
- List of saved configurations
- Edit panel for configuration details

## License / 许可证

MIT License

## Author / 作者

LegnaOS

