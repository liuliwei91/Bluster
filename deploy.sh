#!/bin/bash

# Bluster博客Docker部署脚本
# 作者: Assistant
# 版本: 1.0

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查Docker是否安装
check_docker() {
    if ! command -v docker &> /dev/null; then
        log_error "Docker未安装，请先安装Docker"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose未安装，请先安装Docker Compose"
        exit 1
    fi
    
    log_success "Docker环境检查通过"
}

# 创建数据目录
create_data_dir() {
    if [ ! -d "./data" ]; then
        mkdir -p ./data
        log_info "创建数据目录: ./data"
    fi
}

# 构建镜像
build_image() {
    log_info "开始构建Docker镜像..."
    docker-compose build
    log_success "Docker镜像构建完成"
}

# 启动服务
start_service() {
    log_info "启动Bluster博客服务..."
    docker-compose up -d
    log_success "服务启动成功"
}

# 停止服务
stop_service() {
    log_info "停止Bluster博客服务..."
    docker-compose down
    log_success "服务已停止"
}

# 重启服务
restart_service() {
    log_info "重启Bluster博客服务..."
    docker-compose restart
    log_success "服务重启完成"
}

# 查看日志
view_logs() {
    log_info "查看服务日志..."
    docker-compose logs -f
}

# 查看状态
check_status() {
    log_info "检查服务状态..."
    docker-compose ps
    echo ""
    log_info "健康检查状态:"
    docker inspect bluster-blog --format='{{.State.Health.Status}}' 2>/dev/null || echo "健康检查未配置"
}

# 清理资源
cleanup() {
    log_warning "这将删除所有容器、镜像和数据，确定要继续吗？ (y/N)"
    read -r response
    if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        log_info "清理Docker资源..."
        docker-compose down -v --rmi all
        docker system prune -f
        log_success "清理完成"
    else
        log_info "取消清理操作"
    fi
}

# 备份数据
backup_data() {
    local backup_dir="./backups/$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$backup_dir"
    
    if [ -d "./data" ]; then
        log_info "备份数据到: $backup_dir"
        cp -r ./data/* "$backup_dir/"
        log_success "数据备份完成"
    else
        log_warning "数据目录不存在，跳过备份"
    fi
}

# 显示帮助信息
show_help() {
    echo "Bluster博客Docker部署脚本"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  deploy    - 完整部署（构建+启动）"
    echo "  build     - 仅构建镜像"
    echo "  start     - 启动服务"
    echo "  stop      - 停止服务"
    echo "  restart   - 重启服务"
    echo "  status    - 查看服务状态"
    echo "  logs      - 查看服务日志"
    echo "  backup    - 备份数据"
    echo "  cleanup   - 清理所有资源"
    echo "  help      - 显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0 deploy   # 完整部署"
    echo "  $0 logs     # 查看日志"
}

# 完整部署
full_deploy() {
    log_info "开始完整部署Bluster博客..."
    check_docker
    create_data_dir
    build_image
    start_service
    
    echo ""
    log_success "部署完成！"
    log_info "访问地址: http://localhost:8080"
    log_info "管理后台: http://localhost:8080/admin"
    log_info "默认用户名: admin"
    log_info "默认密码: admin123"
    echo ""
    log_warning "请及时修改默认密码！"
}

# 主函数
main() {
    case "${1:-help}" in
        "deploy")
            full_deploy
            ;;
        "build")
            check_docker
            build_image
            ;;
        "start")
            check_docker
            create_data_dir
            start_service
            ;;
        "stop")
            stop_service
            ;;
        "restart")
            restart_service
            ;;
        "status")
            check_status
            ;;
        "logs")
            view_logs
            ;;
        "backup")
            backup_data
            ;;
        "cleanup")
            cleanup
            ;;
        "help")
            show_help
            ;;
        *)
            log_error "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
}

# 执行主函数
main "$@"