use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use crate::config::{AppConfig, ConfigType, NetworkConfig};
use crate::network;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::thread;

/// 后台网络状态
#[derive(Clone, Default)]
struct NetworkState {
    ssid: Option<String>,
    router_mac: Option<String>,
    config: Option<NetworkConfig>,
    is_loading: bool,
}

pub struct NetworkSwitcherApp {
    config: AppConfig,
    current_ssid: Option<String>,
    current_router_mac: Option<String>,
    current_network_config: Option<NetworkConfig>,
    network_services: Vec<String>,
    selected_service_idx: usize,

    // 编辑状态
    editing_config: Option<NetworkConfig>,
    new_dns_input: String,
    status_message: String,
    show_add_dialog: bool,
    new_config_name: String,
    new_ssid_input: String,
    bind_router_mac: bool,

    // 添加对话框状态
    add_config_type: ConfigType,
    add_service_idx: usize,

    // 自动检测
    last_check: Instant,
    last_applied_key: Option<String>,

    // 后台刷新状态
    bg_state: Arc<Mutex<NetworkState>>,
    is_refreshing: bool,

    // 密码验证
    is_authenticated: bool,
    password_input: String,
    password_error: bool,
}

impl Default for NetworkSwitcherApp {
    fn default() -> Self {
        let config = AppConfig::load();
        let services = network::get_network_services();
        let selected_idx = services
            .iter()
            .position(|s| s == &config.network_service)
            .unwrap_or(0);

        let current_config = if !services.is_empty() {
            Some(network::get_current_config(&services[selected_idx]))
        } else {
            None
        };

        Self {
            config,
            current_ssid: None,
            current_router_mac: None,
            current_network_config: current_config,
            network_services: services,
            selected_service_idx: selected_idx,
            editing_config: None,
            new_dns_input: String::new(),
            status_message: String::new(),
            show_add_dialog: false,
            new_config_name: String::new(),
            new_ssid_input: String::new(),
            bind_router_mac: true,
            add_config_type: ConfigType::Wifi,
            add_service_idx: selected_idx,
            last_check: Instant::now() - std::time::Duration::from_secs(10),
            last_applied_key: None,
            bg_state: Arc::new(Mutex::new(NetworkState::default())),
            is_refreshing: false,
            is_authenticated: false,
            password_input: String::new(),
            password_error: false,
        }
    }
}

