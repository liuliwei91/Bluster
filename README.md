# Bluster 博客系统

一个基于 Rust 和 Actix-web 构建的现代化博客系统，支持文章管理、用户认证、密码重置等功能。

## 功能特性

- 📝 文章管理（创建、编辑、删除）
- 👤 用户认证和会话管理
- 🔐 安全的密码重置功能
- 🛡️ 安全问题验证
- 📱 响应式设计
- 🐳 Docker 容器化部署
- 🗄️ SQLite 数据库

## 快速开始

### 方式一：Docker 部署（推荐）

#### 前置要求

- Docker
- Docker Compose

#### 一键部署

**Linux/macOS:**
```bash
# 给脚本执行权限
chmod +x deploy.sh

# 完整部署
./deploy.sh deploy
```

**Windows:**
```cmd
# 完整部署
deploy.bat deploy
```

#### 手动部署

1. 克隆项目
```bash
git clone <repository-url>
cd Bluster
```

2. 构建并启动
```bash
docker-compose up -d --build
```

3. 访问应用
- 博客首页: http://localhost:8080
- 管理后台: http://localhost:8080/admin

### 方式二：本地开发

#### 前置要求

- Rust 1.75+
- SQLite3

#### 安装步骤

1. 安装依赖
```bash
cargo build
```

2. 运行应用
```bash
cargo run
```

## 部署脚本使用

### Linux/macOS (deploy.sh)

```bash
# 完整部署
./deploy.sh deploy

# 仅构建镜像
./deploy.sh build

# 启动服务
./deploy.sh start

# 停止服务
./deploy.sh stop

# 重启服务
./deploy.sh restart

# 查看状态
./deploy.sh status

# 查看日志
./deploy.sh logs

# 备份数据
./deploy.sh backup

# 清理资源
./deploy.sh cleanup

# 显示帮助
./deploy.sh help
```

### Windows (deploy.bat)

```cmd
REM 完整部署
deploy.bat deploy

REM 仅构建镜像
deploy.bat build

REM 启动服务
deploy.bat start

REM 停止服务
deploy.bat stop

REM 重启服务
deploy.bat restart

REM 查看状态
deploy.bat status

REM 查看日志
deploy.bat logs

REM 备份数据
deploy.bat backup

REM 清理资源
deploy.bat cleanup

REM 显示帮助
deploy.bat help
```

## 默认账户

- **用户名**: admin
- **密码**: admin

⚠️ **重要**: 首次登录后请立即修改默认密码！

## 目录结构

```
Bluster/
├── src/
│   ├── main.rs          # 主应用程序
│   └── models.rs        # 数据模型
├── templates/           # HTML 模板
│   ├── admin/          # 管理后台模板
│   ├── base.html       # 基础模板
│   ├── index.html      # 首页模板
│   └── ...
├── data/               # 数据库文件目录
├── Dockerfile          # Docker 镜像构建文件
├── docker-compose.yml  # Docker Compose 配置
├── deploy.sh          # Linux/macOS 部署脚本
├── deploy.bat         # Windows 部署脚本
└── README.md          # 项目说明
```

## 配置说明

### 环境变量

- `RUST_LOG`: 日志级别 (默认: info)
- `DATABASE_URL`: 数据库连接字符串 (默认: sqlite:///app/data/blog.db)

### 端口配置

- 应用端口: 8080
- 可通过修改 `docker-compose.yml` 中的端口映射来更改

## 数据持久化

数据库文件存储在 `./data` 目录中，该目录会被挂载到 Docker 容器中，确保数据持久化。

## 备份与恢复

### 备份

```bash
# 使用部署脚本备份
./deploy.sh backup

# 或手动备份
cp -r ./data ./backup-$(date +%Y%m%d)
```

### 恢复

```bash
# 停止服务
./deploy.sh stop

# 恢复数据
cp -r ./backup-20240101/* ./data/

# 启动服务
./deploy.sh start
```

## 故障排除

### 常见问题

1. **端口被占用**
   - 修改 `docker-compose.yml` 中的端口映射
   - 或停止占用 8080 端口的其他服务

2. **权限问题**
   - 确保部署脚本有执行权限: `chmod +x deploy.sh`
   - 确保 Docker 有足够权限

3. **数据库初始化失败**
   - 检查 `./data` 目录权限
   - 查看容器日志: `./deploy.sh logs`

### 查看日志

```bash
# 实时查看日志
./deploy.sh logs

# 或使用 Docker 命令
docker-compose logs -f
```

## 开发指南

### 本地开发环境

1. 安装 Rust
2. 克隆项目
3. 运行 `cargo run`
4. 访问 http://localhost:8080

### 代码结构

- `src/main.rs`: 主要的路由和处理函数
- `src/models.rs`: 数据库模型和操作
- `templates/`: Tera 模板文件

## 安全注意事项

1. 修改默认管理员密码
2. 在生产环境中使用 HTTPS
3. 定期备份数据
4. 保持系统更新

## 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

## 贡献

欢迎提交 Issue 和 Pull Request！

## 支持

如果遇到问题，请：

1. 查看本文档的故障排除部分
2. 检查 GitHub Issues
3. 提交新的 Issue