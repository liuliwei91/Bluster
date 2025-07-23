# Bluster博客部署指南

## 概述

本文档描述了如何在生产环境中部署支持Markdown功能的Bluster博客系统。

## 系统要求

### 最低配置
- CPU: 1核心
- 内存: 512MB
- 存储: 1GB可用空间
- 操作系统: Linux/Windows/macOS

### 推荐配置
- CPU: 2核心
- 内存: 1GB
- 存储: 5GB可用空间
- 操作系统: Ubuntu 20.04+ / CentOS 8+ / Windows 10+

### 软件依赖
- Docker 20.10+
- Docker Compose 2.0+
- curl (用于健康检查)

## 快速部署

### Linux/macOS
```bash
# 克隆项目
git clone <repository-url>
cd bluster-blog

# 执行部署
chmod +x deploy.sh
./deploy.sh deploy
```

### Windows
```cmd
# 克隆项目
git clone <repository-url>
cd bluster-blog

# 执行部署
deploy.bat deploy
```

## 环境配置

### 环境变量说明

| 变量名 | 默认值 | 说明 |
|--------|--------|------|
| `RUST_LOG` | info | 日志级别 |
| `DATABASE_URL` | sqlite:///app/data/blog.db | 数据库连接字符串 |
| `MARKDOWN_CACHE_TTL` | 3600 | Markdown缓存过期时间(秒) |
| `MARKDOWN_MAX_CACHE_SIZE` | 1000 | 最大缓存条目数 |
| `MARKDOWN_MAX_CONTENT_SIZE` | 1048576 | 最大内容大小(字节) |
| `MARKDOWN_SYNTAX_THEME` | base16-ocean.dark | 代码高亮主题 |
| `MARKDOWN_ENABLE_TABLES` | true | 启用表格支持 |
| `MARKDOWN_ENABLE_STRIKETHROUGH` | true | 启用删除线支持 |
| `MARKDOWN_ENABLE_TASKLISTS` | true | 启用任务列表支持 |
| `FILE_UPLOAD_MAX_SIZE` | 5242880 | 文件上传最大大小(字节) |

### 自定义配置

1. 复制环境配置文件：
```bash
cp .env.production .env
```

2. 编辑配置文件：
```bash
nano .env
```

3. 重新部署：
```bash
./deploy.sh restart
```

## Markdown功能验证

部署完成后，系统会自动验证以下功能：

### 基础功能
- [x] 主页访问
- [x] 管理后台访问
- [x] API端点响应

### Markdown功能
- [x] 文章Markdown渲染
- [x] 代码语法高亮
- [x] 表格支持
- [x] 链接和图片支持
- [x] 文件导入导出
- [x] 实时预览

### 手动验证步骤

1. **访问主页**: http://localhost:8080
2. **登录管理后台**: http://localhost:8080/admin
   - 用户名: admin
   - 密码: admin123
3. **创建Markdown文章**:
   - 点击"新建文章"
   - 输入Markdown内容
   - 使用预览功能
   - 保存并查看渲染效果
4. **测试文件导入**:
   - 准备.md文件
   - 使用导入功能
   - 验证内容正确性
5. **测试文件导出**:
   - 选择现有文章
   - 点击导出按钮
   - 验证导出文件格式

## 监控和维护

### 健康检查
```bash
# 检查服务状态
./deploy.sh status

# 查看日志
./deploy.sh logs

# 重启服务
./deploy.sh restart
```

### 数据备份
```bash
# 手动备份
./deploy.sh backup

# 自动备份(建议设置cron任务)
0 2 * * * /path/to/bluster-blog/deploy.sh backup
```

### 性能监控

监控以下指标：
- CPU使用率
- 内存使用率
- 磁盘空间
- 响应时间
- 错误率

### 日志管理

日志位置：
- 应用日志: `docker-compose logs bluster`
- 系统日志: `/var/log/docker/`

## 故障排除

### 常见问题

1. **服务启动失败**
   ```bash
   # 检查端口占用
   netstat -tlnp | grep 8080
   
   # 检查Docker状态
   docker ps -a
   
   # 查看详细日志
   docker-compose logs bluster
   ```

2. **Markdown渲染异常**
   ```bash
   # 检查环境变量
   docker exec bluster-blog env | grep MARKDOWN
   
   # 重启服务
   ./deploy.sh restart
   ```

3. **文件上传失败**
   ```bash
   # 检查文件大小限制
   docker exec bluster-blog env | grep FILE_UPLOAD
   
   # 检查磁盘空间
   df -h
   ```

4. **数据库连接问题**
   ```bash
   # 检查数据库文件
   ls -la data/
   
   # 检查权限
   docker exec bluster-blog ls -la /app/data/
   ```

### 性能优化

1. **Markdown缓存优化**
   ```bash
   # 增加缓存大小
   export MARKDOWN_MAX_CACHE_SIZE=2000
   export MARKDOWN_CACHE_TTL=7200
   ```

2. **文件上传优化**
   ```bash
   # 调整上传限制
   export FILE_UPLOAD_MAX_SIZE=10485760
   ```

3. **数据库优化**
   ```bash
   # 定期清理和优化
   docker exec bluster-blog sqlite3 /app/data/blog.db "VACUUM;"
   ```

## 安全建议

1. **修改默认密码**
   - 首次登录后立即修改admin密码
   - 设置强密码策略

2. **网络安全**
   - 使用反向代理(Nginx/Apache)
   - 启用HTTPS
   - 配置防火墙规则

3. **文件安全**
   - 限制文件上传类型
   - 扫描上传文件
   - 定期备份数据

4. **系统安全**
   - 定期更新系统和Docker镜像
   - 监控异常访问
   - 配置日志审计

## 升级指南

1. **备份数据**
   ```bash
   ./deploy.sh backup
   ```

2. **拉取最新代码**
   ```bash
   git pull origin main
   ```

3. **重新构建和部署**
   ```bash
   ./deploy.sh deploy
   ```

4. **验证功能**
   ```bash
   ./deploy.sh status
   ```

## 支持和联系

如遇到问题，请：
1. 查看本文档的故障排除部分
2. 检查项目Issues页面
3. 提交新的Issue并附上详细日志

---

**注意**: 本部署指南适用于支持Markdown功能的Bluster博客系统。请确保在生产环境中遵循安全最佳实践。