impl NetworkSwitcherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 加载中文字体
        Self::setup_fonts(&cc.egui_ctx);
        Self::default()
    }

    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();

        // 尝试加载系统中文字体
        let font_paths = [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ];

        let mut font_loaded = false;
        for path in font_paths {
            if let Ok(font_data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "chinese".to_owned(),
                    FontData::from_owned(font_data).into(),
                );

                // 将中文字体添加到首选字体列表
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, "chinese".to_owned());

                fonts
                    .families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .insert(0, "chinese".to_owned());

                font_loaded = true;
                break;
            }
        }

        if font_loaded {
            ctx.set_fonts(fonts);
        }
    }

    /// 在后台线程刷新网络状态
    fn refresh_in_background(&mut self, service: String) {
        if self.is_refreshing {
            return;
        }
        self.is_refreshing = true;

        let bg_state = Arc::clone(&self.bg_state);

        // 先标记正在加载
        if let Ok(mut state) = bg_state.lock() {
            state.is_loading = true;
        }

        thread::spawn(move || {
            // 获取网络标识信息
            let identity = network::get_network_identity();
            let config = network::get_current_config(&service);

            if let Ok(mut state) = bg_state.lock() {
                state.ssid = if identity.is_wired {
                    identity.service_name.map(|s| format!("[有线] {}", s))
                } else {
                    identity.ssid
                };
                state.router_mac = identity.router_mac;
                state.config = Some(config);
                state.is_loading = false;
            }
        });
    }

    /// 检查后台刷新结果并应用
    fn check_bg_state(&mut self) -> bool {
        let mut network_changed = false;
        if let Ok(state) = self.bg_state.lock() {
            if !state.is_loading && self.is_refreshing {
                // 检测网络是否变化（SSID 或 MAC）
                if self.current_ssid != state.ssid || self.current_router_mac != state.router_mac {
                    network_changed = true;
                }
                self.current_ssid = state.ssid.clone();
                self.current_router_mac = state.router_mac.clone();
                self.current_network_config = state.config.clone();
                self.is_refreshing = false;
            }
        }
        network_changed
    }

    /// 当网络变化时自动应用配置
    fn try_auto_apply(&mut self) {
        if !self.config.auto_switch {
            return;
        }

        // 获取当前网络信息
        let ssid = match &self.current_ssid {
            Some(s) => s.clone(),
            None => return,
        };
        let router_mac = self.current_router_mac.as_deref();

        // 查找自动应用的配置
        if let Some(cfg) = self.config.find_auto_apply_config(&ssid, router_mac).cloned() {
            let key = cfg.config_key();
            // 如果已经应用过相同配置，跳过
            if self.last_applied_key.as_ref() == Some(&key) {
                return;
            }
            self.apply_config_internal(&cfg);
        } else {
            // 没有匹配的自动配置，清除上次应用记录
            self.last_applied_key = None;
        }
    }

    /// 内部应用配置
    fn apply_config_internal(&mut self, cfg: &NetworkConfig) {
        let target_service = cfg.target_service
            .as_ref()
            .unwrap_or(&self.network_services[self.selected_service_idx])
            .clone();

        match network::apply_config(&target_service, cfg) {
            Ok(_) => {
                self.status_message = format!(
                    "✅ 已应用配置: {} -> {}",
                    cfg.name, target_service
                );
                self.last_applied_key = Some(cfg.config_key());
                // 刷新当前配置显示
                self.refresh_in_background(target_service);
            }
            Err(e) => {
                self.status_message = format!("❌ 应用失败: {}", e);
            }
        }
    }

    /// 检查网络变化并自动应用配置
    fn check_and_auto_apply(&mut self, ctx: &egui::Context) {
        use std::time::Duration;

        // 检查后台状态更新，如果 SSID 变化则立即尝试应用配置
        let ssid_changed = self.check_bg_state();
        if ssid_changed {
            self.try_auto_apply();
        }

        // 每5秒检查一次
        if self.last_check.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.last_check = Instant::now();

        // 在后台线程更新网络信息
        let service = self.network_services[self.selected_service_idx].clone();
        self.refresh_in_background(service);

        // 请求重绘以更新状态
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// 渲染密码输入界面
    fn render_password_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("🔐 Network Switcher");
                ui.add_space(20.0);
                ui.label("请输入密码 / Enter Password");
                ui.add_space(10.0);

                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.password_input)
                        .password(true)
                        .hint_text("密码 / Password")
                        .desired_width(200.0)
                );

                // 回车提交
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.verify_password();
                }

                ui.add_space(10.0);

                if ui.button("🔓 解锁 / Unlock").clicked() {
                    self.verify_password();
                }

                if self.password_error {
                    ui.add_space(10.0);
                    ui.colored_label(egui::Color32::RED, "❌ 密码错误 / Wrong Password");
                }
            });
        });
    }

    /// 验证密码
    fn verify_password(&mut self) {
        const PASSWORD: &str = "Legna";
        if self.password_input == PASSWORD {
            self.is_authenticated = true;
            self.password_error = false;
            // 密码验证成功后立即刷新网络状态
            let service = self.network_services[self.selected_service_idx].clone();
            self.refresh_in_background(service);
        } else {
            self.password_error = true;
            self.password_input.clear();
        }
    }
}

