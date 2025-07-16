# 使用官方Rust镜像作为构建环境
FROM 1.88.0-slim-trixie as builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /app

# 复制Cargo文件
COPY Cargo.toml ./

# 创建一个虚拟的main.rs来缓存依赖
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/main.rs

# 复制源代码
COPY src ./src
COPY templates ./templates

# 构建应用
RUN touch src/main.rs
RUN cargo build --release

# 使用轻量级的运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# 创建应用用户
RUN useradd -m -u 1000 appuser

# 设置工作目录
WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/bluster /app/bluster
COPY --from=builder /app/templates /app/templates

# 创建数据目录
RUN mkdir -p /app/data && chown -R appuser:appuser /app

# 切换到应用用户
USER appuser

# 暴露端口
EXPOSE 8080

# 设置环境变量
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:///app/data/blog.db

# 启动应用
CMD ["./bluster"]