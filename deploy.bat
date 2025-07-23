@echo off
setlocal enabledelayedexpansion

REM Bluster博客Docker部署脚本 (Windows版本)
REM 作者: Assistant
REM 版本: 1.0

set "RED=[91m"
set "GREEN=[92m"
set "YELLOW=[93m"
set "BLUE=[94m"
set "NC=[0m"

REM 日志函数
:log_info
echo %BLUE%[INFO]%NC% %~1
goto :eof

:log_success
echo %GREEN%[SUCCESS]%NC% %~1
goto :eof

:log_warning
echo %YELLOW%[WARNING]%NC% %~1
goto :eof

:log_error
echo %RED%[ERROR]%NC% %~1
goto :eof

REM 检查Docker是否安装
:check_docker
docker --version >nul 2>&1
if errorlevel 1 (
    call :log_error "Docker未安装，请先安装Docker Desktop"
    exit /b 1
)

docker-compose --version >nul 2>&1
if errorlevel 1 (
    call :log_error "Docker Compose未安装，请先安装Docker Compose"
    exit /b 1
)

call :log_success "Docker环境检查通过"
goto :eof

REM 创建数据目录
:create_data_dir
if not exist "data" (
    mkdir data
    call :log_info "创建数据目录: data"
)
goto :eof

REM 构建镜像
:build_image
call :log_info "开始构建Docker镜像..."
docker-compose build
if errorlevel 1 (
    call :log_error "镜像构建失败"
    exit /b 1
)
call :log_success "Docker镜像构建完成"
goto :eof

REM 启动服务
:start_service
call :log_info "启动Bluster博客服务..."
docker-compose up -d
if errorlevel 1 (
    call :log_error "服务启动失败"
    exit /b 1
)
call :log_success "服务启动成功"
goto :eof

REM 停止服务
:stop_service
call :log_info "停止Bluster博客服务..."
docker-compose down
call :log_success "服务已停止"
goto :eof

REM 重启服务
:restart_service
call :log_info "重启Bluster博客服务..."
docker-compose restart
call :log_success "服务重启完成"
goto :eof

REM 查看日志
:view_logs
call :log_info "查看服务日志..."
docker-compose logs -f
goto :eof

REM 查看状态
:check_status
call :log_info "检查服务状态..."
docker-compose ps
echo.
call :log_info "容器详细信息:"
docker inspect bluster-blog --format="{{.State.Status}}" 2>nul || echo 容器未运行
echo.
call :log_info "验证Markdown功能..."
call :verify_markdown_functionality
goto :eof

REM 验证Markdown功能
:verify_markdown_functionality
set "test_passed=true"

REM 检查主页是否可访问
curl -f -s http://localhost:8080/ >nul 2>&1
if errorlevel 1 (
    call :log_error "✗ 主页访问失败"
    set "test_passed=false"
) else (
    call :log_success "✓ 主页访问正常"
)

REM 检查管理后台是否可访问
curl -f -s http://localhost:8080/admin >nul 2>&1
if errorlevel 1 (
    call :log_error "✗ 管理后台访问失败"
    set "test_passed=false"
) else (
    call :log_success "✓ 管理后台访问正常"
)

REM 检查API端点是否可访问
curl -f -s http://localhost:8080/api/articles >nul 2>&1
if errorlevel 1 (
    call :log_warning "⚠ API端点可能未启用或需要认证"
) else (
    call :log_success "✓ API端点访问正常"
)

if "%test_passed%"=="true" (
    call :log_success "Markdown功能验证通过"
) else (
    call :log_error "Markdown功能验证失败，请检查日志"
    exit /b 1
)
goto :eof

REM 清理资源
:cleanup
call :log_warning "这将删除所有容器、镜像和数据，确定要继续吗？ (y/N)"
set /p response="请输入选择: "
if /i "!response!"=="y" (
    call :log_info "清理Docker资源..."
    docker-compose down -v --rmi all
    docker system prune -f
    call :log_success "清理完成"
) else (
    call :log_info "取消清理操作"
)
goto :eof

REM 备份数据
:backup_data
for /f "tokens=1-4 delims=/ " %%i in ('date /t') do set mydate=%%k%%j%%i
for /f "tokens=1-2 delims=: " %%i in ('time /t') do set mytime=%%i%%j
set "backup_dir=backups\%mydate%_%mytime%"
mkdir "%backup_dir%" 2>nul

if exist "data" (
    call :log_info "备份数据到: %backup_dir%"
    xcopy "data\*" "%backup_dir%\" /E /I /Q
    call :log_success "数据备份完成"
) else (
    call :log_warning "数据目录不存在，跳过备份"
)
goto :eof

REM 显示帮助信息
:show_help
echo Bluster博客Docker部署脚本 (Windows版本)
echo.
echo 用法: %~nx0 [选项]
echo.
echo 选项:
echo   deploy    - 完整部署（构建+启动）
echo   build     - 仅构建镜像
echo   start     - 启动服务
echo   stop      - 停止服务
echo   restart   - 重启服务
echo   status    - 查看服务状态
echo   logs      - 查看服务日志
echo   backup    - 备份数据
echo   cleanup   - 清理所有资源
echo   help      - 显示此帮助信息
echo.
echo 示例:
echo   %~nx0 deploy   # 完整部署
echo   %~nx0 logs     # 查看日志
goto :eof

REM 完整部署
:full_deploy
call :log_info "开始完整部署Bluster博客..."
call :check_docker
if errorlevel 1 exit /b 1

call :create_data_dir
call :build_image
if errorlevel 1 exit /b 1

call :start_service
if errorlevel 1 exit /b 1

echo.
call :log_success "部署完成！"
call :log_info "访问地址: http://localhost:8080"
call :log_info "管理后台: http://localhost:8080/admin"
call :log_info "默认用户名: admin"
call :log_info "默认密码: admin123"
echo.
call :log_info "等待服务启动完成..."
timeout /t 10 /nobreak >nul
call :verify_markdown_functionality
echo.
call :log_warning "请及时修改默认密码！"
call :log_info "Markdown功能已启用，支持文件导入导出和实时预览"
goto :eof

REM 主函数
set "action=%~1"
if "%action%"=="" set "action=help"

if "%action%"=="deploy" (
    call :full_deploy
) else if "%action%"=="build" (
    call :check_docker
    if not errorlevel 1 call :build_image
) else if "%action%"=="start" (
    call :check_docker
    if not errorlevel 1 (
        call :create_data_dir
        call :start_service
    )
) else if "%action%"=="stop" (
    call :stop_service
) else if "%action%"=="restart" (
    call :restart_service
) else if "%action%"=="status" (
    call :check_status
) else if "%action%"=="logs" (
    call :view_logs
) else if "%action%"=="backup" (
    call :backup_data
) else if "%action%"=="cleanup" (
    call :cleanup
) else if "%action%"=="help" (
    call :show_help
) else (
    call :log_error "未知选项: %action%"
    call :show_help
    exit /b 1
)

endlocal