impl eframe::App for NetworkSwitcherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 如果未验证密码，显示密码输入界面
        if !self.is_authenticated {
            self.render_password_screen(ctx);
            return;
        }

        // 自动检查和应用网络配置（后台执行）
        self.check_and_auto_apply(ctx);

        // 请求持续刷新以支持自动检测
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🌐 网络配置切换器");
            ui.add_space(10.0);

            // 当前网络状态
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("📡 当前状态");
                    if self.is_refreshing {
                        ui.spinner();
                    }
                    if ui.button("🔄 刷新").clicked() && !self.is_refreshing {
                        let service = self.network_services[self.selected_service_idx].clone();
                        self.refresh_in_background(service);
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("网络连接 / Network:");
                    let network_display = self.current_ssid.as_deref().unwrap_or("加载中... / Loading...");
                    if network_display.starts_with("[有线]") {
                        ui.strong(format!("🔌 {}", network_display));
                    } else {
                        ui.strong(format!("📶 {}", network_display));
                    }
                });

                // 显示路由器 MAC（用于唯一标识）
                if let Some(ref mac) = self.current_router_mac {
                    ui.horizontal(|ui| {
                        ui.label("路由器 MAC:");
                        ui.strong(mac);
                    });
                }

                let mut service_changed: Option<String> = None;
                ui.horizontal(|ui| {
                    ui.label("网络服务 / Service:");
                    egui::ComboBox::from_id_salt("service_select")
                        .selected_text(&self.network_services[self.selected_service_idx])
                        .show_ui(ui, |ui| {
                            for (i, service) in self.network_services.iter().enumerate() {
                                if ui.selectable_value(&mut self.selected_service_idx, i, service).clicked() {
                                    service_changed = Some(service.clone());
                                }
                            }
                        });
                });
                if let Some(service) = service_changed {
                    self.config.network_service = service.clone();
                    self.refresh_in_background(service);
                    let _ = self.config.save();
                }

                // 显示当前配置信息
                if let Some(ref cfg) = self.current_network_config {
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("IP:");
                        ui.strong(cfg.ip_address.as_deref().unwrap_or("N/A"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("子网掩码 / Subnet:");
                        ui.strong(cfg.subnet_mask.as_deref().unwrap_or("N/A"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("路由器 / Router:");
                        ui.strong(cfg.router.as_deref().unwrap_or("N/A"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("DNS:");
                        if cfg.dns_servers.is_empty() {
                            ui.strong("自动 / Auto");
                        } else {
                            ui.strong(cfg.dns_servers.join(", "));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("模式 / Mode:");
                        ui.strong(if cfg.use_dhcp { "DHCP" } else { "静态 / Static" });
                    });
                }
            });
            
            ui.add_space(10.0);
            
            // 自动切换开关
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.config.auto_switch, "自动切换配置").changed() {
                    let _ = self.config.save();
                }
            });
            
            ui.add_space(10.0);
            self.render_config_list(ui);
            ui.add_space(10.0);
            self.render_edit_panel(ui);
            
            // 状态消息
            if !self.status_message.is_empty() {
                ui.add_space(10.0);
                ui.colored_label(egui::Color32::from_rgb(100, 200, 100), &self.status_message);
            }
        });
        
        self.render_add_dialog(ctx);
    }
}

impl NetworkSwitcherApp {
    fn render_config_list(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("已保存的配置");
                if ui.button("➕ 添加").clicked() {
                    self.show_add_dialog = true;
                    self.new_config_name.clear();
                    self.new_ssid_input = self.current_ssid.clone().unwrap_or_default();
                    self.bind_router_mac = true;
                }
            });

            ui.separator();

            // 按名称排序显示
            let mut configs: Vec<_> = self.config.configs.values().cloned().collect();
            configs.sort_by(|a, b| a.name.cmp(&b.name));

            let current_ssid = self.current_ssid.clone();
            let current_mac = self.current_router_mac.clone();

            for cfg in configs {
                let target = cfg.target_service.as_deref().unwrap_or("Wi-Fi");

                // 检查是否匹配当前网络
                let is_matching = cfg.matches_network(
                    current_ssid.as_deref().unwrap_or(""),
                    current_mac.as_deref()
                );

                ui.horizontal(|ui| {
                    // 显示配置名称和信息
                    let display = cfg.display_name();

                    if is_matching {
                        ui.strong(format!("● {}", display));
                    } else {
                        ui.label(format!("  {}", display));
                    }

                    ui.label(format!("→ {}", target));

                    if ui.button("编辑").clicked() {
                        self.editing_config = Some(cfg.clone());
                    }

                    if ui.button("应用").clicked() {
                        self.apply_config_internal(&cfg);
                    }

                    let key = cfg.config_key();
                    if ui.button("🗑").clicked() {
                        self.config.remove_config(&key);
                        let _ = self.config.save();
                    }
                });
            }

            if self.config.configs.is_empty() {
                ui.label("暂无保存的配置，点击「添加」创建新配置");
            }
        });
    }

    fn render_edit_panel(&mut self, ui: &mut egui::Ui) {
        let mut should_save = false;
        let mut should_cancel = false;
        let mut dns_to_remove: Option<usize> = None;
        let mut dns_to_add: Option<String> = None;

        let services_clone = self.network_services.clone();

        if let Some(ref mut editing) = self.editing_config {
            ui.group(|ui| {
                ui.label("📝 编辑配置");
                ui.separator();

                // 配置名称
                ui.horizontal(|ui| {
                    ui.label("配置名称 / Name:");
                    ui.text_edit_singleline(&mut editing.name);
                });

                // 匹配的 SSID
                ui.horizontal(|ui| {
                    ui.label("匹配 SSID:");
                    ui.text_edit_singleline(&mut editing.ssid);
                    ui.label("(留空表示不限)");
                });

                // 路由器 MAC
                ui.horizontal(|ui| {
                    ui.label("路由器 MAC:");
                    let mut mac = editing.router_mac.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut mac).changed() {
                        editing.router_mac = if mac.is_empty() { None } else { Some(mac) };
                    }
                    ui.label("(留空表示不限)");
                });

                // 自动应用开关
                ui.checkbox(&mut editing.auto_apply, "🔄 自动应用 (连接此网络时自动使用此配置)");

                ui.add_space(5.0);

                // 目标网络服务选择
                ui.horizontal(|ui| {
                    ui.label("目标服务 / Target:");
                    let current_target = editing.target_service
                        .clone()
                        .unwrap_or_else(|| "Wi-Fi".to_string());
                    egui::ComboBox::from_id_salt("target_service_edit")
                        .selected_text(&current_target)
                        .show_ui(ui, |ui| {
                            for service in &services_clone {
                                if ui.selectable_label(
                                    editing.target_service.as_ref() == Some(service),
                                    service
                                ).clicked() {
                                    editing.target_service = Some(service.clone());
                                }
                            }
                        });
                });

                ui.add_space(5.0);
                ui.checkbox(&mut editing.use_dhcp, "使用 DHCP / Use DHCP");

                if !editing.use_dhcp {
                    ui.horizontal(|ui| {
                        ui.label("IP 地址 / IP:");
                        let mut ip = editing.ip_address.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut ip).changed() {
                            editing.ip_address = Some(ip);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("子网掩码 / Subnet:");
                        let mut mask = editing.subnet_mask.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut mask).changed() {
                            editing.subnet_mask = Some(mask);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("路由器 / Router:");
                        let mut router = editing.router.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut router).changed() {
                            editing.router = Some(router);
                        }
                    });
                }

                ui.add_space(5.0);
                ui.label("DNS 服务器 / DNS Servers:");

                for (i, dns) in editing.dns_servers.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(dns);
                        if ui.button("❌").clicked() {
                            dns_to_remove = Some(i);
                        }
                    });
                }

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_dns_input);
                    if ui.button("添加 DNS").clicked() && !self.new_dns_input.is_empty() {
                        dns_to_add = Some(self.new_dns_input.clone());
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("💾 保存").clicked() {
                        should_save = true;
                    }
                    if ui.button("取消").clicked() {
                        should_cancel = true;
                    }
                });
            });
        }

        // 处理延迟的操作
        if let Some(idx) = dns_to_remove {
            if let Some(ref mut editing) = self.editing_config {
                editing.dns_servers.remove(idx);
            }
        }

        if let Some(dns) = dns_to_add {
            if let Some(ref mut editing) = self.editing_config {
                editing.dns_servers.push(dns);
            }
            self.new_dns_input.clear();
        }

        if should_save {
            if let Some(editing) = self.editing_config.take() {
                self.config.add_config(editing);
                let _ = self.config.save();
                self.status_message = "配置已保存".to_string();
            }
        }

        if should_cancel {
            self.editing_config = None;
        }
    }

    fn render_add_dialog(&mut self, ctx: &egui::Context) {
        if self.show_add_dialog {
            egui::Window::new("添加新配置 / Add Config")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    // 配置名称
                    ui.horizontal(|ui| {
                        ui.label("配置名称 / Name:");
                        ui.text_edit_singleline(&mut self.new_config_name);
                    });

                    ui.add_space(5.0);

                    // 配置类型选择
                    ui.horizontal(|ui| {
                        ui.label("类型 / Type:");
                        ui.radio_value(&mut self.add_config_type, ConfigType::Wifi, "📶 WiFi");
                        ui.radio_value(&mut self.add_config_type, ConfigType::Service, "🔌 有线/服务");
                    });

                    ui.add_space(5.0);

                    // 匹配的 SSID
                    ui.horizontal(|ui| {
                        ui.label("匹配 SSID:");
                        ui.text_edit_singleline(&mut self.new_ssid_input);
                    });

                    // 绑定路由器 MAC
                    ui.checkbox(&mut self.bind_router_mac, "🔒 绑定路由器 MAC（精确匹配网络）");
                    if self.bind_router_mac {
                        if let Some(ref mac) = self.current_router_mac {
                            ui.label(format!("   当前 MAC: {}", mac));
                        }
                    }

                    // 目标服务选择
                    ui.horizontal(|ui| {
                        ui.label("应用到服务:");
                        egui::ComboBox::from_id_salt("add_service_select")
                            .selected_text(&self.network_services[self.add_service_idx])
                            .show_ui(ui, |ui| {
                                for (i, service) in self.network_services.iter().enumerate() {
                                    ui.selectable_value(&mut self.add_service_idx, i, service);
                                }
                            });
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let can_add = !self.new_config_name.is_empty();

                        if ui.button("从当前获取配置").clicked() && can_add {
                            let service = self.network_services[self.add_service_idx].clone();
                            let router_mac = if self.bind_router_mac {
                                self.current_router_mac.clone()
                            } else {
                                None
                            };
                            let mut cfg = network::get_current_config(&service);
                            cfg.name = self.new_config_name.clone();
                            cfg.ssid = self.new_ssid_input.clone();
                            cfg.router_mac = router_mac;
                            cfg.config_type = self.add_config_type.clone();
                            cfg.target_service = Some(service);
                            cfg.auto_apply = false;
                            self.editing_config = Some(cfg);
                            self.show_add_dialog = false;
                        }

                        if ui.button("创建空白配置").clicked() && can_add {
                            let service = self.network_services[self.add_service_idx].clone();
                            let router_mac = if self.bind_router_mac {
                                self.current_router_mac.clone()
                            } else {
                                None
                            };
                            let cfg = NetworkConfig::new(
                                self.new_config_name.clone(),
                                self.new_ssid_input.clone(),
                                Some(service),
                                self.add_config_type.clone(),
                                router_mac
                            );
                            self.editing_config = Some(cfg);
                            self.show_add_dialog = false;
                        }

                        if ui.button("取消").clicked() {
                            self.show_add_dialog = false;
                        }
                    });

                    if self.new_config_name.is_empty() {
                        ui.colored_label(egui::Color32::RED, "⚠️ 请输入配置名称");
                    }
                });
        }
    }
}

